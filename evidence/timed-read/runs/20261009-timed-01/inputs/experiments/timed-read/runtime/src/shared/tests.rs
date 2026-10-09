use super::*;

#[test]
fn ingestion_and_cloning_preserve_the_original_byte_allocation() {
    let bytes = vec![3, 5, 7];
    let original = bytes.as_ptr();
    let (payload, receipt) = SharedBytes::new(bytes).unwrap();
    let other = payload.clone();
    assert_eq!(payload.storage.bytes.as_ptr(), original);
    assert!(Arc::ptr_eq(&payload.storage, &other.storage));
    assert_eq!((other.len(), other.read(2)), (3, 7));
    assert_eq!(receipt.snapshot().live_references, 2);
    drop(payload);
    assert_eq!(other.read(1), 5);
    assert_eq!(receipt.snapshot().physical_drops, 0);
    drop(other);
    assert_eq!(
        receipt.snapshot(),
        SharedSnapshot {
            live_bytes: 0,
            freed_bytes: 3,
            physical_drops: 1,
            live_references: 0,
            references_created: 2,
            references_dropped: 2,
        }
    );
}

#[test]
fn receipt_clones_never_retain_payload_storage() {
    let (payload, receipt) = SharedBytes::new(vec![1]).unwrap();
    let observer = receipt.clone();
    drop(payload);
    assert_eq!(observer.snapshot().physical_drops, 1);
    drop(receipt);
    assert_eq!(observer.snapshot().live_references, 0);
}

#[test]
fn accounting_uses_capacity_even_for_empty_payloads() {
    for capacity in [0, 9, 65536] {
        let bytes = Vec::with_capacity(capacity);
        let actual = bytes.capacity() as u64;
        let (payload, receipt) = SharedBytes::new(bytes).unwrap();
        assert!(payload.is_empty());
        assert_eq!(receipt.snapshot().live_bytes, actual);
        drop(payload);
        assert_eq!(
            (
                receipt.snapshot().freed_bytes,
                receipt.snapshot().physical_drops
            ),
            (actual, 1)
        );
    }
}

#[test]
fn capacity_limit_cannot_be_bypassed_with_short_length() {
    let mut bytes = Vec::with_capacity(65537);
    bytes.push(1);
    assert!(SharedBytes::new(bytes).is_err());
}

#[test]
fn envelope_move_does_not_manufacture_a_reference() {
    let (payload, receipt) = SharedBytes::new(vec![11]).unwrap();
    let message = SharedEnvelope::new(17, payload);
    assert_eq!(message.tag(), 17);
    let received = message.into_bytes();
    assert_eq!(
        (received.read(0), receipt.snapshot().references_created),
        (11, 1)
    );
    drop(received);
    assert_eq!(receipt.snapshot().references_dropped, 1);
}

#[test]
fn simultaneous_last_readers_release_physical_storage_once() {
    let (first, receipt) = SharedBytes::new(vec![13, 17]).unwrap();
    let second = first.clone();
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let threads: Vec<_> = [first, second]
        .into_iter()
        .map(|payload| {
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                assert_eq!(payload.read(0) + payload.read(1), 30);
                drop(payload);
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    let s = receipt.snapshot();
    assert_eq!(
        (
            s.live_references,
            s.references_created,
            s.references_dropped,
            s.physical_drops
        ),
        (0, 2, 2, 1)
    );
}
