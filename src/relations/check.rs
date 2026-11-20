//! Static checks on rules — YOUR file.  [student]  (Milestones M6, M7)
//!
//! A rule body is a list of expressions (see [`Rule`]); the parser has not
//! sorted them into generators and filters, and it could not have. These
//! checks are run by `Interpreter::check` (from M6), after the program is
//! loaded and before `main` runs, and together make the engine in `engine.rs`
//! safe to run: after them, every generator names a relation over variables
//! and literals, every variable a filter or the head uses is bound by a
//! generator, and every filter is pure, so it can be evaluated any number of
//! times in any order.
//!
//! Each check is its own function, called once per rule by the provided
//! driver (`check_rules` in `relations/mod.rs`); [`check_guards`] is a
//! separate M7 check over the whole program. Everything else in this file —
//! how you compute the impure functions, the rule's scope, a witness path — is
//! yours to shape; the driver calls only the four functions below.
//!
//! To list a milestone's holes:
//!
//! ```text
//! grep -rn 'todo_m6!' src
//! ```

use super::RuleError;
use crate::ast::{Decl, Expr, Name, Program, Rel, Rule, Span, Term, Ty, TyKind, UnOp};
use crate::interp::prelude::native_names;
use helpers::{all_subexprs, bound_names, free_vars};
use std::collections::{HashMap, HashSet};

/// The impure prelude primitives, by name: `print`, `to_string`, and the
/// `read_*` family read or write the outside world, so a call to one is an
/// effect wherever it appears.
const IMPURE_NATIVES: &[&str] = &[
    "print",
    "println",
    "to_string",
    "read_int",
    "read_bool",
    "read_line",
    "read_int_opt",
    "read_bool_opt",
    "read_line_opt",
];

fn is_native(name: &str) -> bool {
    native_names().any(|n| n == name)
}

/// Which conjuncts of `rule` are generators: a call whose callee names a
/// relation declared in `program`. Everything else is a filter. A generator's
/// arguments must be variables or literals (`ArgumentNotATerm`), and its
/// count must match the relation's declared arity (`Arity`); the head must
/// name a declared relation of the right arity too. A bare conjunct whose
/// callee names neither a declaration nor a native is a call on something
/// that is not a relation (`NotARelation`).
pub fn check_generators(program: &Program, rule: &Rule) -> Result<Vec<bool>, RuleError> {
    match program.decls.get(&rule.head.name) {
        Some(Decl::Relation(cols)) => {
            let cols = cols.clone();
            if rule.head.terms.len() != cols.len() {
                return Err(RuleError::Arity {
                    name: rule.head.name.clone(),
                    expected: cols.len(),
                    found: rule.head.terms.len(),
                    span: rule.head.span,
                });
            }
            let mut gens = Vec::with_capacity(rule.body.len());
            for term in &rule.body {
                if let Expr::Call(callee, _, cspan) = term {
                    if let Expr::Var(name, _) = callee.as_ref() {
                        if !program.decls.contains_key(name) && !is_native(name) {
                            return Err(RuleError::NotARelation {
                                name: name.clone(),
                                span: *cspan,
                            });
                        }
                    }
                }
                gens.push(match term {
                    Expr::Call(callee, args, span) => match callee.as_ref() {
                        Expr::Var(name, _) => match program.decls.get(name) {
                            Some(Decl::Relation(cols)) => {
                                for arg in args {
                                    if !matches!(arg, Expr::Lit(_, _) | Expr::Var(_, _)) {
                                        return Err(RuleError::ArgumentNotATerm {
                                            span: arg.span(),
                                        });
                                    }
                                }
                                if args.len() != cols.len() {
                                    return Err(RuleError::Arity {
                                        name: name.clone(),
                                        expected: cols.len(),
                                        found: args.len(),
                                        span: *span,
                                    });
                                }
                                true
                            }
                            _ => false,
                        },
                        _ => false,
                    },
                    _ => false,
                });
            }
            Ok(gens)
        }
        _ => Err(RuleError::NotARelation {
            name: rule.head.name.clone(),
            span: rule.head.span,
        }),
    }
}

/// Range restriction: every variable in the head, and every variable a
/// filter reads, is bound by some generator of the same rule — or names a
/// top-level definition (a global constant or a function), which every
/// definition may refer to. A generator-bound variable shadows a top-level
/// name; a variable bound by neither is `Unbound`. `generators` is
/// `check_generators`'s answer for this rule.
pub fn check_range_restriction(
    program: &Program,
    rule: &Rule,
    generators: &[bool],
) -> Result<(), RuleError> {
    // The variables the generators bind, in the order they appear.
    let mut names: Vec<Name> = Vec::new();
    for (term, is_generator) in rule.body.iter().zip(generators) {
        if *is_generator {
            if let Expr::Call(_, args, _) = term {
                for arg in args {
                    if let Expr::Var(name, _) = arg {
                        if !names.contains(name) {
                            names.push(name.clone());
                        }
                    }
                }
            }
        }
    }

    // A head variable outside that set is Unbound at the head.
    for term in &rule.head.terms {
        if let Term::Var(name) = term {
            if !names.contains(name) {
                return Err(RuleError::Unbound {
                    var: name.clone(),
                    span: rule.head.span,
                });
            }
        }
    }

    // A name a filter reads, from free_vars, that is not generator-bound and
    // not a top-level name or a native is Unbound at the filter.
    for (term, is_generator) in rule.body.iter().zip(generators) {
        if *is_generator {
            continue;
        }
        for var in free_vars(term) {
            if names.contains(&var) {
                continue;
            }
            if is_native(&var) {
                continue;
            }
            match program.decls.get(&var) {
                Some(_) => continue,
                None => {
                    return Err(RuleError::Unbound {
                        var,
                        span: term.span(),
                    })
                }
            }
        }
    }
    Ok(())
}

/// The declared functions the filter's calls reach, in the order they are
/// first reached (the filter's own callees, then their bodies, and so on).
fn reached_fns(term: &Expr, program: &Program) -> Vec<Name> {
    let mut out: Vec<Name> = Vec::new();
    push_callees(term, program, &mut out);
    let mut i = 0;
    while i < out.len() {
        let f = out[i].clone();
        i += 1;
        if let Some(Decl::Fn(_, _, _, body)) = program.decls.get(&f) {
            push_callees(body, program, &mut out);
        }
    }
    out
}

fn push_callees(e: &Expr, program: &Program, out: &mut Vec<Name>) {
    for sub in all_subexprs(e) {
        if let Expr::Call(callee, _, _) = sub {
            if let Expr::Var(name, _) = callee.as_ref() {
                if matches!(program.decls.get(name), Some(Decl::Fn(..))) && !out.contains(name) {
                    out.push(name.clone());
                }
            }
        }
    }
}

/// Whether the function `f` can reach a call to itself, directly or through
/// other functions.
fn calls_itself(f: &Name, program: &Program) -> bool {
    let mut seen: Vec<Name> = vec![f.clone()];
    let mut i = 0;
    while i < seen.len() {
        let g = seen[i].clone();
        i += 1;
        if let Some(Decl::Fn(_, _, _, body)) = program.decls.get(&g) {
            for sub in all_subexprs(body) {
                if let Expr::Call(callee, _, _) = sub {
                    if let Expr::Var(name, _) = callee.as_ref() {
                        if name == f {
                            return true;
                        }
                        if matches!(program.decls.get(name), Some(Decl::Fn(..)))
                            && !seen.contains(name)
                        {
                            seen.push(name.clone());
                        }
                    }
                }
            }
        }
    }
    false
}

/// Whether `ty` contains a function type anywhere inside it.
fn contains_fn_ty(t: &Ty) -> bool {
    match &t.kind {
        TyKind::Fn(..) => true,
        TyKind::Tuple(ts) | TyKind::Named(_, ts) => ts.iter().any(contains_fn_ty),
        TyKind::List(x) | TyKind::Ref(x) => contains_fn_ty(x),
        _ => false,
    }
}

/// The names of the impure functions, with the shortest witness from each
/// body to the effect it reaches: the **least** fixpoint of "reaches an
/// effect". A body reaches an effect when it contains a never-pure form
/// (empty witness), calls an impure function or a native effect or a relation
/// or a global, or calls through a parameter or a local (a name the analysis
/// cannot follow); the function passed as a callback to `map` / `filter` /
/// `fold` counts as called when it is an impure function, a global, or a
/// local.
fn compute_impure(program: &Program) -> HashMap<Name, Vec<Name>> {
    let fns: Vec<Name> = program
        .decls
        .iter()
        .filter(|(_, d)| matches!(d, Decl::Fn(..)))
        .map(|(n, _)| n.clone())
        .collect();
    let mut impure: HashMap<Name, Vec<Name>> = HashMap::new();
    loop {
        let mut next: HashMap<Name, Vec<Name>> = HashMap::new();
        for name in &fns {
            let Some(Decl::Fn(_, params, _, body)) = program.decls.get(name) else {
                continue;
            };
            let mut locals = bound_names(body);
            for p in params {
                locals.insert(p.name.clone());
            }
            if let Some((_, w)) = witness_in(body, &locals, program, &impure) {
                next.insert(name.clone(), w);
            }
        }
        if next == impure {
            return next;
        }
        impure = next;
    }
}

/// The first effect `e` reaches, in source order: a `(span, witness)` pair,
/// the witness empty when the effect is a form of the expression's own.
/// `locals` are the names bound within the body being judged (a function's
/// parameters plus its `let`s and such); `impure` is the set of impure
/// functions known so far.
fn witness_in(
    e: &Expr,
    locals: &HashSet<Name>,
    program: &Program,
    impure: &HashMap<Name, Vec<Name>>,
) -> Option<(Span, Vec<Name>)> {
    for sub in all_subexprs(e) {
        let span = sub.span();
        match sub {
            // The never-pure forms: every query, a relational loop, a
            // while loop, an assignment, and a reference read or write.
            Expr::Relation(rel, _) => {
                return Some((
                    span,
                    match rel {
                        Rel::Query(q) => vec![q.name.clone()],
                        _ => Vec::new(),
                    },
                ))
            }
            Expr::ForQuery(q, _, _) => return Some((span, vec![q.name.clone()])),
            Expr::While(..) | Expr::For(..) | Expr::Assign(..) => return Some((span, Vec::new())),
            Expr::Unary(UnOp::Ref | UnOp::Deref, _, _) => return Some((span, Vec::new())),
            Expr::Call(callee, args, _) => match callee.as_ref() {
                Expr::Var(name, _) => {
                    // A locally bound name: a call through it cannot be
                    // followed, whatever function shares its name.
                    if locals.contains(name) {
                        return Some((span, vec![name.clone()]));
                    }
                    // An impure function: the witness runs through it.
                    if let Some(w) = impure.get(name) {
                        let mut v = vec![name.clone()];
                        v.extend(w.iter().cloned());
                        return Some((span, v));
                    }
                    // A native effect.
                    if IMPURE_NATIVES.contains(&name.as_str()) {
                        return Some((span, vec![name.clone()]));
                    }
                    // A relation (a query) or a global (a closure the
                    // analysis cannot follow).
                    match program.decls.get(name) {
                        Some(Decl::Relation(_)) | Some(Decl::Global(..)) => {
                            return Some((span, vec![name.clone()]));
                        }
                        Some(_) => {}
                        None => {}
                    }
                    // The callback position of `map` / `filter` / `fold`:
                    // the function handed in is as good as called.
                    let cb = match name.as_str() {
                        "map" | "filter" => Some(1),
                        "fold" => Some(2),
                        _ => None,
                    };
                    if let Some(idx) = cb {
                        if let Some(Expr::Var(n2, _)) = args.get(idx) {
                            if let Some(w) = impure.get(n2) {
                                let mut v = vec![n2.clone()];
                                v.extend(w.iter().cloned());
                                return Some((span, v));
                            }
                            if matches!(
                                program.decls.get(n2),
                                Some(Decl::Relation(_)) | Some(Decl::Global(..))
                            ) || locals.contains(n2)
                            {
                                return Some((span, vec![n2.clone()]));
                            }
                        }
                    }
                    // A declared pure function: nothing here.
                }
                // A computed callee: an applied lambda, not followable.
                _ => return Some((span, Vec::new())),
            },
            Expr::Var(name, _) if IMPURE_NATIVES.contains(&name.as_str()) => {
                return Some((span, vec![name.clone()]));
            }
            _ => {}
        }
    }
    None
}

/// A filter must be pure (reference, "Purity and effects"). The judgment
/// makes three checks per filter, in body order, at the filter's span:
/// a function it reaches with a function-typed parameter is `HigherOrder`,
/// an effect it reaches is `Impure { witness }`, and a function it can
/// reach itself is `Recursive { name }`.
pub fn check_pure(program: &Program, rule: &Rule, generators: &[bool]) -> Result<(), RuleError> {
    let impure = compute_impure(program);

    for (term, is_generator) in rule.body.iter().zip(generators) {
        if *is_generator {
            continue;
        }
        let locals = bound_names(term);

        // An undeclared, non-native name in call position names no relation.
        for sub in all_subexprs(term) {
            if let Expr::Call(callee, _, cspan) = sub {
                if let Expr::Var(name, _) = callee.as_ref() {
                    if !locals.contains(name)
                        && !is_native(name)
                        && !program.decls.contains_key(name)
                    {
                        return Err(RuleError::NotARelation {
                            name: name.clone(),
                            span: *cspan,
                        });
                    }
                }
            }
        }

        // A function the filter reaches with a function-typed parameter
        // cannot be shown total.
        for name in reached_fns(term, program) {
            if let Some(Decl::Fn(_, params, _, _)) = program.decls.get(&name) {
                if params.iter().any(|p| contains_fn_ty(&p.ty)) {
                    return Err(RuleError::HigherOrder {
                        name: name.clone(),
                        span: term.span(),
                    });
                }
            }
        }

        // An effect the filter reaches: the witness is the chain of calls
        // from the filter to the effect, outermost first.
        if let Some((span, witness)) = witness_in(term, &locals, program, &impure) {
            return Err(RuleError::Impure { span, witness });
        }

        // A function the filter reaches that can reach itself.
        for name in reached_fns(term, program) {
            if calls_itself(&name, program) {
                return Err(RuleError::Recursive {
                    name: name.clone(),
                    span: term.span(),
                });
            }
        }
    }
    Ok(())
}

/// Every `match` guard in the program must be pure (M7): a guard is a test
/// the matcher runs while choosing an arm, and an effect there would make the
/// choice depend on how many arms were tried. The same purity judgment as a
/// rule filter's, including that no function it reaches takes a
/// function-typed parameter (the analysis cannot follow one); recursion is
/// allowed, since a guard runs once per arm and need not be total. Reports
/// `ImpureGuard { witness }`.
pub fn check_guards(program: &Program) -> Result<(), RuleError> {
    let _ = program;
    todo_m7!("check_guards: every match guard in the program is pure")
}

/// Provided helpers (Milestone M6): walks over expressions that no rule
/// describes. Paste this module at the end of the file, unchanged.
pub mod helpers {
    use crate::ast::{Expr, Name, QArg, Query, Rel, Stmt};
    use std::collections::HashSet;

    /// The immediate subexpressions of `e`, in source order. A query
    /// contributes its bound arguments, never its holes.
    pub fn all_subexprs(e: &Expr) -> Vec<&Expr> {
        let mut cs: Vec<&Expr> = Vec::new();
        cs.push(e);
        match e {
            Expr::Lit(..) | Expr::Var(..) => {}
            Expr::Unary(_, e1, _)
            | Expr::Field(e1, _, _)
            | Expr::Proj(e1, _, _)
            | Expr::Return(e1, _)
            | Expr::Try(e1, _)
            | Expr::Lambda(_, e1, _) => cs.extend(all_subexprs(e1)),
            Expr::Binary(_, l, r, _) | Expr::Assign(l, r, _) => {
                cs.extend(all_subexprs(l));
                cs.extend(all_subexprs(r));
            }
            Expr::If(c, t, els, _) => {
                cs.extend(all_subexprs(c));
                cs.extend(all_subexprs(t));
                if let Some(e1) = els {
                    cs.extend(all_subexprs(e1));
                }
            }
            Expr::While(c, b, _) | Expr::For(_, c, b, _) => {
                cs.extend(all_subexprs(c));
                cs.extend(all_subexprs(b));
            }
            Expr::Call(f, args, _) | Expr::Method(f, _, args, _) => {
                cs.extend(all_subexprs(f));
                args.iter().for_each(|arg| cs.extend(all_subexprs(arg)));
            }
            Expr::Tuple(es, _) | Expr::List(es, _) | Expr::Ctor(_, es, _) => {
                es.iter().for_each(|e| cs.extend(all_subexprs(e)));
            }
            Expr::Struct(_, fields, _) => fields
                .iter()
                .map(|(_, e)| e)
                .for_each(|e| cs.extend(all_subexprs(e))),
            Expr::Match(scrut, arms, _) => {
                cs.push(scrut);
                for a in arms {
                    if let Some(g) = &a.guard {
                        cs.extend(all_subexprs(g));
                    }
                    cs.extend(all_subexprs(&a.body));
                }
            }
            Expr::Block(stmts, tail, _) => {
                for s in stmts {
                    match s {
                        Stmt::Let(_, _, e1, _) | Stmt::Expr(e1) => cs.extend(all_subexprs(e1)),
                    }
                }
                if let Some(t) = tail {
                    cs.extend(all_subexprs(t));
                }
            }
            Expr::ForQuery(q, b, _) => {
                query_exprs(q).for_each(|e| cs.extend(all_subexprs(e)));
                cs.extend(all_subexprs(b));
            }
            Expr::Relation(rel, _) => match rel {
                Rel::Add(q) | Rel::Solutions(q) | Rel::Query(q) => {
                    query_exprs(q).for_each(|e| cs.extend(all_subexprs(e)));
                }
                Rel::Clear(_) => {}
            },
        }
        cs
    }

    /// The immediate subexpressions of `e`, in source order. A query
    /// contributes its bound arguments, never its holes.
    pub fn subexprs(e: &Expr) -> Vec<&Expr> {
        let mut cs: Vec<&Expr> = Vec::new();
        match e {
            Expr::Lit(..) | Expr::Var(..) => {}
            Expr::Unary(_, e1, _)
            | Expr::Field(e1, _, _)
            | Expr::Proj(e1, _, _)
            | Expr::Return(e1, _)
            | Expr::Try(e1, _)
            | Expr::Lambda(_, e1, _) => cs.push(e1),
            Expr::Binary(_, l, r, _) | Expr::Assign(l, r, _) => {
                cs.push(l);
                cs.push(r);
            }
            Expr::If(c, t, els, _) => {
                cs.push(c);
                cs.push(t);
                if let Some(e1) = els {
                    cs.push(e1);
                }
            }
            Expr::While(c, b, _) | Expr::For(_, c, b, _) => {
                cs.push(c);
                cs.push(b);
            }
            Expr::Call(f, args, _) | Expr::Method(f, _, args, _) => {
                cs.push(f);
                cs.extend(args.iter());
            }
            Expr::Tuple(es, _) | Expr::List(es, _) | Expr::Ctor(_, es, _) => {
                cs.extend(es.iter());
            }
            Expr::Struct(_, fields, _) => cs.extend(fields.iter().map(|(_, e)| e)),
            Expr::Match(scrut, arms, _) => {
                cs.push(scrut);
                for a in arms {
                    if let Some(g) = &a.guard {
                        cs.push(g);
                    }
                    cs.push(&a.body);
                }
            }
            Expr::Block(stmts, tail, _) => {
                for s in stmts {
                    match s {
                        Stmt::Let(_, _, e1, _) | Stmt::Expr(e1) => cs.push(e1),
                    }
                }
                if let Some(t) = tail {
                    cs.push(t);
                }
            }
            Expr::ForQuery(q, b, _) => {
                cs.extend(query_exprs(q));
                cs.push(b);
            }
            Expr::Relation(rel, _) => match rel {
                Rel::Add(q) | Rel::Solutions(q) | Rel::Query(q) => {
                    cs.extend(query_exprs(q));
                }
                Rel::Clear(_) => {}
            },
        }
        cs
    }

    /// The bound arguments of a query: every argument that is not a hole.
    pub fn query_exprs(q: &Query) -> impl Iterator<Item = &Expr> {
        q.args.iter().filter_map(|a| match a {
            QArg::Expr(e) => Some(e),
            QArg::Hole(_) => None,
        })
    }

    /// The names `e` reads and does not bind itself, each once, in order of
    /// first appearance: every variable in value position, minus those bound
    /// within `e` (a `let` over the rest of its block, a lambda's parameters,
    /// a `for` variable, a relational `for`'s named holes, a pattern's
    /// names). A direct call's callee, `f` in `f(x)`, is not counted; its
    /// arguments are.
    pub fn free_vars(e: &Expr) -> Vec<Name> {
        let mut out = Vec::new();
        free_vars_into(e, &HashSet::new(), &mut out);
        out
    }

    fn free_vars_into(e: &Expr, bound: &HashSet<Name>, out: &mut Vec<Name>) {
        match e {
            Expr::Var(x, _) => {
                if !bound.contains(x) && !out.contains(x) {
                    out.push(x.clone());
                }
            }
            Expr::Call(callee, args, _) => {
                if !matches!(&**callee, Expr::Var(..)) {
                    free_vars_into(callee, bound, out);
                }
                for a in args {
                    free_vars_into(a, bound, out);
                }
            }
            Expr::Lambda(params, body, _) => {
                let mut inner = bound.clone();
                inner.extend(params.iter().map(|p| p.name.clone()));
                free_vars_into(body, &inner, out);
            }
            Expr::Block(stmts, tail, _) => {
                let mut inner = bound.clone();
                for s in stmts {
                    match s {
                        Stmt::Let(x, _, rhs, _) => {
                            free_vars_into(rhs, &inner, out);
                            inner.insert(x.clone());
                        }
                        Stmt::Expr(e1) => free_vars_into(e1, &inner, out),
                    }
                }
                if let Some(t) = tail {
                    free_vars_into(t, &inner, out);
                }
            }
            Expr::For(x, iter, body, _) => {
                free_vars_into(iter, bound, out);
                let mut inner = bound.clone();
                inner.insert(x.clone());
                free_vars_into(body, &inner, out);
            }
            Expr::Match(scrut, arms, _) => {
                free_vars_into(scrut, bound, out);
                for arm in arms {
                    let mut inner = bound.clone();
                    inner.extend(arm.pat.binders());
                    if let Some(g) = &arm.guard {
                        free_vars_into(g, &inner, out);
                    }
                    free_vars_into(&arm.body, &inner, out);
                }
            }
            Expr::ForQuery(q, body, _) => {
                let mut inner = bound.clone();
                for a in &q.args {
                    match a {
                        QArg::Expr(e1) => free_vars_into(e1, bound, out),
                        QArg::Hole(Some(x)) => {
                            inner.insert(x.clone());
                        }
                        QArg::Hole(None) => {}
                    }
                }
                free_vars_into(body, &inner, out);
            }
            _ => {
                for c in subexprs(e) {
                    free_vars_into(c, bound, out);
                }
            }
        }
    }

    /// Every name bound anywhere within `e`: `let` names, lambda
    /// parameters, `for` variables, a relational `for`'s named holes, and
    /// the names a `match` arm's pattern binds.
    pub fn bound_names(e: &Expr) -> HashSet<Name> {
        let mut out = HashSet::new();
        bound_names_into(e, &mut out);
        out
    }

    fn bound_names_into(e: &Expr, out: &mut HashSet<Name>) {
        match e {
            Expr::Block(stmts, _, _) => {
                for s in stmts {
                    if let Stmt::Let(x, ..) = s {
                        out.insert(x.clone());
                    }
                }
            }
            Expr::Lambda(params, _, _) => {
                out.extend(params.iter().map(|p| p.name.clone()));
            }
            Expr::Match(_, arms, _) => {
                for arm in arms {
                    out.extend(arm.pat.binders());
                }
            }
            Expr::For(x, ..) => {
                out.insert(x.clone());
            }
            Expr::ForQuery(q, ..) => {
                for a in &q.args {
                    if let QArg::Hole(Some(x)) = a {
                        out.insert(x.clone());
                    }
                }
            }
            _ => {}
        }
        for c in subexprs(e) {
            bound_names_into(c, out);
        }
    }
}
