//! Closed whole-module profile: owning records, scalar tags, acyclic helpers.
use proc_macro::TokenStream;
use proc_macro2::TokenStream as Tokens;
use quote::{format_ident, quote};
use std::collections::{BTreeMap, BTreeSet};
use syn::{spanned::Spanned, BinOp, Expr, Item, ItemFn, ItemMod, Lit, Pat, Stmt};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Unit,
    U64,
    Bool,
    Words,
    Pid,
    Message,
    SharedBytes,
    SharedMessage,
    SharedReceiveResult,
    ReceiveResult,
    MonitorRef,
    Down,
    Event,
    EventResult,
    DownResult,
    Exit,
    ExitResult,
    LinkResult,
    Spawned,
    Record(usize),
    Tag(usize),
}
#[derive(Clone)]
struct Data {
    name: syn::Ident,
    fields: BTreeMap<String, Kind>,
    ordered: Vec<(syn::Ident, Kind)>,
    variants: Vec<(syn::Ident, Option<Kind>)>,
    is_enum: bool,
}
#[derive(Clone)]
struct Function {
    item: ItemFn,
    args: Vec<(syn::Ident, bool, Kind)>,
    output: Kind,
}
struct Schema {
    data: Vec<Data>,
    functions: BTreeMap<String, Function>,
}
#[derive(Clone, Copy)]
struct Binding {
    kind: Kind,
    mutable: bool,
}
type Scope = BTreeMap<String, Binding>;

fn error(node: impl Spanned, reason: &str) -> syn::Error {
    syn::Error::new(node.span(), format!("bream structured subset: {reason}"))
}
fn allowed_name(ident: &syn::Ident) -> syn::Result<()> {
    let name = ident.to_string();
    if name.starts_with("r#")
        || name.starts_with("__bream_")
        || [
            "program",
            "u64",
            "bool",
            "Words",
            "Pid",
            "Message",
            "SharedBytes",
            "SharedMessage",
            "SharedReceiveResult",
            "clone_shared",
            "shared_len",
            "shared_read",
            "send_shared",
            "receive_shared",
            "receive_shared_any",
            "receive_shared_for",
            "receive_shared_any_for",
            "shared_tag",
            "shared_bytes",
            "ReceiveResult",
            "MonitorRef",
            "Down",
            "Event",
            "EventResult",
            "DownResult",
            "Exit",
            "ExitResult",
            "LinkResult",
            "Spawned",
            "spawn_link",
            "spawn_monitor",
            "spawn_link_monitor",
            "spawned_pid",
            "spawned_ref",
            "link",
            "try_link",
            "unlink",
            "trap_exit",
            "receive_exit",
            "receive_exit_for",
            "exit_from",
            "exit_reason",
            "monitor",
            "demonitor",
            "demonitor_flush",
            "receive_event",
            "receive_event_for",
            "receive_down",
            "receive_down_for",
            "down_ref",
            "down_target",
            "down_reason",
            "receive_for",
            "receive_any_for",
            "self_id",
            "input_target",
            "send",
            "receive",
            "receive_any",
            "message_tag",
            "message_words",
            "owned_input",
            "input_tag",
            "input_argument",
            "observe",
            "result",
            "mark",
            "read",
            "write",
            "len",
        ]
        .contains(&name.as_str())
    {
        return Err(error(ident, "reserved or raw identifier"));
    }
    Ok(())
}
fn segments(path: &syn::Path) -> Option<Vec<String>> {
    if path.leading_colon.is_some()
        || path
            .segments
            .iter()
            .any(|s| !matches!(s.arguments, syn::PathArguments::None))
    {
        return None;
    }
    Some(path.segments.iter().map(|s| s.ident.to_string()).collect())
}
fn ident_pattern(pat: &Pat) -> syn::Result<(&syn::Ident, bool)> {
    match pat {
        Pat::Ident(p) if p.attrs.is_empty() && p.by_ref.is_none() && p.subpat.is_none() => {
            allowed_name(&p.ident)?;
            Ok((&p.ident, p.mutability.is_some()))
        }
        _ => Err(error(pat, "only plain owning bindings are admitted")),
    }
}

impl Schema {
    fn kind(&self, ty: &syn::Type) -> syn::Result<Kind> {
        if let syn::Type::Path(path) = ty {
            if path.qself.is_none() {
                if let Some(parts) = segments(&path.path) {
                    if parts.len() == 1 {
                        return match parts[0].as_str() {
                            "u64" => Ok(Kind::U64),
                            "bool" => Ok(Kind::Bool),
                            "Words" => Ok(Kind::Words),
                            "Pid" => Ok(Kind::Pid),
                            "Message" => Ok(Kind::Message),
                            "SharedBytes" => Ok(Kind::SharedBytes),
                            "SharedMessage" => Ok(Kind::SharedMessage),
                            "SharedReceiveResult" => Ok(Kind::SharedReceiveResult),
                            "ReceiveResult" => Ok(Kind::ReceiveResult),
                            "MonitorRef" => Ok(Kind::MonitorRef),
                            "Down" => Ok(Kind::Down),
                            "Event" => Ok(Kind::Event),
                            "EventResult" => Ok(Kind::EventResult),
                            "DownResult" => Ok(Kind::DownResult),
                            "Exit" => Ok(Kind::Exit),
                            "ExitResult" => Ok(Kind::ExitResult),
                            "LinkResult" => Ok(Kind::LinkResult),
                            "Spawned" => Ok(Kind::Spawned),
                            name => self
                                .data
                                .iter()
                                .position(|d| d.name == name)
                                .map(|i| {
                                    if self.data[i].is_enum {
                                        Kind::Tag(i)
                                    } else {
                                        Kind::Record(i)
                                    }
                                })
                                .ok_or_else(|| error(ty, "unknown or external type")),
                        };
                    }
                }
            }
        }
        Err(error(
            ty,
            "only private checked data or concrete primitive/Words types are admitted",
        ))
    }
    fn tokens(&self, kind: Kind) -> Tokens {
        match kind {
            Kind::Unit => quote!(()),
            Kind::U64 => quote!(__bream_core::primitive::u64),
            Kind::Bool => quote!(__bream_core::primitive::bool),
            Kind::Words => quote!(__bream_runtime::OwnedWords),
            Kind::Pid => quote!(__bream_runtime::ProcessId),
            Kind::Message => quote!(__bream_runtime::Envelope),
            Kind::SharedBytes => quote!(__bream_runtime::SharedBytes),
            Kind::SharedMessage => quote!(__bream_runtime::SharedEnvelope),
            Kind::SharedReceiveResult => quote!(__bream_runtime::SharedReceiveResult),
            Kind::ReceiveResult => quote!(__bream_runtime::ReceiveResult),
            Kind::MonitorRef => quote!(__bream_runtime::MonitorRef),
            Kind::Down => quote!(__bream_runtime::Down),
            Kind::Event => quote!(__bream_runtime::Event),
            Kind::EventResult => quote!(__bream_runtime::EventResult),
            Kind::DownResult => quote!(__bream_runtime::DownResult),
            Kind::Exit => quote!(__bream_runtime::Exit),
            Kind::ExitResult => quote!(__bream_runtime::ExitResult),
            Kind::LinkResult => quote!(__bream_runtime::LinkResult),
            Kind::Spawned => quote!(__bream_runtime::Spawned),
            Kind::Record(i) | Kind::Tag(i) => {
                let name = &self.data[i].name;
                quote!(#name)
            }
        }
    }
    fn variant(&self, path: &syn::Path) -> syn::Result<(usize, syn::Ident, Option<Kind>)> {
        let parts = segments(path).ok_or_else(|| error(path, "external/generic constructor"))?;
        if parts.len() != 2 {
            return Err(error(path, "expected private Enum::Variant"));
        }
        for (i, data) in self.data.iter().enumerate() {
            if data.is_enum && data.name == parts[0] {
                if let Some((ident, kind)) = data.variants.iter().find(|(n, _)| *n == parts[1]) {
                    return Ok((i, ident.clone(), *kind));
                }
            }
        }
        Err(error(path, "unknown enum variant"))
    }
    fn pattern_variant(&self, path: &syn::Path) -> syn::Result<(Kind, Tokens, Option<Kind>)> {
        if let Some(parts) = segments(path) {
            if parts.len() == 2 && parts[0] == "Event" {
                return match parts[1].as_str() {
                    "Message" => Ok((
                        Kind::Event,
                        quote!(__bream_runtime::Event::Message),
                        Some(Kind::Message),
                    )),
                    "Shared" => Ok((
                        Kind::Event,
                        quote!(__bream_runtime::Event::Shared),
                        Some(Kind::SharedMessage),
                    )),
                    "Down" => Ok((
                        Kind::Event,
                        quote!(__bream_runtime::Event::Down),
                        Some(Kind::Down),
                    )),
                    "Exit" => Ok((
                        Kind::Event,
                        quote!(__bream_runtime::Event::Exit),
                        Some(Kind::Exit),
                    )),
                    _ => Err(error(path, "unknown event variant")),
                };
            }
            if parts.len() == 2 && parts[0] == "EventResult" {
                return match parts[1].as_str() {
                    "Event" => Ok((
                        Kind::EventResult,
                        quote!(__bream_runtime::EventResult::Event),
                        Some(Kind::Event),
                    )),
                    "Timeout" => Ok((
                        Kind::EventResult,
                        quote!(__bream_runtime::EventResult::Timeout),
                        None,
                    )),
                    _ => Err(error(path, "unknown event result variant")),
                };
            }
            if parts.len() == 2 && parts[0] == "DownResult" {
                return match parts[1].as_str() {
                    "Down" => Ok((
                        Kind::DownResult,
                        quote!(__bream_runtime::DownResult::Down),
                        Some(Kind::Down),
                    )),
                    "Timeout" => Ok((
                        Kind::DownResult,
                        quote!(__bream_runtime::DownResult::Timeout),
                        None,
                    )),
                    _ => Err(error(path, "unknown down result variant")),
                };
            }
            if parts.len() == 2 && parts[0] == "LinkResult" {
                return match parts[1].as_str() {
                    "Linked" => Ok((
                        Kind::LinkResult,
                        quote!(__bream_runtime::LinkResult::Linked),
                        None,
                    )),
                    "NoProcess" => Ok((
                        Kind::LinkResult,
                        quote!(__bream_runtime::LinkResult::NoProcess),
                        None,
                    )),
                    _ => Err(error(path, "unknown link result variant")),
                };
            }
            if parts.len() == 2 && parts[0] == "ExitResult" {
                return match parts[1].as_str() {
                    "Exit" => Ok((
                        Kind::ExitResult,
                        quote!(__bream_runtime::ExitResult::Exit),
                        Some(Kind::Exit),
                    )),
                    "Timeout" => Ok((
                        Kind::ExitResult,
                        quote!(__bream_runtime::ExitResult::Timeout),
                        None,
                    )),
                    _ => Err(error(path, "unknown exit result variant")),
                };
            }
            if parts.len() == 2 && parts[0] == "SharedReceiveResult" {
                return match parts[1].as_str() {
                    "Message" => Ok((
                        Kind::SharedReceiveResult,
                        quote!(__bream_runtime::SharedReceiveResult::Message),
                        Some(Kind::SharedMessage),
                    )),
                    "Timeout" => Ok((
                        Kind::SharedReceiveResult,
                        quote!(__bream_runtime::SharedReceiveResult::Timeout),
                        None,
                    )),
                    _ => Err(error(path, "unknown shared receive result variant")),
                };
            }
            if parts.len() == 2 && parts[0] == "ReceiveResult" {
                return match parts[1].as_str() {
                    "Message" => Ok((
                        Kind::ReceiveResult,
                        quote!(__bream_runtime::ReceiveResult::Message),
                        Some(Kind::Message),
                    )),
                    "Timeout" => Ok((
                        Kind::ReceiveResult,
                        quote!(__bream_runtime::ReceiveResult::Timeout),
                        None,
                    )),
                    _ => Err(error(path, "unknown receive result variant")),
                };
            }
        }
        let (index, variant, payload) = self.variant(path)?;
        let name = &self.data[index].name;
        Ok((Kind::Tag(index), quote!(#name::#variant), payload))
    }
}

struct Lowerer<'a> {
    schema: &'a Schema,
    names: BTreeSet<String>,
    calls: BTreeSet<String>,
    output: Kind,
}

impl<'a> Lowerer<'a> {
    fn bind(&mut self, ident: &syn::Ident) -> syn::Result<()> {
        allowed_name(ident)?;
        if self.schema.functions.contains_key(&ident.to_string())
            || !self.names.insert(ident.to_string())
        {
            return Err(error(
                ident,
                "shadowing or function-name binding is forbidden",
            ));
        }
        Ok(())
    }
    fn place(&self, expr: &Expr, scope: &Scope) -> syn::Result<(Tokens, Binding)> {
        match expr {
            Expr::Path(p) if p.attrs.is_empty() && p.qself.is_none() => {
                let parts = segments(&p.path).ok_or_else(|| error(expr, "external place"))?;
                if parts.len() != 1 {
                    return Err(error(expr, "expected local place"));
                }
                let binding = scope
                    .get(&parts[0])
                    .ok_or_else(|| error(expr, "unknown local"))?;
                Ok((quote!(#p), *binding))
            }
            Expr::Field(f) if f.attrs.is_empty() => {
                let (base, binding) = self.place(&f.base, scope)?;
                let Kind::Record(i) = binding.kind else {
                    return Err(error(expr, "field access requires a checked record"));
                };
                let syn::Member::Named(name) = &f.member else {
                    return Err(error(expr, "unnamed field"));
                };
                let kind = self.schema.data[i]
                    .fields
                    .get(&name.to_string())
                    .ok_or_else(|| error(expr, "unknown field"))?;
                Ok((
                    quote!((#base).#name),
                    Binding {
                        kind: *kind,
                        mutable: binding.mutable,
                    },
                ))
            }
            _ => Err(error(
                expr,
                "only local or checked-record places are admitted",
            )),
        }
    }
    fn words(&self, expr: &Expr, scope: &Scope, mutable: bool) -> syn::Result<Tokens> {
        let (tokens, binding) = self.place(expr, scope)?;
        if binding.kind != Kind::Words || (mutable && !binding.mutable) {
            return Err(error(
                expr,
                "read/write/len requires concrete Words with matching mutability",
            ));
        }
        Ok(tokens)
    }
    fn value(&mut self, expr: &Expr, scope: &Scope) -> syn::Result<(Tokens, Kind)> {
        match expr {
            Expr::Lit(lit) if lit.attrs.is_empty() => match &lit.lit {
                Lit::Int(n) if n.suffix().is_empty() || n.suffix() == "u64" => {
                    let n = n.base10_parse::<u64>()?;
                    Ok((quote!(#n), Kind::U64))
                }
                Lit::Bool(b) => Ok((quote!(#b), Kind::Bool)),
                _ => Err(error(expr, "unsupported literal")),
            },
            Expr::Path(p)
                if p.attrs.is_empty() && p.qself.is_none() && p.path.segments.len() == 2 =>
            {
                let (i, variant, payload) = self.schema.variant(&p.path)?;
                if payload.is_some() {
                    return Err(error(expr, "variant needs its payload"));
                }
                let name = &self.schema.data[i].name;
                Ok((quote!(#name::#variant), Kind::Tag(i)))
            }
            Expr::Path(_) | Expr::Field(_) => {
                let (tokens, binding) = self.place(expr, scope)?;
                Ok((tokens, binding.kind))
            }
            Expr::Paren(p) if p.attrs.is_empty() => {
                let (v, k) = self.value(&p.expr, scope)?;
                Ok((quote!((#v)), k))
            }
            Expr::Unary(u) if u.attrs.is_empty() && matches!(u.op, syn::UnOp::Not(_)) => {
                let (v, k) = self.value(&u.expr, scope)?;
                if !matches!(k, Kind::U64 | Kind::Bool) {
                    return Err(error(expr, "not requires a primitive"));
                }
                Ok((quote!(!(#v)), k))
            }
            Expr::Binary(b) if b.attrs.is_empty() => {
                let (l, lk) = self.value(&b.left, scope)?;
                let (r, rk) = self.value(&b.right, scope)?;
                let op = &b.op;
                match op {
                    BinOp::Add(_) | BinOp::Sub(_) | BinOp::Mul(_)
                        if lk == Kind::U64 && rk == Kind::U64 =>
                    {
                        let method = match op {
                            BinOp::Add(_) => quote!(wrapping_add),
                            BinOp::Sub(_) => quote!(wrapping_sub),
                            _ => quote!(wrapping_mul),
                        };
                        Ok((quote!((#l).#method(#r)), Kind::U64))
                    }
                    BinOp::BitAnd(_) | BinOp::BitOr(_) | BinOp::BitXor(_)
                        if lk == Kind::U64 && rk == Kind::U64 =>
                    {
                        Ok((quote!((#l) #op (#r)), Kind::U64))
                    }
                    BinOp::Lt(_) | BinOp::Le(_) | BinOp::Gt(_) | BinOp::Ge(_)
                        if lk == Kind::U64 && rk == Kind::U64 =>
                    {
                        Ok((quote!((#l) #op (#r)), Kind::Bool))
                    }
                    BinOp::Eq(_) | BinOp::Ne(_)
                        if lk == rk
                            && matches!(
                                lk,
                                Kind::U64 | Kind::Bool | Kind::Pid | Kind::MonitorRef
                            ) =>
                    {
                        Ok((quote!((#l) #op (#r)), Kind::Bool))
                    }
                    BinOp::And(_) | BinOp::Or(_) if lk == Kind::Bool && rk == Kind::Bool => {
                        Ok((quote!((#l) #op (#r)), Kind::Bool))
                    }
                    _ => Err(error(expr, "unsupported primitive operator/types")),
                }
            }
            Expr::Struct(s) if s.attrs.is_empty() && s.qself.is_none() && s.rest.is_none() => {
                let parts = segments(&s.path).ok_or_else(|| error(expr, "external constructor"))?;
                if parts.len() != 1 {
                    return Err(error(expr, "constructor requires checked record"));
                }
                let i = self
                    .schema
                    .data
                    .iter()
                    .position(|d| !d.is_enum && d.name == parts[0])
                    .ok_or_else(|| error(expr, "unknown record"))?;
                let data = &self.schema.data[i];
                let name = data.name.clone();
                let fields = data.fields.clone();
                let mut seen = BTreeSet::new();
                let mut values = Vec::new();
                for field in &s.fields {
                    let syn::Member::Named(member) = &field.member else {
                        return Err(error(field, "named fields required"));
                    };
                    if !field.attrs.is_empty() || !seen.insert(member.to_string()) {
                        return Err(error(field, "field attributes/duplicates forbidden"));
                    }
                    let expected = fields
                        .get(&member.to_string())
                        .ok_or_else(|| error(field, "unknown field"))?;
                    let (value, kind) = self.value(&field.expr, scope)?;
                    if kind != *expected {
                        return Err(error(field, "field type mismatch"));
                    }
                    values.push(quote!(#member: #value));
                }
                if seen.len() != fields.len() {
                    return Err(error(expr, "all record fields required"));
                }
                // Keep authored initializer order, including suspension in a later field.
                Ok((quote!(#name { #(#values),* }), Kind::Record(i)))
            }
            Expr::Call(call) if call.attrs.is_empty() => self.call(call, scope),
            _ => Err(error(
                expr,
                "unsupported expression or unqualified execution",
            )),
        }
    }
    fn call(&mut self, call: &syn::ExprCall, scope: &Scope) -> syn::Result<(Tokens, Kind)> {
        let Expr::Path(path) = &*call.func else {
            return Err(error(call, "function values/callbacks forbidden"));
        };
        if !path.attrs.is_empty() || path.qself.is_some() {
            return Err(error(call, "call attributes/qualified self forbidden"));
        }
        let parts =
            segments(&path.path).ok_or_else(|| error(call, "external/generic calls forbidden"))?;
        if parts.len() == 2 {
            let (i, variant, payload) = self.schema.variant(&path.path)?;
            let Some(expected) = payload else {
                return Err(error(call, "unit variant takes no arguments"));
            };
            if call.args.len() != 1 {
                return Err(error(call, "variant arity"));
            }
            let (value, kind) = self.value(&call.args[0], scope)?;
            if kind != expected {
                return Err(error(call, "variant payload type"));
            }
            let name = &self.schema.data[i].name;
            return Ok((quote!(#name::#variant(#value)), Kind::Tag(i)));
        }
        if parts.len() != 1 {
            return Err(error(call, "external calls forbidden"));
        }
        let name = parts[0].as_str();
        if ["spawn_link", "spawn_monitor", "spawn_link_monitor"].contains(&name) {
            if call.args.len() != 4 {
                return Err(error(
                    call,
                    "spawn requires a local child, moved Words, u64 tag and u64 argument",
                ));
            }
            let Expr::Path(child) = &call.args[0] else {
                return Err(error(
                    &call.args[0],
                    "spawn child must be a bare checked local function",
                ));
            };
            if !child.attrs.is_empty() || child.qself.is_some() {
                return Err(error(
                    child,
                    "spawn child must be a bare checked local function",
                ));
            }
            let child_parts = segments(&child.path)
                .ok_or_else(|| error(child, "spawn child must be a bare checked local function"))?;
            if child_parts.len() != 1 {
                return Err(error(
                    child,
                    "spawn child must be a bare checked local function",
                ));
            }
            let child_name = &child_parts[0];
            let function = self.schema.functions.get(child_name).ok_or_else(|| {
                error(child, "spawn child must be declared in this checked module")
            })?;
            if !function.args.is_empty() || function.output != Kind::Unit {
                return Err(error(
                    child,
                    "spawn child requires no parameters and unit output",
                ));
            }
            self.calls.insert(child_name.clone());
            let factory = format_ident!("__bream_child_{}", child_name);
            let (words, wk) = self.value(&call.args[1], scope)?;
            let (tag, tk) = self.value(&call.args[2], scope)?;
            let (argument, ak) = self.value(&call.args[3], scope)?;
            if (wk, tk, ak) != (Kind::Words, Kind::U64, Kind::U64) {
                return Err(error(
                    call,
                    "spawn requires moved Words, u64 tag and u64 argument",
                ));
            }
            let method = &path.path.segments[0].ident;
            return Ok((
                quote!({
                    let __bream_child_program = #factory();
                    let __bream_words = #words;
                    let __bream_tag = #tag;
                    let __bream_argument = #argument;
                    __bream_context.#method(__bream_child_program, __bream_words, __bream_tag, __bream_argument)
                }),
                if name == "spawn_link" {
                    Kind::Pid
                } else {
                    Kind::Spawned
                },
            ));
        }
        if ["spawned_pid", "spawned_ref"].contains(&name) && call.args.len() == 1 {
            let (child, kind) = self.value(&call.args[0], scope)?;
            if kind != Kind::Spawned {
                return Err(error(
                    call,
                    "spawn result operation requires concrete Spawned",
                ));
            }
            return Ok(if name == "spawned_pid" {
                (quote!((#child).pid()), Kind::Pid)
            } else {
                (quote!((#child).reference()), Kind::MonitorRef)
            });
        }
        if ["self_id", "input_target"].contains(&name) && call.args.is_empty() {
            let method = &path.path.segments[0].ident;
            return Ok((quote!(__bream_context.#method()), Kind::Pid));
        }
        if ["link", "try_link", "unlink"].contains(&name) && call.args.len() == 1 {
            let (target, kind) = self.value(&call.args[0], scope)?;
            if kind != Kind::Pid {
                return Err(error(call, "link/try_link/unlink requires concrete Pid"));
            }
            let method = &path.path.segments[0].ident;
            return Ok((
                quote!({
                    let __bream_target = #target;
                    __bream_context.#method(__bream_target)
                }),
                if name == "try_link" {
                    Kind::LinkResult
                } else {
                    Kind::Unit
                },
            ));
        }
        if name == "trap_exit" && call.args.len() == 1 {
            let (enabled, kind) = self.value(&call.args[0], scope)?;
            if kind != Kind::Bool {
                return Err(error(call, "trap_exit requires concrete bool"));
            }
            return Ok((
                quote!({
                    let __bream_enabled = #enabled;
                    __bream_context.trap_exit(__bream_enabled)
                }),
                Kind::Bool,
            ));
        }
        if (name == "receive_exit" && call.args.len() == 1)
            || (name == "receive_exit_for" && call.args.len() == 2)
        {
            let (target, kind) = self.value(&call.args[0], scope)?;
            if kind != Kind::Pid {
                return Err(error(
                    call,
                    "exit receive requires a fixed Pid, not a predicate",
                ));
            }
            if name == "receive_exit" {
                return Ok((
                    quote!({
                        let __bream_target = #target;
                        __bream_context.receive_exit(__bream_target).await
                    }),
                    Kind::Exit,
                ));
            }
            let (delay, kind) = self.value(&call.args[1], scope)?;
            if kind != Kind::U64 {
                return Err(error(call, "exit timeout requires u64 ticks"));
            }
            return Ok((
                quote!({
                    let __bream_target = #target;
                    let __bream_delay = #delay;
                    __bream_context.receive_exit_for(__bream_target, __bream_delay).await
                }),
                Kind::ExitResult,
            ));
        }
        if ["exit_from", "exit_reason"].contains(&name) && call.args.len() == 1 {
            let (exit, kind) = self.value(&call.args[0], scope)?;
            if kind != Kind::Exit {
                return Err(error(call, "exit operation requires concrete Exit"));
            }
            return Ok(match name {
                "exit_from" => (quote!((#exit).from()), Kind::Pid),
                _ => (quote!((#exit).reason_code()), Kind::U64),
            });
        }
        if name == "monitor" && call.args.len() == 1 {
            let (target, kind) = self.value(&call.args[0], scope)?;
            if kind != Kind::Pid {
                return Err(error(call, "monitor requires concrete Pid"));
            }
            return Ok((
                quote!({
                    let __bream_target = #target;
                    __bream_context.monitor(__bream_target)
                }),
                Kind::MonitorRef,
            ));
        }
        if ["demonitor", "demonitor_flush"].contains(&name) && call.args.len() == 1 {
            let (reference, kind) = self.value(&call.args[0], scope)?;
            if kind != Kind::MonitorRef {
                return Err(error(call, "demonitor requires concrete MonitorRef"));
            }
            let operation = if name == "demonitor_flush" {
                quote!(__bream_context.demonitor_flush(__bream_reference).await)
            } else {
                quote!(__bream_context.demonitor(__bream_reference))
            };
            return Ok((
                quote!({ let __bream_reference = #reference; #operation; }),
                Kind::Unit,
            ));
        }
        if name == "receive_event" && call.args.is_empty() {
            return Ok((quote!(__bream_context.receive_event().await), Kind::Event));
        }
        if name == "receive_event_for" && call.args.len() == 1 {
            let (delay, kind) = self.value(&call.args[0], scope)?;
            if kind != Kind::U64 {
                return Err(error(call, "event timeout requires u64 ticks"));
            }
            return Ok((
                quote!({
                    let __bream_delay = #delay;
                    __bream_context.receive_event_for(__bream_delay).await
                }),
                Kind::EventResult,
            ));
        }
        if (name == "receive_down" && call.args.len() == 1)
            || (name == "receive_down_for" && call.args.len() == 2)
        {
            let (reference, kind) = self.value(&call.args[0], scope)?;
            if kind != Kind::MonitorRef {
                return Err(error(
                    call,
                    "down receive requires a fixed MonitorRef, not a predicate",
                ));
            }
            if name == "receive_down" {
                return Ok((
                    quote!({
                        let __bream_reference = #reference;
                        __bream_context.receive_down(__bream_reference).await
                    }),
                    Kind::Down,
                ));
            }
            let (delay, kind) = self.value(&call.args[1], scope)?;
            if kind != Kind::U64 {
                return Err(error(call, "down timeout requires u64 ticks"));
            }
            return Ok((
                quote!({
                    let __bream_reference = #reference;
                    let __bream_delay = #delay;
                    __bream_context.receive_down_for(__bream_reference, __bream_delay).await
                }),
                Kind::DownResult,
            ));
        }
        if ["down_ref", "down_target", "down_reason"].contains(&name) && call.args.len() == 1 {
            let (down, kind) = self.value(&call.args[0], scope)?;
            if kind != Kind::Down {
                return Err(error(call, "down operation requires concrete Down"));
            }
            return Ok(match name {
                "down_ref" => (quote!((#down).reference()), Kind::MonitorRef),
                "down_target" => (quote!((#down).target()), Kind::Pid),
                _ => (quote!((#down).reason_code()), Kind::U64),
            });
        }
        if ["clone_shared", "shared_len", "shared_read"].contains(&name)
            && call.args.len() == if name == "shared_read" { 2 } else { 1 }
        {
            let (bytes, binding) = self.place(&call.args[0], scope)?;
            if binding.kind != Kind::SharedBytes {
                return Err(error(
                    call,
                    "shared byte operation requires concrete SharedBytes place",
                ));
            }
            return match name {
                "clone_shared" => Ok((quote!((#bytes).clone()), Kind::SharedBytes)),
                "shared_len" => Ok((quote!((#bytes).len()), Kind::U64)),
                _ => {
                    let (index, kind) = self.value(&call.args[1], scope)?;
                    if kind != Kind::U64 {
                        return Err(error(call, "shared byte index requires u64"));
                    }
                    Ok((quote!((#bytes).read(#index)), Kind::U64))
                }
            };
        }
        if name == "send_shared" && call.args.len() == 3 {
            let (target, tk) = self.value(&call.args[0], scope)?;
            let (tag, tagk) = self.value(&call.args[1], scope)?;
            let (bytes, bk) = self.value(&call.args[2], scope)?;
            if (tk, tagk, bk) != (Kind::Pid, Kind::U64, Kind::SharedBytes) {
                return Err(error(
                    call,
                    "send_shared requires Pid, u64 tag and moved SharedBytes",
                ));
            }
            return Ok((
                quote!({
                    let __bream_target = #target;
                    let __bream_tag = #tag;
                    let __bream_bytes = #bytes;
                    __bream_context.send_shared(__bream_target, __bream_tag, __bream_bytes);
                }),
                Kind::Unit,
            ));
        }
        if name == "receive_shared" && call.args.len() == 1 {
            let (tag, kind) = self.value(&call.args[0], scope)?;
            if kind != Kind::U64 {
                return Err(error(
                    call,
                    "shared receive requires a fixed u64 tag, not a predicate",
                ));
            }
            return Ok((
                quote!({
                    let __bream_tag = #tag;
                    __bream_context.receive_shared(__bream_core::option::Option::Some(__bream_tag)).await
                }),
                Kind::SharedMessage,
            ));
        }
        if name == "receive_shared_any" && call.args.is_empty() {
            return Ok((
                quote!(
                    __bream_context
                        .receive_shared(__bream_core::option::Option::None)
                        .await
                ),
                Kind::SharedMessage,
            ));
        }
        if (name == "receive_shared_for" && call.args.len() == 2)
            || (name == "receive_shared_any_for" && call.args.len() == 1)
        {
            let (tag, delay_index) = if name == "receive_shared_for" {
                let (tag, kind) = self.value(&call.args[0], scope)?;
                if kind != Kind::U64 {
                    return Err(error(call, "timed shared receive requires a fixed u64 tag"));
                }
                (quote!(__bream_core::option::Option::Some(#tag)), 1)
            } else {
                (quote!(__bream_core::option::Option::None), 0)
            };
            let (delay, kind) = self.value(&call.args[delay_index], scope)?;
            if kind != Kind::U64 {
                return Err(error(call, "shared receive timeout requires u64 ticks"));
            }
            return Ok((
                quote!({
                    let __bream_tag = #tag;
                    let __bream_delay = #delay;
                    __bream_context.receive_shared_for(__bream_tag, __bream_delay).await
                }),
                Kind::SharedReceiveResult,
            ));
        }
        if ["shared_tag", "shared_bytes"].contains(&name) && call.args.len() == 1 {
            let (message, kind) = self.value(&call.args[0], scope)?;
            if kind != Kind::SharedMessage {
                return Err(error(
                    call,
                    "shared message operation requires concrete SharedMessage",
                ));
            }
            return if name == "shared_tag" {
                Ok((quote!((#message).tag()), Kind::U64))
            } else {
                Ok((quote!((#message).into_bytes()), Kind::SharedBytes))
            };
        }
        if name == "send" && call.args.len() == 3 {
            let (target, tk) = self.value(&call.args[0], scope)?;
            let (tag, tagk) = self.value(&call.args[1], scope)?;
            let (words, wk) = self.value(&call.args[2], scope)?;
            if (tk, tagk, wk) != (Kind::Pid, Kind::U64, Kind::Words) {
                return Err(error(call, "send requires Pid, u64 tag and moved Words"));
            }
            return Ok((
                quote!({
                    let __bream_target = #target;
                    let __bream_tag = #tag;
                    let __bream_words = #words;
                    __bream_context.send(__bream_target, __bream_tag, __bream_words);
                }),
                Kind::Unit,
            ));
        }
        if name == "receive" && call.args.len() == 1 {
            let (tag, kind) = self.value(&call.args[0], scope)?;
            if kind != Kind::U64 {
                return Err(error(
                    call,
                    "receive requires a fixed u64 tag, not a predicate",
                ));
            }
            return Ok((
                quote!({
                    let __bream_tag = #tag;
                    __bream_context.receive(__bream_core::option::Option::Some(__bream_tag)).await
                }),
                Kind::Message,
            ));
        }
        if name == "receive_any" && call.args.is_empty() {
            return Ok((
                quote!(
                    __bream_context
                        .receive(__bream_core::option::Option::None)
                        .await
                ),
                Kind::Message,
            ));
        }
        if (name == "receive_for" && call.args.len() == 2)
            || (name == "receive_any_for" && call.args.len() == 1)
        {
            let (tag, delay_index) = if name == "receive_for" {
                let (tag, kind) = self.value(&call.args[0], scope)?;
                if kind != Kind::U64 {
                    return Err(error(call, "timed receive requires a fixed u64 tag"));
                }
                (quote!(__bream_core::option::Option::Some(#tag)), 1)
            } else {
                (quote!(__bream_core::option::Option::None), 0)
            };
            let (delay, kind) = self.value(&call.args[delay_index], scope)?;
            if kind != Kind::U64 {
                return Err(error(call, "receive timeout requires u64 ticks"));
            }
            return Ok((
                quote!({
                    let __bream_tag = #tag;
                    let __bream_delay = #delay;
                    __bream_context.receive_for(__bream_tag, __bream_delay).await
                }),
                Kind::ReceiveResult,
            ));
        }
        if ["message_tag", "message_words"].contains(&name) && call.args.len() == 1 {
            let (message, kind) = self.value(&call.args[0], scope)?;
            if kind != Kind::Message {
                return Err(error(call, "message operation requires concrete Message"));
            }
            return if name == "message_tag" {
                Ok((quote!((#message).tag()), Kind::U64))
            } else {
                Ok((quote!((#message).into_words()), Kind::Words))
            };
        }
        if ["owned_input", "input_tag", "input_argument"].contains(&name) && call.args.is_empty() {
            let method = &path.path.segments[0].ident;
            return Ok((
                quote!(__bream_context.#method()),
                if name == "owned_input" {
                    Kind::Words
                } else {
                    Kind::U64
                },
            ));
        }
        if ["observe", "result", "mark"].contains(&name) && call.args.len() == 1 {
            let (value, kind) = self.value(&call.args[0], scope)?;
            if kind != Kind::U64 {
                return Err(error(call, "observation requires u64"));
            }
            let method = &path.path.segments[0].ident;
            // Evaluate a checked helper argument before borrowing the context
            // receiver; otherwise result(helper()) creates overlapping borrows.
            return Ok((
                quote!({ let __bream_value = #value; __bream_context.#method(__bream_value) }),
                Kind::Unit,
            ));
        }
        if name == "len" && call.args.len() == 1 {
            let words = self.words(&call.args[0], scope, false)?;
            return Ok((quote!((#words).len()), Kind::U64));
        }
        if ["read", "write"].contains(&name)
            && call.args.len() == if name == "read" { 2 } else { 3 }
        {
            let words = self.words(&call.args[0], scope, name == "write")?;
            let (index, kind) = self.value(&call.args[1], scope)?;
            if kind != Kind::U64 {
                return Err(error(call, "index requires u64"));
            }
            if name == "read" {
                return Ok((quote!((#words).read(#index)), Kind::U64));
            }
            let (value, kind) = self.value(&call.args[2], scope)?;
            if kind != Kind::U64 {
                return Err(error(call, "write value requires u64"));
            }
            return Ok((quote!((#words).write(#index, #value)), Kind::Unit));
        }
        let function = self
            .schema
            .functions
            .get(name)
            .ok_or_else(|| error(call, "unknown/unqualified helper"))?
            .clone();
        if name == "run" {
            return Err(error(call, "entry cannot be called as helper"));
        }
        if function.args.len() != call.args.len() {
            return Err(error(call, "helper arity mismatch"));
        }
        let mut values = Vec::new();
        for (arg, (_, _, expected)) in call.args.iter().zip(&function.args) {
            let (value, kind) = self.value(arg, scope)?;
            if kind != *expected {
                return Err(error(arg, "helper argument type mismatch"));
            }
            values.push(value);
        }
        self.calls.insert(name.to_owned());
        let target = format_ident!("__bream_helper_{}", name);
        // Author arguments are evaluated left-to-right BEFORE reborrowing the
        // shared context, allowing nested calls and owning argument temporaries.
        Ok((
            quote!(#target(#(#values,)* &mut *__bream_context).await),
            function.output,
        ))
    }
    fn block(&mut self, block: &syn::Block, parent: &Scope, in_loop: bool) -> syn::Result<Tokens> {
        let mut scope = parent.clone();
        let mut output = Tokens::new();
        for statement in &block.stmts {
            output.extend(quote!(__bream_context.checkpoint().await;));
            match statement {
                Stmt::Local(local) if local.attrs.is_empty() => {
                    let init = local
                        .init
                        .as_ref()
                        .ok_or_else(|| error(local, "initialization required"))?;
                    if init.diverge.is_some() {
                        return Err(error(local, "let-else forbidden"));
                    }
                    let (pattern, annotation) = match &local.pat {
                        Pat::Type(p) if p.attrs.is_empty() => (&*p.pat, Some(&*p.ty)),
                        other => (other, None),
                    };
                    let (ident, mutable) = ident_pattern(pattern)?;
                    let (value, kind) = self.value(&init.expr, &scope)?;
                    if kind == Kind::Unit {
                        return Err(error(local, "unit locals not admitted"));
                    }
                    if let Some(ty) = annotation {
                        if self.schema.kind(ty)? != kind {
                            return Err(error(ty, "annotation type mismatch"));
                        }
                    }
                    self.bind(ident)?;
                    let ty = self.schema.tokens(kind);
                    let mutability = if mutable { quote!(mut) } else { quote!() };
                    output.extend(quote!(let #mutability #ident: #ty = #value;));
                    scope.insert(ident.to_string(), Binding { kind, mutable });
                }
                Stmt::Expr(expr, _) => output.extend(self.statement(expr, &scope, in_loop)?),
                _ => {
                    return Err(error(
                        statement,
                        "local items, attributes and macros forbidden",
                    ))
                }
            }
        }
        Ok(output)
    }
    fn statement(&mut self, expr: &Expr, scope: &Scope, in_loop: bool) -> syn::Result<Tokens> {
        match expr {
            Expr::Loop(l) if l.attrs.is_empty() && l.label.is_none() => {
                let body = self.block(&l.body, scope, true)?;
                Ok(quote!(loop { __bream_context.checkpoint().await; #body }))
            }
            Expr::While(w) if w.attrs.is_empty() && w.label.is_none() => {
                let (condition, kind) = self.value(&w.cond, scope)?;
                if kind != Kind::Bool {
                    return Err(error(expr, "condition requires bool"));
                }
                let body = self.block(&w.body, scope, true)?;
                Ok(
                    quote!(loop { __bream_context.checkpoint().await; if !(#condition) { break; } #body }),
                )
            }
            Expr::ForLoop(f) if f.attrs.is_empty() && f.label.is_none() => {
                let (ident, mutable) = ident_pattern(&f.pat)?;
                if mutable {
                    return Err(error(expr, "mutable range binding forbidden"));
                }
                let Expr::Range(range) = &*f.expr else {
                    return Err(error(expr, "only primitive exclusive ranges"));
                };
                if !range.attrs.is_empty() || !matches!(range.limits, syn::RangeLimits::HalfOpen(_))
                {
                    return Err(error(expr, "range form unsupported"));
                }
                let (start, sk) = self.value(
                    range
                        .start
                        .as_ref()
                        .ok_or_else(|| error(range, "start required"))?,
                    scope,
                )?;
                let (end, ek) = self.value(
                    range
                        .end
                        .as_ref()
                        .ok_or_else(|| error(range, "end required"))?,
                    scope,
                )?;
                if sk != Kind::U64 || ek != Kind::U64 {
                    return Err(error(expr, "range bounds require u64"));
                }
                self.bind(ident)?;
                let mut inner = scope.clone();
                inner.insert(
                    ident.to_string(),
                    Binding {
                        kind: Kind::U64,
                        mutable: false,
                    },
                );
                let body = self.block(&f.body, &inner, true)?;
                Ok(
                    quote!(for #ident in (#start)..(#end) { __bream_context.checkpoint().await; #body }),
                )
            }
            Expr::If(i) if i.attrs.is_empty() => {
                let (condition, kind) = self.value(&i.cond, scope)?;
                if kind != Kind::Bool {
                    return Err(error(expr, "condition requires bool"));
                }
                let yes = self.block(&i.then_branch, scope, in_loop)?;
                let no = if let Some((_, other)) = &i.else_branch {
                    if !matches!(&**other, Expr::If(_) | Expr::Block(_)) {
                        return Err(error(other, "else requires block/if"));
                    }
                    let tokens = self.statement(other, scope, in_loop)?;
                    quote!(else { #tokens })
                } else {
                    quote!()
                };
                Ok(quote!(if #condition { #yes } #no))
            }
            Expr::Block(b) if b.attrs.is_empty() && b.label.is_none() => {
                let body = self.block(&b.block, scope, in_loop)?;
                Ok(quote!({ #body }))
            }
            Expr::Match(m) if m.attrs.is_empty() => {
                let (value, kind) = self.value(&m.expr, scope)?;
                if !matches!(
                    kind,
                    Kind::Tag(_)
                        | Kind::ReceiveResult
                        | Kind::SharedReceiveResult
                        | Kind::Event
                        | Kind::EventResult
                        | Kind::DownResult
                        | Kind::ExitResult
                        | Kind::LinkResult
                ) {
                    return Err(error(expr, "match requires a checked enum"));
                }
                let expected = kind;
                let mut arms = Vec::new();
                for arm in &m.arms {
                    if !arm.attrs.is_empty() || arm.guard.is_some() {
                        return Err(error(arm, "arm attributes/guards forbidden"));
                    }
                    let mut inner = scope.clone();
                    let pattern = match &arm.pat {
                        Pat::Wild(p) if p.attrs.is_empty() => quote!(_),
                        Pat::Path(p) if p.attrs.is_empty() && p.qself.is_none() => {
                            let (variant_kind, variant, payload) =
                                self.schema.pattern_variant(&p.path)?;
                            if variant_kind != expected || payload.is_some() {
                                return Err(error(p, "unit variant mismatch"));
                            }
                            quote!(#variant)
                        }
                        Pat::TupleStruct(p)
                            if p.attrs.is_empty() && p.qself.is_none() && p.elems.len() == 1 =>
                        {
                            let (variant_kind, variant, payload) =
                                self.schema.pattern_variant(&p.path)?;
                            let Some(kind) = payload else {
                                return Err(error(p, "variant has no payload"));
                            };
                            if variant_kind != expected {
                                return Err(error(p, "enum mismatch"));
                            }
                            let (ident, mutable) = ident_pattern(&p.elems[0])?;
                            self.bind(ident)?;
                            inner.insert(ident.to_string(), Binding { kind, mutable });
                            let modifier = if mutable { quote!(mut) } else { quote!() };
                            quote!(#variant(#modifier #ident))
                        }
                        _ => return Err(error(&arm.pat, "unsupported enum pattern")),
                    };
                    let body = match &*arm.body {
                        Expr::Block(body) if body.attrs.is_empty() && body.label.is_none() => {
                            self.block(&body.block, &inner, in_loop)?
                        }
                        // rustfmt removes the braces around a sole nested match.
                        // Reuse the complete checked lowering, including patterns,
                        // operand types, arm restrictions and generated checkpoints.
                        Expr::Match(_) => {
                            let nested = self.statement(&arm.body, &inner, in_loop)?;
                            quote!(__bream_context.checkpoint().await; #nested)
                        }
                        _ => {
                            return Err(error(
                                arm,
                                "match arm needs a checked block or nested match",
                            ))
                        }
                    };
                    arms.push(quote!(#pattern => { #body }));
                }
                Ok(quote!(match #value { #(#arms),* }))
            }
            Expr::Assign(a) if a.attrs.is_empty() => {
                let (target, binding) = self.place(&a.left, scope)?;
                let (value, kind) = self.value(&a.right, scope)?;
                if !binding.mutable || kind != binding.kind {
                    return Err(error(expr, "assignment type/mutability"));
                }
                Ok(quote!(#target = #value;))
            }
            Expr::Binary(b) if b.attrs.is_empty() && matches!(b.op, BinOp::AddAssign(_)) => {
                let (target, binding) = self.place(&b.left, scope)?;
                let (value, kind) = self.value(&b.right, scope)?;
                if !binding.mutable || binding.kind != Kind::U64 || kind != Kind::U64 {
                    return Err(error(expr, "+= requires mutable u64"));
                }
                Ok(quote!(#target = (#target).wrapping_add(#value);))
            }
            Expr::Return(r) if r.attrs.is_empty() => {
                let (value, kind) = if let Some(value) = &r.expr {
                    self.value(value, scope)?
                } else {
                    (quote!(()), Kind::Unit)
                };
                if kind != self.output {
                    return Err(error(expr, "return type mismatch"));
                }
                Ok(quote!(return #value;))
            }
            Expr::Break(b)
                if in_loop && b.attrs.is_empty() && b.label.is_none() && b.expr.is_none() =>
            {
                Ok(quote!(break;))
            }
            Expr::Continue(c) if in_loop && c.attrs.is_empty() && c.label.is_none() => {
                Ok(quote!(continue;))
            }
            Expr::Call(_) => {
                let (value, _) = self.value(expr, scope)?;
                Ok(quote!(#value;))
            }
            _ => Err(error(
                expr,
                "unsupported statement or unqualified execution",
            )),
        }
    }
}

fn parse_schema(module: &ItemMod) -> syn::Result<Schema> {
    if !module.attrs.is_empty()
        || !matches!(module.vis, syn::Visibility::Inherited)
        || module.unsafety.is_some()
    {
        return Err(error(
            module,
            "requires private inline module without attributes",
        ));
    }
    allowed_name(&module.ident)?;
    let (_, items) = module
        .content
        .as_ref()
        .ok_or_else(|| error(module, "inline module required"))?;
    let mut schema = Schema {
        data: Vec::new(),
        functions: BTreeMap::new(),
    };
    let mut names = BTreeSet::new();
    for item in items {
        let (ident, attrs, vis, generics, is_enum) = match item {
            Item::Struct(s) => (&s.ident, &s.attrs, &s.vis, &s.generics, false),
            Item::Enum(e) => (&e.ident, &e.attrs, &e.vis, &e.generics, true),
            Item::Fn(f) => {
                allowed_name(&f.sig.ident)?;
                if !names.insert(f.sig.ident.to_string()) {
                    return Err(error(f, "duplicate declaration"));
                }
                continue;
            }
            _ => {
                return Err(error(
                    item,
                    "only private checked structs/enums/functions; no impl/import/alias/macro",
                ))
            }
        };
        allowed_name(ident)?;
        if !attrs.is_empty()
            || !matches!(vis, syn::Visibility::Inherited)
            || !generics.params.is_empty()
            || generics.where_clause.is_some()
        {
            return Err(error(item, "data attributes/visibility/generics forbidden"));
        }
        if !names.insert(ident.to_string()) {
            return Err(error(item, "duplicate declaration"));
        }
        schema.data.push(Data {
            name: ident.clone(),
            fields: BTreeMap::new(),
            ordered: Vec::new(),
            variants: Vec::new(),
            is_enum,
        });
    }
    if schema.data.len() > 8 {
        return Err(error(module, "at most eight data types"));
    }
    for item in items {
        match item {
            Item::Struct(s) => {
                let i = schema.data.iter().position(|d| d.name == s.ident).unwrap();
                let syn::Fields::Named(fields) = &s.fields else {
                    return Err(error(s, "named record fields required"));
                };
                if fields.named.is_empty() || fields.named.len() > 8 {
                    return Err(error(s, "record needs one to eight fields"));
                }
                for field in &fields.named {
                    let ident = field.ident.as_ref().unwrap();
                    allowed_name(ident)?;
                    if !field.attrs.is_empty() || !matches!(field.vis, syn::Visibility::Inherited) {
                        return Err(error(field, "field attributes/visibility forbidden"));
                    }
                    let kind = schema.kind(&field.ty)?;
                    if schema.data[i]
                        .fields
                        .insert(ident.to_string(), kind)
                        .is_some()
                    {
                        return Err(error(field, "duplicate field"));
                    }
                    schema.data[i].ordered.push((ident.clone(), kind));
                }
            }
            Item::Enum(e) => {
                let i = schema.data.iter().position(|d| d.name == e.ident).unwrap();
                if e.variants.is_empty() || e.variants.len() > 8 {
                    return Err(error(e, "enum needs one to eight variants"));
                }
                let mut seen = BTreeSet::new();
                for variant in &e.variants {
                    allowed_name(&variant.ident)?;
                    if !variant.attrs.is_empty()
                        || variant.discriminant.is_some()
                        || !seen.insert(variant.ident.to_string())
                    {
                        return Err(error(
                            variant,
                            "variant attributes/discriminants/duplicates forbidden",
                        ));
                    }
                    let payload = match &variant.fields {
                        syn::Fields::Unit => None,
                        syn::Fields::Unnamed(f) if f.unnamed.len() == 1 => {
                            let field = &f.unnamed[0];
                            if !field.attrs.is_empty()
                                || !matches!(field.vis, syn::Visibility::Inherited)
                            {
                                return Err(error(
                                    field,
                                    "variant field attributes/visibility forbidden",
                                ));
                            }
                            let kind = schema.kind(&field.ty)?;
                            if !matches!(kind, Kind::U64 | Kind::Bool) {
                                return Err(error(field, "tag payload must be primitive"));
                            }
                            Some(kind)
                        }
                        _ => return Err(error(variant, "only unit/single-primitive variants")),
                    };
                    schema.data[i]
                        .variants
                        .push((variant.ident.clone(), payload));
                }
            }
            Item::Fn(f) => {
                let sig = &f.sig;
                if !f.attrs.is_empty()
                    || !matches!(f.vis, syn::Visibility::Inherited)
                    || sig.asyncness.is_some()
                    || sig.constness.is_some()
                    || sig.unsafety.is_some()
                    || sig.abi.is_some()
                    || sig.variadic.is_some()
                    || !sig.generics.params.is_empty()
                    || sig.generics.where_clause.is_some()
                {
                    return Err(error(
                        f,
                        "only private synchronous nongeneric checked functions",
                    ));
                }
                let mut args = Vec::new();
                for arg in &sig.inputs {
                    let syn::FnArg::Typed(arg) = arg else {
                        return Err(error(arg, "methods forbidden"));
                    };
                    if !arg.attrs.is_empty() {
                        return Err(error(arg, "argument attributes forbidden"));
                    }
                    let (ident, mutable) = ident_pattern(&arg.pat)?;
                    args.push((ident.clone(), mutable, schema.kind(&arg.ty)?));
                }
                if args.len() > 8 {
                    return Err(error(f, "at most eight owning parameters"));
                }
                let output = match &sig.output {
                    syn::ReturnType::Default => Kind::Unit,
                    syn::ReturnType::Type(_, ty) => schema.kind(ty)?,
                };
                schema.functions.insert(
                    sig.ident.to_string(),
                    Function {
                        item: f.clone(),
                        args,
                        output,
                    },
                );
            }
            _ => unreachable!(),
        }
    }
    let entry = schema
        .functions
        .get("run")
        .ok_or_else(|| error(module, "fn run() entry required"))?;
    if !entry.args.is_empty() || entry.output != Kind::Unit || schema.functions.len() > 16 {
        return Err(error(
            module,
            "unit entry with no arguments and at most sixteen functions",
        ));
    }
    // Finite record nesting bounds generated drop/poll bookkeeping in this profile.
    fn data_depth(schema: &Schema, i: usize, path: &mut Vec<usize>) -> syn::Result<(usize, usize)> {
        if path.contains(&i) {
            return Err(error(
                &schema.data[i].name,
                "recursive record types forbidden",
            ));
        }
        path.push(i);
        let mut depth = 1;
        let mut leaves = 0;
        for (_, kind) in &schema.data[i].ordered {
            if let Kind::Record(child) = kind {
                let (d, n) = data_depth(schema, *child, path)?;
                depth = depth.max(d + 1);
                leaves += n;
            } else {
                leaves += 1;
            }
        }
        path.pop();
        if depth > 4 || leaves > 32 {
            return Err(error(
                &schema.data[i].name,
                "record depth/leaf bound exceeded",
            ));
        }
        Ok((depth, leaves))
    }
    for i in 0..schema.data.len() {
        data_depth(&schema, i, &mut Vec::new())?;
    }
    Ok(schema)
}

pub fn expand(arguments: TokenStream, input: TokenStream) -> TokenStream {
    if !arguments.is_empty() {
        return error(
            proc_macro2::Span::call_site(),
            "attribute takes no arguments",
        )
        .to_compile_error()
        .into();
    }
    let module = syn::parse_macro_input!(input as ItemMod);
    let result = (|| {
        let schema = parse_schema(&module)?;
        let mut bodies = BTreeMap::new();
        let mut graph = BTreeMap::new();
        for (name, function) in &schema.functions {
            let mut lower = Lowerer {
                schema: &schema,
                names: BTreeSet::new(),
                calls: BTreeSet::new(),
                output: function.output,
            };
            let mut scope = Scope::new();
            for (ident, mutable, kind) in &function.args {
                lower.bind(ident)?;
                scope.insert(
                    ident.to_string(),
                    Binding {
                        kind: *kind,
                        mutable: *mutable,
                    },
                );
            }
            bodies.insert(
                name.clone(),
                lower.block(&function.item.block, &scope, false)?,
            );
            graph.insert(name.clone(), lower.calls);
        }
        fn depth(
            name: &str,
            graph: &BTreeMap<String, BTreeSet<String>>,
            path: &mut Vec<String>,
        ) -> Result<usize, &'static str> {
            if path.iter().any(|n| n == name) {
                return Err("recursive helper/spawn cycle forbidden");
            }
            path.push(name.to_owned());
            let mut value = 1;
            for child in &graph[name] {
                value = value.max(1 + depth(child, graph, path)?);
            }
            path.pop();
            if value > 8 {
                return Err("static helper/spawn depth exceeds eight functions");
            }
            Ok(value)
        }
        for name in graph.keys() {
            depth(name, &graph, &mut Vec::new()).map_err(|e| error(&module, e))?;
        }
        let mut definitions = Vec::new();
        for data in &schema.data {
            let name = &data.name;
            if data.is_enum {
                let variants: Vec<_> = data
                    .variants
                    .iter()
                    .map(|(ident, kind)| {
                        if let Some(kind) = kind {
                            let ty = schema.tokens(*kind);
                            quote!(#ident(#ty))
                        } else {
                            quote!(#ident)
                        }
                    })
                    .collect();
                definitions.push(quote!(enum #name { #(#variants),* }));
            } else {
                let fields: Vec<_> = data
                    .ordered
                    .iter()
                    .map(|(ident, kind)| {
                        let ty = schema.tokens(*kind);
                        quote!(#ident: #ty)
                    })
                    .collect();
                definitions.push(quote!(struct #name { #(#fields),* }));
            }
        }
        let mut helpers = Vec::new();
        for (name, function) in &schema.functions {
            if name == "run" {
                continue;
            }
            let target = format_ident!("__bream_helper_{}", name);
            let args: Vec<_> = function
                .args
                .iter()
                .map(|(ident, mutable, kind)| {
                    let ty = schema.tokens(*kind);
                    let modifier = if *mutable { quote!(mut) } else { quote!() };
                    quote!(#modifier #ident: #ty)
                })
                .collect();
            let output = schema.tokens(function.output);
            let body = &bodies[name];
            helpers.push(quote!(async fn #target(#(#args,)* __bream_context: &mut __bream_runtime::ProcessContext) -> #output {
                __bream_context.checkpoint().await; #body
            }));
        }
        // Factories contain only generated code and capture no authored state.
        // Child inputs move through the runtime boundary; every child receives
        // its own context and starts at the checked function's checkpoints.
        let mut child_factories = Vec::new();
        for (child_name, function) in &schema.functions {
            if !function.args.is_empty() || function.output != Kind::Unit {
                continue;
            }
            let factory = format_ident!("__bream_child_{}", child_name);
            let body = &bodies[child_name];
            child_factories.push(quote!(fn #factory() -> __bream_runtime::Program {
                unsafe { __bream_runtime::Program::__from_generated(|mut __bream_owned_context| {
                    __bream_std::boxed::Box::pin(async move {
                        let __bream_context = &mut __bream_owned_context;
                        __bream_context.checkpoint().await;
                        #body
                    })
                }) }
            }));
        }
        let name = &module.ident;
        let entry = &bodies["run"];
        Ok::<Tokens, syn::Error>(quote!(mod #name {
            extern crate core as __bream_core;
            extern crate std as __bream_std;
            extern crate bream_shared_runtime as __bream_runtime;
            #(#definitions)* #(#helpers)* #(#child_factories)*
            pub(super) fn program() -> __bream_runtime::Program {
                unsafe { __bream_runtime::Program::__from_generated(|mut __bream_owned_context| {
                    __bream_std::boxed::Box::pin(async move {
                        let __bream_context = &mut __bream_owned_context;
                        #entry
                    })
                }) }
            }
        }))
    })();
    result.unwrap_or_else(syn::Error::into_compile_error).into()
}
