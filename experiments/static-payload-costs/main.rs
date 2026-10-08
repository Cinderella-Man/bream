//! Standalone PERF-091 observation. No Bream runtime or external dependencies.
#![forbid(unsafe_code)]

use std::hint::black_box;
use std::sync::Arc;
use std::time::Instant;

const TARGET_VIEWS: usize = 2_000_000;
const WARMUP_VIEWS: usize = 100_000;

const fn bytes<const N: usize>(seed: u8) -> [u8; N] {
    let mut result = [0; N];
    let mut i = 0;
    while i < N {
        result[i] = seed.wrapping_add((i as u8).wrapping_mul(31));
        i += 1;
    }
    result
}

static A8: [u8; 8] = bytes(17);
static B8: [u8; 8] = bytes(83);
static A256: [u8; 256] = bytes(17);
static B256: [u8; 256] = bytes(83);
static A4096: [u8; 4096] = bytes(17);
static B4096: [u8; 4096] = bytes(83);

fn constants(size: usize) -> [&'static [u8]; 2] {
    match size {
        8 => [&A8, &B8],
        256 => [&A256, &B256],
        4096 => [&A4096, &B4096],
        _ => panic!("unsupported input size"),
    }
}

fn dynamic(size: usize, source: usize) -> Arc<[u8]> {
    let seed: u8 = if source == 0 { 17 } else { 83 };
    (0..size)
        .map(|i| seed.wrapping_add((i as u8).wrapping_mul(31)))
        .collect::<Vec<_>>()
        .into()
}

// This module's constructor only selects named, compiled static items. It has
// no constructor that leaks dynamic storage or accepts arbitrary static bytes.
mod approved {
    use super::{constants, dynamic, Arc};

    #[derive(Clone)]
    pub(super) enum Payload {
        Static(&'static [u8]),
        Shared(Arc<[u8]>),
    }

    impl Payload {
        pub(super) fn source(size: usize, source: usize, is_static: bool) -> Self {
            if is_static {
                Self::Static(constants(size)[source])
            } else {
                Self::Shared(dynamic(size, source))
            }
        }

        pub(super) fn owners(&self) -> Option<usize> {
            match self {
                Self::Static(_) => None,
                Self::Shared(value) => Some(Arc::strong_count(value)),
            }
        }
    }

    impl AsRef<[u8]> for Payload {
        fn as_ref(&self) -> &[u8] {
            match self {
                Self::Static(value) => value,
                Self::Shared(value) => value,
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Mix {
    Static,
    Mixed,
    Dynamic,
}

impl Mix {
    fn name(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Mixed => "mixed50",
            Self::Dynamic => "dynamic",
        }
    }

    fn is_static(self, source: usize) -> bool {
        match self {
            Self::Static => true,
            Self::Mixed => source == 0,
            Self::Dynamic => false,
        }
    }
}

#[derive(Clone, Copy)]
enum Kind {
    Arc,
    Direct,
    Tagged,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Arc => "arc",
            Self::Direct => "direct",
            Self::Tagged => "tagged",
        }
    }
}

fn digest(value: &[u8]) -> u64 {
    // Intentionally fixed work: this measures handle lifetime and dispatch,
    // not traversal throughput. Full byte equality is checked before timing.
    let value = black_box(value);
    value.len() as u64
        + value[0] as u64 * 3
        + value[value.len() / 2] as u64 * 5
        + value[value.len() - 1] as u64 * 7
}

#[inline(never)]
fn fanout<V: Clone + AsRef<[u8]>>(
    sources: &[V; 2],
    retained: &mut Vec<V>,
    width: usize,
    rounds: usize,
) -> u64 {
    let sources = black_box(sources);
    let mut checksum = 0u64;
    for round in 0..rounds {
        for view in 0..width {
            retained.push(sources[(round + view) & 1].clone());
        }
        // Every view remains owned until every view's checksum was consumed.
        black_box(&*retained);
        for value in retained.iter() {
            checksum = checksum.wrapping_add(digest(value.as_ref()));
        }
        // Last-owner destruction is excluded: source owners survive the run.
        retained.clear();
    }
    black_box(checksum)
}

fn expected(size: usize, width: usize, rounds: usize) -> u64 {
    let d = constants(size).map(digest);
    let total = width * rounds;
    let a = total / 2 + usize::from(width % 2 == 1 && rounds % 2 == 1);
    (a as u64).wrapping_mul(d[0])
        .wrapping_add(((total - a) as u64).wrapping_mul(d[1]))
}

fn measured<V: Clone + AsRef<[u8]>>(
    sources: [V; 2],
    size: usize,
    width: usize,
) -> (u128, usize, u64) {
    for (i, source) in sources.iter().enumerate() {
        assert_eq!(source.as_ref(), constants(size)[i]);
    }
    // Construction, vector allocation, source destruction, and complete byte
    // equality are outside the measured interval, for every representation.
    let mut retained = Vec::with_capacity(width);
    let warmup_rounds = WARMUP_VIEWS.div_ceil(width);
    assert_eq!(
        fanout(&sources, &mut retained, width, warmup_rounds),
        expected(size, width, warmup_rounds)
    );
    let rounds = TARGET_VIEWS.div_ceil(width);
    let start = Instant::now();
    let checksum = fanout(&sources, &mut retained, width, rounds);
    let elapsed = start.elapsed().as_nanos();
    assert_eq!(checksum, expected(size, width, rounds));
    assert!(retained.is_empty());
    (elapsed, rounds, checksum)
}

fn correctness() {
    for size in [8, 256, 4096] {
        for width in [1, 16, 256] {
            for source in 0..2 {
                let owner = dynamic(size, source);
                let weak = Arc::downgrade(&owner);
                let mut views: Vec<_> = (0..width).map(|_| owner.clone()).collect();
                assert_eq!(Arc::strong_count(&owner), width + 1);
                let survivor = views.pop().unwrap();
                drop(owner);
                views.reverse();
                while let Some(view) = views.pop() {
                    assert_eq!(&*view, constants(size)[source]);
                    drop(view);
                }
                assert!(weak.upgrade().is_some());
                assert_eq!(&*survivor, constants(size)[source]);
                assert_eq!(Arc::strong_count(&survivor), 1);
                drop(survivor);
                assert!(weak.upgrade().is_none());

                for is_static in [true, false] {
                    let owner = approved::Payload::source(size, source, is_static);
                    let mut views: Vec<_> = (0..width).map(|_| owner.clone()).collect();
                    assert_eq!(owner.owners(), if is_static { None } else { Some(width + 1) });
                    drop(owner);
                    views.reverse();
                    let survivor = views.pop().unwrap();
                    while let Some(view) = views.pop() {
                        assert_eq!(view.as_ref(), constants(size)[source]);
                    }
                    assert_eq!(survivor.as_ref(), constants(size)[source]);
                    assert_eq!(survivor.owners(), if is_static { None } else { Some(1) });
                }

                let direct = constants(size)[source];
                let mut views = vec![direct; width];
                let survivor = views.pop().unwrap();
                views.reverse();
                drop(views);
                assert_eq!(survivor, constants(size)[source]);
            }
        }
    }
}

fn main() {
    let sample: usize = std::env::args().nth(1).expect("sample index").parse().unwrap();
    correctness();
    println!("# correctness=passed target_views={TARGET_VIEWS} warmup_views={WARMUP_VIEWS}");
    println!("# layout_bytes arc={} direct={} tagged={}",
        std::mem::size_of::<Arc<[u8]>>(),
        std::mem::size_of::<&'static [u8]>(),
        std::mem::size_of::<approved::Payload>());
    println!("sample\torder\tbytes\tfanout\tmix\tkind\trounds\tviews\telapsed_ns\tchecksum");
    let mut cases = Vec::new();
    for size in [8, 256, 4096] {
        for width in [1, 16, 256] {
            for mix in [Mix::Static, Mix::Mixed, Mix::Dynamic] {
                cases.push((size, width, mix));
            }
        }
    }
    if sample % 2 == 1 {
        cases.reverse();
    }
    let mut order = 0;
    for (size, width, mix) in cases {
        let mut kinds = if matches!(mix, Mix::Static) {
            vec![Kind::Arc, Kind::Direct, Kind::Tagged]
        } else {
            vec![Kind::Arc, Kind::Tagged]
        };
        // Forward on even samples, reversed on odd samples; rotate triples to
        // keep each representation out of a permanent thermal/order position.
        if sample % 2 == 1 {
            kinds.reverse();
        }
        if kinds.len() == 3 {
            kinds.rotate_left(sample % 3);
        }
        for kind in kinds {
            let (elapsed, rounds, checksum) = match kind {
                Kind::Arc => measured(
                    std::array::from_fn(|i| {
                        if mix.is_static(i) { Arc::from(constants(size)[i]) }
                        else { dynamic(size, i) }
                    }), size, width),
                Kind::Direct => measured(constants(size), size, width),
                Kind::Tagged => measured(
                    std::array::from_fn(|i| approved::Payload::source(size, i, mix.is_static(i))),
                    size, width),
            };
            println!("{sample}\t{order}\t{size}\t{width}\t{}\t{}\t{rounds}\t{}\t{elapsed}\t{checksum}",
                mix.name(), kind.name(), width * rounds);
            order += 1;
        }
    }
}
