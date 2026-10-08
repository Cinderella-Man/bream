//! Research subset. Not a validator for arbitrary Rust, dependencies, or Drop.
use proc_macro::TokenStream;
use proc_macro2::TokenStream as Tokens;
use quote::quote;
use std::collections::{BTreeMap, BTreeSet};
use syn::{spanned::Spanned, BinOp, Expr, ItemFn, Lit, Pat, Stmt};

mod structured;

#[proc_macro_attribute]
pub fn process_module(arguments: TokenStream, input: TokenStream) -> TokenStream {
    structured::expand(arguments, input)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    U64,
    Bool,
    Buffer,
}
#[derive(Clone, Copy)]
struct Binding {
    kind: Kind,
    mutable: bool,
}
type Scope = BTreeMap<String, Binding>;

#[derive(Default)]
struct Lowerer {
    // No shadowing anywhere in one process, making this research checker small.
    names: BTreeSet<String>,
}

fn rejected(node: impl Spanned, reason: &str) -> syn::Error {
    syn::Error::new(node.span(), format!("bream milestone subset: {reason}"))
}

fn type_tokens(kind: Kind) -> Tokens {
    match kind {
        Kind::U64 => quote!(__bream_core::primitive::u64),
        Kind::Bool => quote!(__bream_core::primitive::bool),
        Kind::Buffer => quote!(__bream_runtime::OwnedWords),
    }
}

fn simple_name(path: &syn::ExprPath) -> Option<String> {
    if path.qself.is_none()
        && path.path.leading_colon.is_none()
        && path.path.segments.len() == 1
        && matches!(path.path.segments[0].arguments, syn::PathArguments::None)
    {
        Some(path.path.segments[0].ident.to_string())
    } else {
        None
    }
}

impl Lowerer {
    fn bind(&mut self, ident: &syn::Ident) -> syn::Result<()> {
        let name = ident.to_string();
        if name.starts_with("r#") {
            return Err(rejected(
                ident,
                "raw identifiers are forbidden in this milestone subset",
            ));
        }
        if name.starts_with("__bream_")
            || ["observe", "result", "owned_input", "read", "write", "len"].contains(&name.as_str())
        {
            return Err(rejected(ident, "reserved binding name"));
        }
        if !self.names.insert(name) {
            return Err(rejected(ident, "shadowed or repeated binding"));
        }
        Ok(())
    }

    fn value(&self, expr: &Expr, scope: &Scope) -> syn::Result<(Tokens, Kind)> {
        let result = match expr {
            Expr::Lit(lit) if lit.attrs.is_empty() => match &lit.lit {
                Lit::Int(n) if n.suffix().is_empty() || n.suffix() == "u64" => {
                    let n = n.base10_parse::<u64>()?;
                    (quote!(#n), Kind::U64)
                }
                Lit::Bool(b) => (quote!(#b), Kind::Bool),
                _ => return Err(rejected(expr, "only u64 and bool literals are supported")),
            },
            Expr::Path(path) if path.attrs.is_empty() => {
                let name = simple_name(path).ok_or_else(|| rejected(expr, "external paths are forbidden"))?;
                let binding = scope.get(&name).ok_or_else(|| rejected(expr, "only local primitive bindings may be read"))?;
                if binding.kind == Kind::Buffer {
                    return Err(rejected(expr, "owned buffers cannot be read, moved, borrowed, or passed by user code"));
                }
                (quote!(#path), binding.kind)
            }
            Expr::Paren(paren) if paren.attrs.is_empty() => {
                let (value, kind) = self.value(&paren.expr, scope)?;
                (quote!((#value)), kind)
            }
            Expr::Binary(binary) if binary.attrs.is_empty() => {
                let (left, lk) = self.value(&binary.left, scope)?;
                let (right, rk) = self.value(&binary.right, scope)?;
                let op = &binary.op;
                match op {
                    BinOp::Add(_) | BinOp::Sub(_) | BinOp::Mul(_) if lk == Kind::U64 && rk == Kind::U64 => {
                        let method = match op {
                            BinOp::Add(_) => quote!(wrapping_add),
                            BinOp::Sub(_) => quote!(wrapping_sub),
                            _ => quote!(wrapping_mul),
                        };
                        (quote!((#left).#method(#right)), Kind::U64)
                    }
                    BinOp::BitXor(_) | BinOp::BitAnd(_) | BinOp::BitOr(_) if lk == Kind::U64 && rk == Kind::U64 =>
                        (quote!((#left) #op (#right)), Kind::U64),
                    BinOp::Lt(_) | BinOp::Le(_) | BinOp::Gt(_) | BinOp::Ge(_) if lk == Kind::U64 && rk == Kind::U64 =>
                        (quote!((#left) #op (#right)), Kind::Bool),
                    BinOp::Eq(_) | BinOp::Ne(_) if lk == rk =>
                        (quote!((#left) #op (#right)), Kind::Bool),
                    BinOp::And(_) | BinOp::Or(_) if lk == Kind::Bool && rk == Kind::Bool =>
                        (quote!((#left) #op (#right)), Kind::Bool),
                    _ => return Err(rejected(expr, "unsupported primitive operator or operand types")),
                }
            }
            Expr::Call(call) if call.attrs.is_empty() => {
                let Expr::Path(path) = &*call.func else { return Err(rejected(expr, "invalid builtin")); };
                let name = simple_name(path).ok_or_else(|| rejected(expr, "external calls forbidden"))?;
                if !path.attrs.is_empty() { return Err(rejected(expr, "attributes forbidden")); }
                if name == "len" && call.args.len() == 1 {
                    let buffer = self.buffer(&call.args[0], scope, false)?;
                    (quote!(#buffer.len()), Kind::U64)
                } else if name == "read" && call.args.len() == 2 {
                    let buffer = self.buffer(&call.args[0], scope, false)?;
                    let (index, kind) = self.value(&call.args[1], scope)?;
                    if kind != Kind::U64 { return Err(rejected(expr, "index requires u64")); }
                    (quote!(#buffer.read(#index)), Kind::U64)
                } else { return Err(rejected(expr, "only concrete read/len value builtins are supported")); }
            }
            Expr::Unary(unary) if unary.attrs.is_empty() && matches!(unary.op, syn::UnOp::Not(_)) => {
                let (value, kind) = self.value(&unary.expr, scope)?;
                (quote!(!(#value)), kind)
            }
            _ => return Err(rejected(expr, "unsupported expression; arbitrary calls, methods, macros, references, unsafe, closures and custom types are forbidden")),
        };
        Ok(result)
    }

    fn initializer(&self, expr: &Expr, scope: &Scope) -> syn::Result<(Tokens, Kind)> {
        if let Expr::Call(call) = expr {
            if !call.attrs.is_empty() {
                return Err(rejected(expr, "attributes are forbidden"));
            }
            if let Expr::Path(path) = &*call.func {
                if path.attrs.is_empty()
                    && simple_name(path).as_deref() == Some("owned_input")
                    && call.args.is_empty()
                {
                    return Ok((quote!(__bream_context.owned_input()), Kind::Buffer));
                }
            }
        }
        self.value(expr, scope)
    }

    fn buffer(&self, expr: &Expr, scope: &Scope, mutable: bool) -> syn::Result<Tokens> {
        let Expr::Path(path) = expr else {
            return Err(rejected(expr, "buffer requires a direct owning local"));
        };
        let name = simple_name(path).ok_or_else(|| rejected(expr, "external buffer forbidden"))?;
        let binding = scope
            .get(&name)
            .ok_or_else(|| rejected(expr, "unknown buffer"))?;
        if !path.attrs.is_empty() || binding.kind != Kind::Buffer || (mutable && !binding.mutable) {
            return Err(rejected(
                expr,
                "builtin requires an owned buffer with matching mutability",
            ));
        }
        Ok(quote!(#path))
    }

    fn block(&mut self, block: &syn::Block, parent: &Scope, in_loop: bool) -> syn::Result<Tokens> {
        let mut scope = parent.clone();
        let mut out = Tokens::new();
        for statement in &block.stmts {
            out.extend(quote!(__bream_context.checkpoint().await;));
            match statement {
                Stmt::Local(local) => {
                    if !local.attrs.is_empty() {
                        return Err(rejected(local, "attributes are forbidden"));
                    }
                    let init = local
                        .init
                        .as_ref()
                        .ok_or_else(|| rejected(local, "locals require initialization"))?;
                    if init.diverge.is_some() {
                        return Err(rejected(local, "let-else is forbidden"));
                    }
                    let (pattern, annotation) = match &local.pat {
                        Pat::Type(typed) if typed.attrs.is_empty() => {
                            (&*typed.pat, Some(&*typed.ty))
                        }
                        other => (other, None),
                    };
                    let Pat::Ident(binding) = pattern else {
                        return Err(rejected(local, "only simple local bindings are supported"));
                    };
                    if binding.by_ref.is_some()
                        || binding.subpat.is_some()
                        || !binding.attrs.is_empty()
                    {
                        return Err(rejected(
                            binding,
                            "binding references, patterns and attributes are forbidden",
                        ));
                    }
                    let (value, kind) = self.initializer(&init.expr, &scope)?;
                    if let Some(annotation) = annotation {
                        let expected = match kind {
                            Kind::U64 => "u64",
                            Kind::Bool => "bool",
                            Kind::Buffer => {
                                return Err(rejected(
                                    annotation,
                                    "buffer type is runtime-owned and inferred",
                                ))
                            }
                        };
                        if !matches!(annotation, syn::Type::Path(p) if p.qself.is_none() && p.path.is_ident(expected))
                        {
                            return Err(rejected(
                                annotation,
                                "only the matching primitive type annotation is supported",
                            ));
                        }
                    }
                    self.bind(&binding.ident)?;
                    let ident = &binding.ident;
                    let mutable = &binding.mutability;
                    let ty = type_tokens(kind);
                    out.extend(quote!(let #mutable #ident: #ty = #value;));
                    scope.insert(
                        ident.to_string(),
                        Binding {
                            kind,
                            mutable: mutable.is_some(),
                        },
                    );
                }
                Stmt::Expr(expr, _) => out.extend(self.statement(expr, &scope, in_loop)?),
                _ => {
                    return Err(rejected(
                        statement,
                        "items and statement macros are forbidden",
                    ))
                }
            }
        }
        Ok(out)
    }

    fn target(&self, expr: &Expr, scope: &Scope) -> syn::Result<(Tokens, Kind)> {
        let Expr::Path(path) = expr else {
            return Err(rejected(
                expr,
                "assignment must target a mutable primitive local",
            ));
        };
        if !path.attrs.is_empty() {
            return Err(rejected(expr, "assignment target attributes are forbidden"));
        }
        let name = simple_name(path).ok_or_else(|| rejected(expr, "external assignment target"))?;
        let binding = scope
            .get(&name)
            .ok_or_else(|| rejected(expr, "unknown assignment target"))?;
        if !binding.mutable || binding.kind == Kind::Buffer {
            return Err(rejected(
                expr,
                "assignment requires a mutable primitive local",
            ));
        }
        Ok((quote!(#path), binding.kind))
    }

    fn statement(&mut self, expr: &Expr, scope: &Scope, in_loop: bool) -> syn::Result<Tokens> {
        match expr {
            Expr::Loop(repeat) if repeat.attrs.is_empty() && repeat.label.is_none() => {
                let body = self.block(&repeat.body, scope, true)?;
                Ok(quote!(loop { __bream_context.checkpoint().await; #body }))
            }
            Expr::While(repeat) if repeat.attrs.is_empty() && repeat.label.is_none() => {
                let (condition, kind) = self.value(&repeat.cond, scope)?;
                if kind != Kind::Bool { return Err(rejected(expr, "while condition must be bool")); }
                let body = self.block(&repeat.body, scope, true)?;
                Ok(quote!(loop { __bream_context.checkpoint().await; if !(#condition) { break; } #body }))
            }
            Expr::ForLoop(repeat) if repeat.attrs.is_empty() && repeat.label.is_none() => {
                let Pat::Ident(binding) = &*repeat.pat else { return Err(rejected(expr, "for requires one primitive binding")); };
                if binding.mutability.is_some() || binding.by_ref.is_some() || binding.subpat.is_some() || !binding.attrs.is_empty() { return Err(rejected(binding, "unsupported for binding")); }
                let Expr::Range(range) = &*repeat.expr else { return Err(rejected(expr, "for supports only an exclusive u64 range")); };
                if !range.attrs.is_empty() || !matches!(range.limits, syn::RangeLimits::HalfOpen(_)) { return Err(rejected(range, "only exclusive ranges are supported")); }
                let start = range.start.as_ref().ok_or_else(|| rejected(range, "range start required"))?;
                let end = range.end.as_ref().ok_or_else(|| rejected(range, "range end required"))?;
                let (start, sk) = self.value(start, scope)?;
                let (end, ek) = self.value(end, scope)?;
                if sk != Kind::U64 || ek != Kind::U64 { return Err(rejected(range, "range bounds must be u64")); }
                self.bind(&binding.ident)?;
                let ident = &binding.ident;
                let mut nested = scope.clone();
                nested.insert(ident.to_string(), Binding {kind: Kind::U64, mutable: false});
                let body = self.block(&repeat.body, &nested, true)?;
                Ok(quote!(for #ident in (#start)..(#end) { __bream_context.checkpoint().await; #body }))
            }
            Expr::If(branch) if branch.attrs.is_empty() => {
                let (condition, kind) = self.value(&branch.cond, scope)?;
                if kind != Kind::Bool { return Err(rejected(expr, "if condition must be bool")); }
                let yes = self.block(&branch.then_branch, scope, in_loop)?;
                let no = if let Some((_, other)) = &branch.else_branch {
                    match &**other {
                        Expr::If(_) | Expr::Block(_) => { let no = self.statement(other, scope, in_loop)?; quote!(else { #no }) },
                        _ => return Err(rejected(other, "else requires a block or if")),
                    }
                } else { Tokens::new() };
                Ok(quote!(if #condition { #yes } #no))
            }
            Expr::Block(block) if block.attrs.is_empty() && block.label.is_none() => {
                let body = self.block(&block.block, scope, in_loop)?;
                Ok(quote!({ #body }))
            }
            Expr::Break(b) if in_loop && b.attrs.is_empty() && b.label.is_none() && b.expr.is_none() => Ok(quote!(break;)),
            Expr::Continue(c) if in_loop && c.attrs.is_empty() && c.label.is_none() => Ok(quote!(continue;)),
            Expr::Assign(assign) if assign.attrs.is_empty() => {
                let (left, lk) = self.target(&assign.left, scope)?;
                let (right, rk) = self.value(&assign.right, scope)?;
                if lk != rk { return Err(rejected(expr, "assignment changes primitive type")); }
                Ok(quote!(#left = #right;))
            }
            Expr::Binary(assign) if assign.attrs.is_empty() && matches!(assign.op, BinOp::AddAssign(_)) => {
                let (left, lk) = self.target(&assign.left, scope)?;
                let (right, rk) = self.value(&assign.right, scope)?;
                if lk != Kind::U64 || rk != Kind::U64 { return Err(rejected(expr, "+= requires u64 locals")); }
                Ok(quote!(#left = (#left).wrapping_add(#right);))
            }
            Expr::Call(call) if call.attrs.is_empty() => {
                if let Expr::Path(path) = &*call.func {
                    if path.attrs.is_empty() && [Some("observe"), Some("result")].contains(&simple_name(path).as_deref()) && call.args.len() == 1 {
                        let (value, kind) = self.value(&call.args[0], scope)?;
                        if kind != Kind::U64 { return Err(rejected(expr, "observe requires u64")); }
                        let method = &path.path.segments[0].ident;
                        return Ok(quote!(__bream_context.#method(#value);));
                    }
                    if path.attrs.is_empty() && simple_name(path).as_deref() == Some("write") && call.args.len() == 3 {
                        let buffer = self.buffer(&call.args[0], scope, true)?;
                        let (index, ik) = self.value(&call.args[1], scope)?;
                        let (value, vk) = self.value(&call.args[2], scope)?;
                        if ik != Kind::U64 || vk != Kind::U64 { return Err(rejected(expr, "write requires u64 index/value")); }
                        return Ok(quote!(#buffer.write(#index, #value);));
                    }
                }
                Err(rejected(expr, "arbitrary calls are forbidden; only observe(u64) is an approved statement builtin"))
            }
            _ => Err(rejected(expr, "unsupported statement; arbitrary calls, macros, unsafe, closures, references, explicit await and custom types are forbidden")),
        }
    }
}

#[proc_macro_attribute]
pub fn process(arguments: TokenStream, input: TokenStream) -> TokenStream {
    if !arguments.is_empty() {
        return syn::Error::new(
            proc_macro2::Span::call_site(),
            "bream milestone subset: attribute takes no arguments",
        )
        .to_compile_error()
        .into();
    }
    let function = syn::parse_macro_input!(input as ItemFn);
    let result = (|| {
        let signature = &function.sig;
        if !function.attrs.is_empty()
            || !matches!(function.vis, syn::Visibility::Inherited)
            || signature.asyncness.is_some()
            || signature.constness.is_some()
            || signature.unsafety.is_some()
            || signature.abi.is_some()
            || signature.variadic.is_some()
            || !signature.generics.params.is_empty()
            || signature.generics.where_clause.is_some()
            || !signature.inputs.is_empty()
            || !matches!(signature.output, syn::ReturnType::Default)
            || signature.ident.to_string().starts_with("__bream_")
            || signature.ident.to_string().starts_with("r#")
        {
            return Err(rejected(&function, "requires a private, synchronous, nongeneric fn with no arguments or return annotation"));
        }
        let body = Lowerer::default().block(&function.block, &Scope::new(), false)?;
        let name = &signature.ident;
        let module = syn::Ident::new(&format!("__bream_generated_{}", name), name.span());
        Ok(quote! {
            mod #module {
                extern crate core as __bream_core;
                extern crate std as __bream_std;
                extern crate bream_spawn_runtime as __bream_runtime;
                pub(super) fn #name() -> __bream_runtime::Program {
                    // The whole body is reconstructed from the checked grammar.
                    unsafe {
                        __bream_runtime::Program::__from_generated(|mut __bream_context| {
                            __bream_std::boxed::Box::pin(async move { #body })
                        })
                    }
                }
            }
            use #module::#name;
        })
    })();
    result.unwrap_or_else(syn::Error::into_compile_error).into()
}
