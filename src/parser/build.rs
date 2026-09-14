//! Tree-building helpers the grammar's actions call.  [frozen — do not edit]
//!
//! Everything here is a small pure function from parsed pieces to AST nodes,
//! including the surface resolutions the AST's header note lists.

use super::{LalrpopError, ParseError};
use crate::ast::*;
use std::collections::HashMap;

pub(super) fn sp(src: SrcId, start: usize, end: usize) -> Span {
    Span { src, start, end }
}

pub(super) fn bx(e: Expr) -> Box<Expr> {
    Box::new(e)
}

/// The end of a production whose last piece is optional: `after` when the
/// piece was present, else `before` (see the note in the grammar).
pub(super) fn end(present: bool, before: usize, after: usize) -> usize {
    if present {
        after
    } else {
        before
    }
}

/// Lift a grammar-level check failure into LALRPOP's error channel.
pub(super) fn user(error: ParseError) -> LalrpopError {
    LalrpopError::User { error }
}

fn invalid(message: impl Into<String>, span: Span) -> ParseError {
    ParseError::Invalid {
        message: message.into(),
        span,
    }
}

pub(super) fn binary(op: BinOp, l: Expr, r: Expr, span: Span) -> Expr {
    Expr::Binary(op, bx(l), bx(r), span)
}

/// A prefix operator. A negative literal (`-1`) never reaches here: the
/// lexer reads it as one token (Appendix B's `int_lit ::= [ '-' ] ( '0' | pos_int )`);
/// `- 1`, with a space, is negation applied to `1`.
pub(super) fn unary(op: UnOp, e: Expr, span: Span) -> Expr {
    Expr::Unary(op, bx(e), span)
}

/// `ref x = e` is `let x = ref e`; the annotation `x: T` then means `ref<T>`.
pub(super) fn ref_of(e: Expr, span: Span) -> Expr {
    Expr::Unary(UnOp::Ref, bx(e), span)
}

/// The annotation on `ref x: T = e` is `T`; the binding's type is `ref<T>`.
pub(super) fn ref_ty(t: Ty) -> Ty {
    let span = t.span;
    Ty {
        kind: TyKind::Ref(std::rc::Rc::new(t)),
        span,
    }
}

/// `e.i`: the index is a literal and must fit a `u32`.
pub(super) fn proj(e: Expr, i: i64, span: Span) -> Result<Expr, ParseError> {
    match u32::try_from(i) {
        Ok(i) => Ok(Expr::Proj(bx(e), i, span)),
        Err(_) => Err(invalid(format!("`.{i}` is not a valid tuple index"), span)),
    }
}

/// `f(a, …)`: a call — unless an argument is a hole, in which case it is a
/// relation query and the callee must be a plain relation name.
pub(super) fn call_or_query(callee: Expr, args: Vec<QArg>, span: Span) -> Result<Expr, ParseError> {
    // `solutions(r(?x))`: a call to a function named `solutions` with a query
    // as its argument — the keyword form was meant, and it takes no parens.
    if let Expr::Var(name, _) = &callee {
        if name == "solutions"
            && args.len() == 1
            && matches!(&args[0], QArg::Expr(Expr::Relation(Rel::Query(_), _)))
        {
            return Err(invalid(
                "`solutions` is followed by its query, without parentheses: `solutions r(?x)`",
                span,
            ));
        }
    }
    if args.iter().any(|a| matches!(a, QArg::Hole(_))) {
        match callee {
            Expr::Var(name, _) => Ok(Expr::Relation(Rel::Query(Query { name, args }), span)),
            _ => Err(invalid(
                "a query hole `?` may only appear in the arguments of a relation",
                span,
            )),
        }
    } else {
        let args = args
            .into_iter()
            .map(|a| match a {
                QArg::Expr(e) => e,
                QArg::Hole(_) => unreachable!(),
            })
            .collect();
        Ok(Expr::Call(bx(callee), args, span))
    }
}

/// One element of a block's interior, before the trailing-value rule is applied.
pub(super) enum Item {
    Stmt(Stmt),
    /// A braced form (`if`, `match`, `while`, `for`, block) with no `;` after it.
    Braced(Expr),
}

/// Assemble a block. A braced form that ends the block with no `;` after it is
/// the block's value, as in Rust; elsewhere it is a statement.
pub(super) fn block(items: Vec<Item>, tail: Option<Expr>, span: Span) -> Expr {
    let mut stmts = Vec::with_capacity(items.len());
    let mut tail = tail;
    let n = items.len();
    for (i, item) in items.into_iter().enumerate() {
        match item {
            Item::Stmt(s) => stmts.push(s),
            Item::Braced(e) if i + 1 == n && tail.is_none() => tail = Some(e),
            Item::Braced(e) => stmts.push(Stmt::Expr(e)),
        }
    }
    Expr::Block(stmts, tail.map(Box::new), span)
}

pub(super) fn named_ty(name: String, span: Span, args: Vec<Ty>) -> Result<Ty, ParseError> {
    let kind = match (name.as_str(), args.is_empty()) {
        ("Int", true) => TyKind::Int,
        ("Bool", true) => TyKind::Bool,
        ("String", true) => TyKind::Str,
        // Appendix B gives the built-in names no argument list.
        ("Int" | "Bool" | "String", false) => {
            return Err(invalid(format!("`{name}` takes no type arguments"), span))
        }
        _ => TyKind::Named(name, std::rc::Rc::from(args)),
    };
    Ok(Ty::at(kind, span))
}

/// A top-level definition, before the program's tables are assembled.
pub(super) enum DefItem {
    /// a named declaration: its name, the name's span, the whole span
    Decl(Name, Span, Span, Decl),
    Impl(Impl),
    Rule(Rule),
}

/// `impl [Tr for] T { … }`: the head parses as a type; with `for` it is the
/// trait, so it must be a plain (possibly applied) name.
pub(super) fn impl_def(
    generics: Generics,
    head: Ty,
    for_ty: Option<Ty>,
    methods: Vec<Method>,
    span: Span,
    head_span: Span,
) -> Result<DefItem, ParseError> {
    let (trait_, ty) = match for_ty {
        None => (None, head),
        Some(ty) => match head.kind {
            TyKind::Named(name, args) => (
                Some(TraitRef {
                    name,
                    args: args.to_vec(),
                    span: head.span.unwrap_or(head_span),
                }),
                ty,
            ),
            _ => {
                return Err(invalid(
                    "`impl Tr for T`: `Tr` must be a trait name",
                    head.span.unwrap_or(head_span),
                ))
            }
        },
    };
    Ok(DefItem::Impl(Impl {
        generics,
        trait_,
        ty,
        methods,
        span,
        head: head_span,
    }))
}

/// A rule body is one `and`-chain, one conjunct per link.
pub(super) fn rule_def(head: Atom, body: Option<Expr>, span: Span) -> Result<Rule, ParseError> {
    let mut conjuncts = Vec::new();
    if let Some(body) = body {
        flatten(body, &mut conjuncts)?;
    }
    Ok(Rule {
        head,
        body: conjuncts,
        span,
    })
}

fn flatten(e: Expr, out: &mut Vec<Expr>) -> Result<(), ParseError> {
    match e {
        Expr::Binary(BinOp::And, l, r, _) => {
            flatten(*l, out)?;
            flatten(*r, out)
        }
        Expr::Relation(Rel::Query(_), span) => Err(invalid(
            "a rule body names logic variables directly; holes `?x` belong in queries",
            span,
        )),
        other => {
            out.push(other);
            Ok(())
        }
    }
}

/// The names in one list — parameters, fields, generics, a trait's methods —
/// must be distinct; `what` names the kind for the message, which blames the
/// second occurrence.
fn distinct<'a>(
    names: impl IntoIterator<Item = (&'a Name, Span)>,
    what: &str,
) -> Result<(), ParseError> {
    let mut seen = std::collections::HashSet::new();
    for (n, span) in names {
        if !seen.insert(n) {
            return Err(invalid(
                format!("{what} `{n}` is named more than once"),
                span,
            ));
        }
    }
    Ok(())
}

/// A lambda's parameters are distinct.
pub(super) fn lambda(params: Vec<LambdaParam>, body: Expr, span: Span) -> Result<Expr, ParseError> {
    distinct(params.iter().map(|p| (&p.name, p.span)), "parameter")?;
    Ok(Expr::Lambda(params, bx(body), span))
}

/// The type parameters of a declaration are distinct.
fn distinct_generics(generics: &Generics) -> Result<(), ParseError> {
    distinct(
        generics.params.iter().map(|p| (&p.name, p.span)),
        "type parameter",
    )
}

/// The names within one declaration are distinct: its type parameters, a
/// function's or method's parameters, a struct's fields, a trait's methods.
fn distinct_within(decl: &Decl) -> Result<(), ParseError> {
    match decl {
        Decl::Fn(generics, params, _, _) => {
            distinct_generics(generics)?;
            distinct(params.iter().map(|p| (&p.name, p.span)), "parameter")
        }
        Decl::Type(generics, _) => distinct_generics(generics),
        Decl::Struct(generics, fields) => {
            distinct_generics(generics)?;
            distinct(fields.iter().map(|f| (&f.name, f.span)), "field")
        }
        Decl::Trait(generics, sigs) => {
            distinct_generics(generics)?;
            distinct(sigs.iter().map(|s| (&s.name, s.span)), "method")?;
            for sig in sigs {
                distinct(sig.params.iter().map(|p| (&p.name, p.span)), "parameter")?;
            }
            Ok(())
        }
        Decl::Global(..) | Decl::Relation(..) => Ok(()),
    }
}

/// Assemble the program's tables; a repeated top-level name is an error, and
/// so is a repeated name within a declaration.
pub(super) fn program(defs: Vec<DefItem>) -> Result<Program, ParseError> {
    let mut decls = HashMap::new();
    let mut spans = HashMap::new();
    let mut impls = Vec::new();
    let mut rules = Vec::new();
    for def in defs {
        match def {
            DefItem::Decl(name, name_span, span, decl) => {
                if decls.contains_key(&name) {
                    return Err(invalid(
                        format!("`{name}` is defined more than once"),
                        name_span,
                    ));
                }
                distinct_within(&decl)?;
                spans.insert(name.clone(), span);
                decls.insert(name, decl);
            }
            DefItem::Impl(i) => {
                distinct_generics(&i.generics)?;
                for m in &i.methods {
                    distinct(m.params.iter().map(|p| (&p.name, p.span)), "parameter")?;
                }
                impls.push(i);
            }
            DefItem::Rule(r) => rules.push(r),
        }
    }
    Ok(Program {
        decls,
        spans,
        impls,
        rules,
    })
}
