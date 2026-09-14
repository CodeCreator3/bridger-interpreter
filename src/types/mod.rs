//! The type checker  [frozen — do not edit]  (comes alive at M5)
//!
//! A separate static pass over the loaded program (Part VI). The provided part
//! is here: the [`Checker`] with its tables, the typing context [`Ctx`] (the
//! judgment's Γ), the inference variables, and the driver that walks the
//! program's declarations. The meaning — one arm per typing rule — is
//! [`Checker::check_expr`], which you write in `types/check.rs`.
//!
//! The pass **annotates on the side**: it leaves the AST untouched and records
//! each expression's type in a [`TypeMap`] keyed by the expression's span,
//! which the determination check reads back and tests inspect; the evaluator
//! receives only the [`Conversions`]. An accepted program is one whose every declaration
//! checks; the evaluator then runs it unchanged, type-erased.

use crate::ast::{
    Decl, Expr, Generics, Impl, Method, Name, Program, Rule, Span, SrcId, Stmt, TraitRef, Ty,
    TyKind,
};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// The impl methods the evaluator must run, keyed by span: the `from` a `?`
/// converts its error through (none for a `?` whose error type already
/// matches), and the instance's method for a call whose receiver head alone
/// could not choose between several impls of one trait.
pub type Conversions = HashMap<Span, Rc<Method>>;

fn synthetic_span() -> Span {
    Span::new(SrcId::SYNTHETIC, 0, 0)
}

/// How deeply an expression may nest before the checker gives up: far more
/// than any written program, and short of the interpreter's stack for every
/// construct (an `if` costs the checker some 9 KB of stack per level).
pub const MAX_NESTING: usize = 150_000;

mod check;
pub mod error;

pub use error::TyError;

/// The result of a successful check: each expression's type, by span.
pub type TypeMap = HashMap<Span, Ty>;

/// The typing context Γ: what each name in scope has for a type, plus the two
/// facts the rules read from the enclosing declaration — its return type (for
/// `return`) and, inside an `impl`, the type `Self` stands for. Immutable from
/// your side, exactly like [`Env`](crate::interp::env::Env): `extend` returns a child.
#[derive(Clone)]
pub struct Ctx(Rc<Scope>);

enum Scope {
    Root {
        ret: Option<Ty>,
        self_ty: Option<Ty>,
    },
    Frame {
        name: Name,
        ty: Ty,
        parent: Ctx,
    },
    /// A new function boundary layered over the bindings in scope: it overrides
    /// `ret` / `self_ty` (for a lambda body, whose `return` targets the lambda,
    /// not the enclosing function) while the parent's bindings stay visible.
    Fn {
        ret: Option<Ty>,
        self_ty: Option<Ty>,
        parent: Ctx,
    },
}

impl Ctx {
    /// An empty context for a declaration with return type `ret` (`None`
    /// outside any function, where `return` is an error) and `Self` bound to
    /// `self_ty` inside an `impl`.
    pub fn new(ret: Option<Ty>, self_ty: Option<Ty>) -> Ctx {
        Ctx(Rc::new(Scope::Root { ret, self_ty }))
    }

    /// The type bound to `x`, searching outward through enclosing scopes.
    pub fn lookup(&self, x: &str) -> Option<Ty> {
        match &*self.0 {
            Scope::Frame { name, ty, parent } => {
                if name == x {
                    Some(ty.clone())
                } else {
                    parent.lookup(x)
                }
            }
            Scope::Fn { parent, .. } => parent.lookup(x),
            Scope::Root { .. } => None,
        }
    }

    /// Bind `x : ty` in a fresh child scope and return it.
    pub fn extend(&self, x: Name, ty: Ty) -> Ctx {
        Ctx(Rc::new(Scope::Frame {
            name: x,
            ty,
            parent: self.clone(),
        }))
    }

    /// Whether the innermost function boundary is a lambda's.
    pub fn in_lambda(&self) -> bool {
        match &*self.0 {
            Scope::Frame { parent, .. } => parent.in_lambda(),
            Scope::Fn { .. } => true,
            Scope::Root { .. } => false,
        }
    }

    /// Enter a nested function boundary: the bindings in scope stay visible, but
    /// `ret` and `self_ty` are overridden. Used for a lambda body, whose `return`
    /// targets the lambda's own result type.
    pub fn enter(&self, ret: Option<Ty>, self_ty: Option<Ty>) -> Ctx {
        Ctx(Rc::new(Scope::Fn {
            ret,
            self_ty,
            parent: self.clone(),
        }))
    }

    /// The enclosing declaration's return type, if inside a function.
    pub fn ret(&self) -> Option<Ty> {
        match &*self.0 {
            Scope::Frame { parent, .. } => parent.ret(),
            Scope::Fn { ret, .. } => ret.clone(),
            Scope::Root { ret, .. } => ret.clone(),
        }
    }

    /// What `Self` stands for, inside an `impl`.
    pub fn self_ty(&self) -> Option<Ty> {
        match &*self.0 {
            Scope::Frame { parent, .. } => parent.self_ty(),
            Scope::Fn { self_ty, .. } => self_ty.clone(),
            Scope::Root { self_ty, .. } => self_ty.clone(),
        }
    }
}

/// The type checker: the program's declarations (the global part of Γ — every
/// `fn` signature, `type`, `struct`, `trait`, and `relation`), the type map
/// being built, and the inference variables solved so far.
pub struct Checker {
    program: Program,
    types: TypeMap,
    metas: Vec<Option<Ty>>,
    conversions: Conversions,
    /// The nesting of the expression being checked, held below
    /// [`MAX_NESTING`] so a pathological tree is a clean error.
    #[allow(dead_code)] // read from M5 on (check_expr's nesting guard)
    pub(super) depth: usize,
    /// Bound obligations gathered at generic use sites — `(trait, type, span)` —
    /// discharged once at the end, after inference has solved the types.
    obligations: Vec<(TraitRef, Ty, Span)>,
    /// The bounds on the type parameters of the declaration currently being
    /// checked (`T -> Show` for `fn f<T: Show>`), so a call through a bound
    /// (`x.show()` with `x : T`) resolves against the trait.
    type_bounds: HashMap<Name, TraitRef>,
    /// The type parameters of the declaration currently being checked, bounded
    /// or not — the names an annotation may use as types besides the declared
    /// ones.
    type_params: HashSet<Name>,
    /// The type of each global, recorded as the driver checks the globals in
    /// initialization order — before any body that reads them — so a
    /// reference to an unannotated global reads its type here instead of
    /// inferring the initializer again (a chain of such globals would
    /// otherwise be re-inferred exponentially often).
    global_types: HashMap<Name, Ty>,
    /// The spans recorded by `record`, in order, so the driver can ask that
    /// every expression of the declaration just checked has a determined type.
    recorded: Vec<Span>,
    /// Expressions that never produce a value (`return e`): their type is
    /// whatever the context wants, and unconstrained when nothing does.
    diverging: HashSet<Span>,
    /// Result-type metavariables a `?` pinned to a carrier — a lambda's
    /// inferred result — so a body that then mismatches can be explained.
    try_pinned: HashSet<u32>,
    /// The callees of calls, exempt from the determination rule: whatever is
    /// unknown in a callee's type is unknown in an argument or in the call
    /// itself, which are the places to blame.
    callees: HashSet<Span>,
    /// Source ranges that never run: the rest of a block after a statement
    /// that leaves, the branches or body of a form whose condition, scrutinee,
    /// or iterable leaves. Their types are exempt from the determination rule
    /// and their bound obligations are not judged, so dead code needs no
    /// annotation — while a live value that a leaving form's type reached is
    /// judged as usual.
    dead_code: Vec<Span>,
}

impl Checker {
    /// A checker over `program`'s declarations.
    pub fn new(program: &Program) -> Self {
        Checker {
            program: program.clone(),
            types: TypeMap::new(),
            metas: Vec::new(),
            conversions: Conversions::new(),
            depth: 0,
            global_types: HashMap::new(),
            recorded: Vec::new(),
            diverging: HashSet::new(),
            try_pinned: HashSet::new(),
            dead_code: Vec::new(),
            callees: HashSet::new(),
            obligations: Vec::new(),
            type_bounds: HashMap::new(),
            type_params: HashSet::new(),
        }
    }

    /// Enter the scope of a declaration's generic parameters: what its
    /// annotations may name and which bounds its calls may use.
    fn enter_generics(&mut self, generics: &crate::ast::Generics) -> Result<(), TyError> {
        // A parameter may not take the name of a type: every lookup by name
        // would have to decide which was meant.
        for p in &generics.params {
            let builtin = matches!(p.name.as_str(), "Int" | "Bool" | "String");
            if builtin || self.decl(&p.name).is_some() {
                return Err(TyError::TypeParamShadows {
                    name: p.name.clone(),
                    span: p.span,
                });
            }
        }
        self.type_bounds = bounds_of(generics);
        self.type_params = generics.params.iter().map(|p| p.name.clone()).collect();
        // Every bound names a declared trait, applied to as many type
        // arguments as it declares, each a well-formed type.
        for b in generics.params.iter().filter_map(|p| p.bound.as_ref()) {
            match self.decl(&b.name) {
                Some(Decl::Trait(tg, _)) => {
                    if tg.params.len() != b.args.len() {
                        return Err(TyError::TraitArity {
                            name: b.name.clone(),
                            expected: tg.params.len(),
                            found: b.args.len(),
                            span: b.span,
                        });
                    }
                }
                _ => {
                    return Err(TyError::UnknownTrait {
                        name: b.name.clone(),
                        span: b.span,
                    })
                }
            }
            for a in &b.args {
                self.validate_type(a, b.span)?;
            }
        }
        Ok(())
    }

    /// Check that a written type is well-formed: every name it uses is a
    /// declared `type` or `struct` applied to the right number of arguments,
    /// or a type parameter of the enclosing declaration (which takes none).
    /// `Self` must already have been resolved (see [`Ty::with_self`]); one
    /// that remains is reported as unknown. `span` locates the error when the
    /// type itself carries no source position.
    pub fn validate_type(&self, ty: &Ty, span: Span) -> Result<(), TyError> {
        let at = ty.span.unwrap_or(span);
        match &ty.kind {
            TyKind::Int | TyKind::Bool | TyKind::Str | TyKind::Unit | TyKind::Meta(_) => Ok(()),
            TyKind::SelfTy => Err(TyError::UnknownType {
                name: "Self".to_string(),
                span: at,
            }),
            TyKind::Tuple(ts) => ts.iter().try_for_each(|t| self.validate_type(t, at)),
            TyKind::List(t) | TyKind::Ref(t) => self.validate_type(t, at),
            TyKind::Fn(ps, r) => {
                ps.iter().try_for_each(|t| self.validate_type(t, at))?;
                self.validate_type(r, at)
            }
            TyKind::Named(name, args) => {
                // `Int<T>`: the parser keeps an applied built-in as a name.
                let expected = if self.type_params.contains(name)
                    || matches!(name.as_str(), "Int" | "Bool" | "String")
                {
                    0
                } else {
                    match self.program.decls.get(name) {
                        Some(Decl::Type(generics, _)) | Some(Decl::Struct(generics, _)) => {
                            generics.params.len()
                        }
                        _ => {
                            return Err(TyError::UnknownType {
                                name: name.clone(),
                                span: at,
                            })
                        }
                    }
                };
                if args.len() != expected {
                    return Err(TyError::TypeArity {
                        name: name.clone(),
                        expected,
                        found: args.len(),
                        span: at,
                    });
                }
                args.iter().try_for_each(|t| self.validate_type(t, at))
            }
        }
    }

    /// Record that `ty` must satisfy trait `trait_` (a bound met at a use site).
    pub fn require_bound(&mut self, trait_: Name, ty: Ty, span: Span) {
        self.require_trait(
            TraitRef {
                name: trait_,
                args: Vec::new(),
                span,
            },
            ty,
            span,
        );
    }

    /// Record that `ty` must satisfy the bound `trait_`, type arguments
    /// included (`T: Conv<String>` is met only by an `impl Conv<String>`).
    pub fn require_trait(&mut self, trait_: TraitRef, ty: Ty, span: Span) {
        self.obligations.push((trait_, ty, span));
    }

    /// The trait bound on type parameter `name` in the declaration being
    /// checked, if any.
    pub fn bound_of(&self, name: &str) -> Option<&TraitRef> {
        self.type_bounds.get(name)
    }

    /// Record the impl method the evaluator must run at `span`: the `from` a
    /// `?` converts through (a `?` whose error already matches records
    /// nothing; M8), or the method chosen among several instances of a trait.
    pub fn record_conversion(&mut self, span: Span, from: Method) {
        self.conversions.insert(span, Rc::new(from));
    }

    /// How many `impl`s of `trait_` (at any instance) the head of `ty` has.
    fn instances(&self, trait_: &str, ty: &Ty) -> usize {
        let key = head_key(ty);
        self.program
            .impls
            .iter()
            .filter(|imp| imp.trait_.as_ref().is_some_and(|t| t.name == trait_))
            .filter(|imp| head_key(&imp.ty) == key)
            .count()
    }

    /// The declaration named `name`, if any.
    pub fn decl(&self, name: &str) -> Option<&Decl> {
        self.program.decls.get(name)
    }

    /// The whole program being checked.
    pub fn program(&self) -> &Program {
        &self.program
    }

    /// Record that the expression at `span` has type `ty`. Call this from
    /// every arm of `check_expr` that returns a type.
    pub fn record(&mut self, span: Span, ty: Ty) {
        self.types.insert(span, ty);
        self.recorded.push(span);
    }

    /// Mark the expression at `span` as one that never produces a value, so
    /// its (arbitrary) type is exempt from `require_resolved`.
    pub fn record_diverging(&mut self, span: Span) {
        self.diverging.insert(span);
    }

    /// Start a declaration: the index its recorded types begin at. The dead
    /// ranges of earlier declarations are done with — each was judged before
    /// the next began — so they are dropped rather than scanned again.
    pub fn begin_item(&mut self) -> usize {
        self.dead_code.clear();
        self.recorded.len()
    }

    /// Note the callee of a call, exempt from the determination rule.
    pub fn note_callee(&mut self, span: Span) {
        self.callees.insert(span);
    }

    /// `whole` leaves because its part `sub` does: mark `whole` diverging and
    /// the code after `sub` within it as never running.
    pub fn dead_after(&mut self, sub: Span, whole: Span) {
        self.record_diverging(whole);
        if sub.end < whole.end {
            self.dead_code
                .push(Span::new(whole.src, sub.end, whole.end));
        }
    }

    /// Whether `span` lies in code that never runs.
    pub fn in_dead_code(&self, span: Span) -> bool {
        self.dead_code
            .iter()
            .any(|d| d.src == span.src && d.start <= span.start && span.end <= d.end)
    }

    /// Note that a `?` pinned the result metavariable `id` to a carrier.
    pub fn note_try_pinned(&mut self, id: u32) {
        self.try_pinned.insert(id);
    }

    /// Whether a `?` pinned the result metavariable `id`.
    pub fn try_pinned(&self, id: u32) -> bool {
        self.try_pinned.contains(&id)
    }

    /// Whether the expression at `span` was marked as never producing a value.
    pub fn is_diverging(&self, span: Span) -> bool {
        self.diverging.contains(&span)
    }

    /// Every expression recorded since `start` has a fully determined type:
    /// one that still contains an inference variable once its declaration is
    /// inferred is a static error, reported at the earliest such expression
    /// ("type annotation needed"). A binding, a lambda parameter, an empty
    /// list, or a `None` that nothing pins down is caught here.
    fn require_resolved(&self, start: usize) -> Result<(), TyError> {
        let first = self.recorded[start..]
            .iter()
            .filter(|span| {
                !self.diverging.contains(span)
                    && !self.callees.contains(span)
                    && !self.in_dead_code(**span)
            })
            .filter(|span| {
                self.types
                    .get(span)
                    .is_some_and(|ty| contains_meta(&self.resolve(ty)))
            })
            .min_by_key(|span| (span.src.0, span.start, span.end));
        match first {
            Some(span) => Err(TyError::Ambiguous { span: *span }),
            None => Ok(()),
        }
    }

    /// The type recorded for the expression at `span`, if any.
    pub fn type_at(&self, span: Span) -> Option<&Ty> {
        self.types.get(&span)
    }

    /// A fresh inference variable (Part VI, local inference): a type to be
    /// solved later by unification.
    pub fn fresh_meta(&mut self) -> Ty {
        self.metas.push(None);
        Ty::meta(self.metas.len() as u32 - 1)
    }

    /// Solve inference variable `id` as `ty`. Your `unify` decides *when*; this
    /// only records it. Panics on an id this checker never issued.
    pub fn solve(&mut self, id: u32, ty: Ty) {
        self.metas[id as usize] = Some(ty);
    }

    /// The solution of inference variable `id`, if it has one.
    pub fn solution(&self, id: u32) -> Option<&Ty> {
        self.metas[id as usize].as_ref()
    }

    /// `ty` with every solved inference variable replaced by its solution,
    /// recursively. An unsolved variable stays a `TyKind::Meta`. Spans are kept.
    pub fn resolve(&self, ty: &Ty) -> Ty {
        self.resolved(ty).unwrap_or_else(|| ty.clone())
    }

    /// `ty` with every solved inference variable replaced, or `None` when it
    /// holds none — so a subtree without variables is shared, never rebuilt.
    fn resolved(&self, ty: &Ty) -> Option<Ty> {
        fn each(c: &Checker, ts: &Rc<[Ty]>) -> Option<Rc<[Ty]>> {
            let rs: Vec<Option<Ty>> = ts.iter().map(|t| c.resolved(t)).collect();
            if rs.iter().all(Option::is_none) {
                return None;
            }
            Some(
                ts.iter()
                    .zip(rs)
                    .map(|(t, r)| r.unwrap_or_else(|| t.clone()))
                    .collect(),
            )
        }
        fn one(c: &Checker, t: &Rc<Ty>) -> Option<Rc<Ty>> {
            c.resolved(t).map(Rc::new)
        }
        let kind = match &ty.kind {
            TyKind::Meta(id) => {
                let t = self.solution(*id)?;
                return Some(self.resolve(t));
            }
            TyKind::Tuple(ts) => TyKind::Tuple(each(self, ts)?),
            TyKind::List(t) => TyKind::List(one(self, t)?),
            TyKind::Ref(t) => TyKind::Ref(one(self, t)?),
            TyKind::Fn(ps, r) => match (each(self, ps), one(self, r)) {
                (None, None) => return None,
                (ps2, r2) => TyKind::Fn(
                    ps2.unwrap_or_else(|| ps.clone()),
                    r2.unwrap_or_else(|| r.clone()),
                ),
            },
            TyKind::Named(n, args) => TyKind::Named(n.clone(), each(self, args)?),
            TyKind::Int | TyKind::Bool | TyKind::Str | TyKind::Unit | TyKind::SelfTy => {
                return None
            }
        };
        Some(Ty {
            kind,
            span: ty.span,
        })
    }

    /// Check every declaration of `program` and return the type map. Each
    /// `fn` body is checked against its declared return type in a context
    /// binding its parameters; each global against its annotation, if any;
    /// each `impl` method likewise, with `Self` bound to the implemented type.
    /// All of it goes through your [`Checker::check_expr`].
    pub fn check_program(program: &Program) -> Result<TypeMap, TyError> {
        Self::check_and_elaborate(program).map(|(types, _)| types)
    }

    /// As [`Checker::check_program`], but also returns the `?` conversions the
    /// evaluator needs ([`Conversions`]) — the elaboration `run` installs before
    /// evaluating, so a cross-type `?` converts its error at run time (M8).
    pub fn check_and_elaborate(program: &Program) -> Result<(TypeMap, Conversions), TyError> {
        let mut checker = Checker::new(program);
        // Globals must have an initialization order (the evaluator runs them in
        // it); with a cycle, an unannotated global's type would also be its own
        // premise, so reject the program here.
        let order = match program.initialization_order() {
            Ok(order) => order,
            Err(cycle) => {
                return Err(TyError::InitializationCycle {
                    names: cycle.names,
                    span: cycle.span,
                })
            }
        };
        // Globals first, in initialization order, each checked once and its
        // type recorded: a later global's initializer, and every function
        // body, then reads the recorded type. A global has the one type its
        // initializer has, so an inference variable left in it is shared by
        // every use.
        for name in &order {
            let Some(Decl::Global(ty, init)) = program.decls.get(name) else {
                continue;
            };
            if program.spans.get(name).is_some_and(|s| s.src == SrcId(0)) {
                continue;
            }
            let decl_span =
                program
                    .spans
                    .get(name)
                    .copied()
                    .unwrap_or(Span::new(SrcId::SYNTHETIC, 0, 0));
            checker.enter_generics(&Generics { params: Vec::new() })?;
            if let Some(ty) = ty {
                checker.validate_type(ty, decl_span)?;
            }
            let ctx = Ctx::new(None, None);
            let start = checker.begin_item();
            let inferred = checker.check_expr(init, &ctx, ty.as_ref())?;
            // A global has one type at every reference, so its initializer
            // (with its annotation) must fix it completely.
            checker.require_resolved(start)?;
            let recorded = ty.clone().unwrap_or_else(|| checker.resolve(&inferred));
            checker.global_types.insert(name.clone(), recorded);
            checker.discharge_bounds()?;
        }
        Self::check_impls(&mut checker)?;
        // `main` takes no parameters and no type parameters (Appendix D,
        // "Bindings and blocks"); a program without one is judged when it runs, so a
        // fragment can still be checked.
        if let Some(Decl::Fn(generics, params, _, _)) = program.decls.get("main") {
            if !generics.params.is_empty() || !params.is_empty() {
                return Err(TyError::MainSignature {
                    span: program
                        .spans
                        .get("main")
                        .copied()
                        .unwrap_or(synthetic_span()),
                });
            }
        }
        // Every remaining declaration, impl, and rule, in source order, so the
        // first error reported is the first in the file.
        enum Item<'a> {
            Decl(&'a Name, &'a Decl),
            Impl(&'a Impl),
            Rule(&'a Rule),
        }
        let synthetic = Span::new(SrcId::SYNTHETIC, 0, 0);
        let mut items: Vec<(Span, Item)> = program
            .decls
            .iter()
            .map(|(n, d)| {
                (
                    program.spans.get(n).copied().unwrap_or(synthetic),
                    Item::Decl(n, d),
                )
            })
            .chain(program.impls.iter().map(|i| (i.span, Item::Impl(i))))
            .chain(program.rules.iter().map(|r| (r.span, Item::Rule(r))))
            .collect();
        items.sort_by_key(|(s, _)| (s.src.0, s.start));
        for (_, item) in items {
            match item {
                Item::Decl(name, decl) => {
                    // The prelude (source 0) is trusted plumbing: its signatures are used
                    // to type calls into it, but its bodies are not checked (they use
                    // forms whose typing rules land in later milestones) and its spans
                    // stay out of the user's type map.
                    if program.spans.get(name).is_some_and(|s| s.src == SrcId(0)) {
                        continue;
                    }
                    let decl_span = program.spans.get(name).copied().unwrap_or(Span::new(
                        SrcId::SYNTHETIC,
                        0,
                        0,
                    ));
                    let start = checker.begin_item();
                    match decl {
                        Decl::Fn(generics, params, ret, body) => {
                            checker.enter_generics(generics)?;
                            let mut ctx = Ctx::new(Some(ret.clone()), None);
                            for p in params {
                                checker.validate_type(&p.ty, p.span)?;
                                ctx = ctx.extend(p.name.clone(), p.ty.clone());
                            }
                            checker.validate_type(ret, decl_span)?;
                            checker
                                .check_expr(body, &ctx, Some(ret))
                                .map_err(|e| missing_result_type(e, name, ret, body))?;
                        }
                        // Globals were checked above, in initialization order.
                        Decl::Global(..) => continue,
                        Decl::Type(generics, variants) => {
                            checker.enter_generics(generics)?;
                            for v in variants {
                                for t in &v.fields {
                                    checker.validate_type(t, v.span)?;
                                }
                            }
                        }
                        Decl::Struct(generics, fields) => {
                            checker.enter_generics(generics)?;
                            for f in fields {
                                checker.validate_type(&f.ty, f.span)?;
                            }
                        }
                        Decl::Relation(tys) => {
                            checker.enter_generics(&Generics { params: Vec::new() })?;
                            for t in tys {
                                checker.validate_type(t, decl_span)?;
                                // Facts are deduplicated by equality, so every column
                                // is data: no function (or relation) at any depth.
                                if !checker.admits_eq(t, &mut HashSet::new()) {
                                    return Err(TyError::RelationColumn {
                                        name: name.clone(),
                                        ty: t.clone(),
                                        span: t.span.unwrap_or(decl_span),
                                    });
                                }
                            }
                        }
                        Decl::Trait(generics, sigs) => {
                            // `Self` is legitimately unresolved in a trait signature:
                            // validate with it standing for a type known to be fine.
                            checker.enter_generics(generics)?;
                            for sig in sigs {
                                let probe = Ty::unit();
                                for p in &sig.params {
                                    checker.validate_type(&p.ty.with_self(&probe), p.span)?;
                                }
                                checker.validate_type(&sig.ret.with_self(&probe), sig.span)?;
                            }
                        }
                    }
                    checker.require_resolved(start)?;
                    checker.discharge_bounds()?;
                }
                Item::Impl(imp) => {
                    if imp.span.src == SrcId(0) {
                        continue; // prelude impls are trusted (see the decls loop above)
                    }
                    checker.enter_generics(&imp.generics)?;
                    checker.validate_type(&imp.ty, imp.span)?;
                    let start = checker.begin_item();
                    for m in &imp.methods {
                        // In the signature, `Self` is the implemented type (T-Self).
                        let ret = m.ret.with_self(&imp.ty);
                        checker.validate_type(&ret, m.span)?;
                        let mut ctx = Ctx::new(Some(ret.clone()), Some(imp.ty.clone()));
                        if m.has_self {
                            ctx = ctx.extend("self".to_string(), imp.ty.clone());
                        }
                        for p in &m.params {
                            let pty = p.ty.with_self(&imp.ty);
                            checker.validate_type(&pty, p.span)?;
                            ctx = ctx.extend(p.name.clone(), pty);
                        }
                        checker
                            .check_expr(&m.body, &ctx, Some(&ret))
                            .map_err(|e| missing_result_type(e, &m.name, &ret, &m.body))?;
                    }
                    checker.require_resolved(start)?;
                    checker.discharge_bounds()?;
                }
                Item::Rule(rule) => {
                    checker.enter_generics(&Generics { params: Vec::new() })?;
                    if rule.span.src == SrcId(0) {
                        continue; // prelude rules are trusted (there are none today)
                    }
                    let start = checker.begin_item();
                    checker.check_rule(rule)?;
                    checker.require_resolved(start)?;
                    checker.discharge_bounds()?;
                }
            }
        }
        // Discharge the bound obligations now that inference has run: each
        // instantiated type parameter with a bound must have an `impl`.
        checker.discharge_bounds()?;
        // Resolve every recorded type, so a node typed before its inference
        // variables were solved still reads back as its final type.
        let resolved = checker
            .types
            .iter()
            .map(|(span, ty)| (*span, checker.resolve(ty)))
            .collect();
        Ok((resolved, checker.conversions))
    }

    /// Whether two impls of one trait for one head are instances the program
    /// cannot tell apart: at every position of their type arguments, either
    /// one side is a bare type parameter or both have the same head. Two
    /// instances are distinct when some position has two different concrete
    /// heads — `From<[T]>` and `From<Map<K, V>>` — so a single generic
    /// instance (`From<T>`) must be the only one.
    fn overlapping(a: &Impl, b: &Impl) -> bool {
        let (Some(ta), Some(tb)) = (&a.trait_, &b.trait_) else {
            return false;
        };
        if ta.name != tb.name
            || head_key(&a.ty) != head_key(&b.ty)
            || ta.args.len() != tb.args.len()
        {
            return false;
        }
        let head = |imp: &Impl, arg: &Ty| match &arg.kind {
            TyKind::Named(n, args)
                if args.is_empty() && imp.generics.params.iter().any(|p| p.name == *n) =>
            {
                None
            }
            // Concrete types with no dispatch head still tell instances apart.
            TyKind::Tuple(ts) => Some(format!("({})", ts.len())),
            TyKind::Fn(..) => Some("fn".to_string()),
            TyKind::Ref(_) => Some("ref".to_string()),
            _ => head_key(arg),
        };
        !ta.args
            .iter()
            .zip(&tb.args)
            .any(|(x, y)| matches!((head(a, x), head(b, y)), (Some(h), Some(k)) if h != k))
    }

    /// Why `imp` may not extend the type it names, if it may not. Dispatch is by
    /// **head**: a declared type or struct, or a built-in — `Int`, `Bool`,
    /// `String`, `()`, or the list head `[T]` — so a tuple, a function type, a
    /// reference, or a bare type parameter has no head to file the impl under. A
    /// head that takes type arguments may be extended at a particular instance
    /// (`impl Len for Seq<Int>`) or generically (`impl<T> Len for [T]`).
    fn impl_target_problem(imp: &Impl) -> Option<String> {
        let params: Vec<&Name> = imp.generics.params.iter().map(|p| &p.name).collect();
        let args: &[Ty] = match &imp.ty.kind {
            TyKind::Int | TyKind::Bool | TyKind::Str | TyKind::Unit => return None,
            TyKind::List(t) => std::slice::from_ref(t),
            TyKind::Named(n, args) => {
                if args.is_empty() && params.contains(&n) {
                    return Some(
                        "a bare type parameter has no head type to dispatch on; \
                     an impl extends a declared type, struct, or built-in type"
                            .to_string(),
                    );
                }
                args
            }
            TyKind::Tuple(_)
            | TyKind::Fn(..)
            | TyKind::Ref(_)
            | TyKind::SelfTy
            | TyKind::Meta(_) => {
                return Some(
                    "an impl extends a declared type, struct, or built-in type \
                 (`Int`, `Bool`, `String`, `()`, or a list `[T]`)"
                        .to_string(),
                )
            }
        };
        // The arguments may specialize the head (`impl Len for Seq<Int>`):
        // a call on a receiver of known type is resolved to the covering impl
        // when the program is checked, and coherence keeps instances apart.
        let _ = args;
        None
    }

    /// The `impl` blocks as a whole: **coherence** — one `impl` per trait
    /// instance and head, instances told apart by their arguments' outermost
    /// form, and one definition of a method name per head type, so dispatch
    /// by name is well defined — and **conformance** — an
    /// `impl Tr for τ` provides exactly the trait's methods, each with the
    /// trait's signature under `Self = τ` and the trait's type arguments.
    fn check_impls(checker: &mut Checker) -> Result<(), TyError> {
        let program = checker.program().clone();
        // One impl per (trait *with its arguments*, head): `From<A>` and
        // `From<B>` for one type are two impls — provided their instances
        // are told apart by the heads of their type arguments (below).
        let mut trait_impls: HashSet<(String, String)> = HashSet::new();
        // A method name belongs to one trait per head: the trait may provide
        // it at several instances, but a second trait, an inherent method, or
        // a built-in conformance of the same name on the head collides —
        // dispatch at run time is by head and name alone. The built-in
        // conformances are `Ord`'s and `Len`'s methods on the built-in heads.
        let mut trait_methods: HashMap<(String, Name), String> = BUILTIN_METHODS
            .iter()
            .map(|(head, m, tr)| (((*head).to_string(), (*m).to_string()), (*tr).to_string()))
            .collect();
        // The built-in conformances are impls the prelude already provides —
        // `Ord` for `Int`/`String`/`Bool`, `Len` for `String`/lists — with
        // their methods `cmp` and `length`; a program may not define them again.
        let mut methods: HashSet<(String, Name)> = HashSet::new();
        for imp in &program.impls {
            if imp.span.src == SrcId(0) {
                continue;
            }
            if let Some(reason) = Self::impl_target_problem(imp) {
                return Err(TyError::ImplTarget {
                    ty: imp.ty.clone(),
                    reason,
                    span: imp.ty.span.unwrap_or(imp.head),
                });
            }
            let key = head_key(&imp.ty).unwrap_or_default();
            // The impl's own bounds are what its type is judged under.
            checker.enter_generics(&imp.generics)?;
            checker.validate_type(&imp.ty, imp.span)?;
            if let Some(tr) = &imp.trait_ {
                // The structural traits have no impls; and an `Ord` type must
                // admit equality, so `T: Ord` implies `T: Eq`.
                if matches!(tr.name.as_str(), "Eq" | "Print") {
                    return Err(TyError::ImplTarget {
                        ty: imp.ty.clone(),
                        reason: format!(
                            "`{}` is satisfied by a type's shape and cannot be implemented",
                            tr.name
                        ),
                        span: tr.span,
                    });
                }
                if tr.name == "Ord" && !checker.admits_eq(&imp.ty, &mut HashSet::new()) {
                    return Err(TyError::NoEquality {
                        ty: imp.ty.clone(),
                        is_param: false,
                        span: imp.ty.span.unwrap_or(imp.head),
                    });
                }
                if builtin_conforms(&tr.name, Some(&key)) {
                    return Err(TyError::BuiltinImpl {
                        trait_: tr.name.clone(),
                        ty: imp.ty.clone(),
                        span: imp.head,
                    });
                }
                if !trait_impls.insert((trait_display(tr), key.clone())) {
                    return Err(TyError::DuplicateImpl {
                        trait_: trait_display(tr),
                        ty: imp.ty.clone(),
                        span: imp.head,
                    });
                }
            }
            for m in &imp.methods {
                let taken = match &imp.trait_ {
                    Some(tr) => {
                        methods.contains(&(key.clone(), m.name.clone()))
                            || trait_methods
                                .insert((key.clone(), m.name.clone()), tr.name.clone())
                                .is_some_and(|owner| owner != tr.name)
                    }
                    None => {
                        trait_methods.contains_key(&(key.clone(), m.name.clone()))
                            || !methods.insert((key.clone(), m.name.clone()))
                    }
                };
                if taken {
                    return Err(TyError::DuplicateMethod {
                        method: m.name.clone(),
                        ty: imp.ty.clone(),
                        span: m.sig,
                    });
                }
            }
            let Some(tr) = &imp.trait_ else {
                continue;
            };
            let Some(Decl::Trait(generics, sigs)) = program.decls.get(&tr.name) else {
                return Err(TyError::UnknownTrait {
                    name: tr.name.clone(),
                    span: tr.span,
                });
            };
            if generics.params.len() != tr.args.len() {
                return Err(TyError::TraitArity {
                    name: tr.name.clone(),
                    expected: generics.params.len(),
                    found: tr.args.len(),
                    span: tr.span,
                });
            }
            let map: HashMap<Name, Ty> = generics
                .params
                .iter()
                .map(|p| p.name.clone())
                .zip(tr.args.iter().cloned())
                .collect();
            for sig in sigs {
                let Some(m) = imp.methods.iter().find(|m| m.name == sig.name) else {
                    return Err(TyError::MissingMethod {
                        trait_: tr.name.clone(),
                        method: sig.name.clone(),
                        span: imp.head,
                    });
                };
                let expected: Vec<Ty> = sig
                    .params
                    .iter()
                    .map(|p| p.ty.substitute(&map).with_self(&imp.ty))
                    .collect();
                let found: Vec<Ty> = m.params.iter().map(|p| p.ty.with_self(&imp.ty)).collect();
                let expected_ret = sig.ret.substitute(&map).with_self(&imp.ty);
                let found_ret = m.ret.with_self(&imp.ty);
                if sig.has_self != m.has_self || expected != found || expected_ret != found_ret {
                    return Err(TyError::SignatureMismatch {
                        method: m.name.clone(),
                        expected: sig_text(&sig.name, sig.has_self, &expected, &expected_ret),
                        found: sig_text(&m.name, m.has_self, &found, &found_ret),
                        span: m.sig,
                    });
                }
            }
            for m in &imp.methods {
                if !sigs.iter().any(|s| s.name == m.name) {
                    return Err(TyError::NotInTrait {
                        trait_: tr.name.clone(),
                        method: m.name.clone(),
                        span: m.sig,
                    });
                }
            }
        }
        // Two impls of one trait for one head must be instances the program
        // can tell apart by the heads of their trait arguments.
        for (i, a) in program.impls.iter().enumerate() {
            for b in &program.impls[..i] {
                if a.span.src != SrcId(0) && Self::overlapping(a, b) {
                    return Err(TyError::OverlappingImpls {
                        trait_: a.trait_.as_ref().map(trait_display).unwrap_or_default(),
                        ty: a.ty.clone(),
                        span: a.head,
                    });
                }
            }
        }
        Ok(())
    }

    /// Check every bound obligation gathered so far: the type it constrains
    /// (once resolved) must have an `impl` of the trait, or be a type parameter
    /// of the current declaration carrying that bound. Run at the end of each
    /// declaration, while its bounds are in scope. An obligation whose type is
    /// still not determined cannot be answered and is an error, like any other
    /// undetermined type ("type annotation needed").
    fn discharge_bounds(&mut self) -> Result<(), TyError> {
        for (trait_, ty, span) in std::mem::take(&mut self.obligations) {
            let ty = self.resolve(&ty);
            // A bound raised in code that never runs has nothing to judge.
            if self.in_dead_code(span) {
                continue;
            }
            // A definite "no" (a function type, whatever is still unknown
            // inside it) is reported as such; an answer that would depend on
            // an unknown is an undetermined type.
            if self.bound_holds(&trait_, &ty) {
                if contains_meta(&ty) {
                    return Err(TyError::Ambiguous { span });
                }
                continue;
            }
            if !matches!(trait_.name.as_str(), "Eq" | "Print") {
                return Err(self.failing_bound(&trait_, &ty, span));
            }
            // `Eq` and `Print` are structural: satisfied by the type's shape
            // (or a type parameter's bound), never by an `impl`.
            let is_param = self.is_type_param(&ty);
            return Err(match trait_.name.as_str() {
                "Eq" => TyError::NoEquality { ty, is_param, span },
                "Print" => TyError::NotPrintable { ty, is_param, span },
                _ => TyError::UnsatisfiedBound {
                    trait_: trait_display(&trait_),
                    is_param,
                    ty,
                    span,
                },
            });
        }
        Ok(())
    }

    /// Whether `ty` satisfies trait `trait_`: a built-in conformance the checker
    /// knows (the prelude's magic — `Int`/`String`/`Bool` are `Ord`, `String`
    /// and `[T]` are `Len`), or a declared `impl trait_ for ty`.
    #[cfg_attr(not(feature = "m8"), allow(dead_code))] // evidence by `impl` counts from M8
    fn has_impl(&self, trait_: &TraitRef, ty: &Ty) -> bool {
        let key = head_key(ty);
        if trait_.args.is_empty() && builtin_conforms(&trait_.name, key.as_deref()) {
            return true;
        }
        let Some(key) = key else {
            return false;
        };
        self.program.impls.iter().any(|imp| {
            imp.trait_.as_ref().is_some_and(|t| t.name == trait_.name)
                && head_key(&imp.ty).as_deref() == Some(key.as_str())
                && self.impl_applies(imp, trait_, ty)
        })
    }

    /// Whether `imp` (whose trait name and head already match) is an impl of
    /// `trait_` for `ty`: its type and its trait arguments match `ty` and
    /// `trait_`'s arguments under one instantiation of its parameters, and
    /// every bound on those parameters holds there. So
    /// `impl<T: Show> Show for [T]` is an impl for `[Int]` only when `Int` is
    /// `Show`, and for `[fn(Int) -> Int]` not at all.
    #[cfg_attr(not(feature = "m8"), allow(dead_code))] // evidence by `impl` counts from M8
    fn impl_applies(&self, imp: &crate::ast::Impl, trait_: &TraitRef, ty: &Ty) -> bool {
        let params: Vec<Name> = imp.generics.params.iter().map(|p| p.name.clone()).collect();
        let mut sub = HashMap::new();
        if !match_ty(&imp.ty, &params, ty, &mut sub) {
            return false;
        }
        let Some(tr) = &imp.trait_ else {
            return false;
        };
        if tr.args.len() != trait_.args.len()
            || !tr
                .args
                .iter()
                .zip(&trait_.args)
                .all(|(pat, arg)| match_ty(pat, &params, &self.resolve(arg), &mut sub))
        {
            return false;
        }
        imp.generics
            .params
            .iter()
            .all(|p| match (&p.bound, sub.get(&p.name)) {
                (Some(b), Some(arg)) => self.bound_holds(b, arg),
                _ => true,
            })
    }

    /// Why `ty` fails the bound `trait_` (a trait with impls, not `Eq`/`Print`):
    /// the head implements it at several instances, or an impl that would
    /// cover it has a bound of its own that fails — reported at that inner
    /// bound, recursively — or no impl covers it at all.
    fn failing_bound(&self, trait_: &TraitRef, ty: &Ty, span: Span) -> TyError {
        let ty = self.resolve(ty);
        if self.instances(&trait_.name, &ty) > 1 {
            return TyError::AmbiguousInstance {
                trait_: trait_.name.clone(),
                ty,
                span,
            };
        }
        let key = head_key(&ty);
        for imp in &self.program.impls {
            let Some(tr) = &imp.trait_ else { continue };
            if tr.name != trait_.name || head_key(&imp.ty) != key {
                continue;
            }
            let params: Vec<Name> = imp.generics.params.iter().map(|p| p.name.clone()).collect();
            let mut sub = HashMap::new();
            let covers = match_ty(&imp.ty, &params, &ty, &mut sub)
                && tr.args.len() == trait_.args.len()
                && tr
                    .args
                    .iter()
                    .zip(&trait_.args)
                    .all(|(pat, arg)| match_ty(pat, &params, &self.resolve(arg), &mut sub));
            if !covers {
                continue;
            }
            for p in &imp.generics.params {
                if let (Some(b), Some(arg)) = (&p.bound, sub.get(&p.name)) {
                    if !self.bound_holds(b, arg) {
                        return match b.name.as_str() {
                            "Eq" => TyError::NoEquality {
                                ty: arg.clone(),
                                is_param: false,
                                span,
                            },
                            "Print" => TyError::NotPrintable {
                                ty: arg.clone(),
                                is_param: false,
                                span,
                            },
                            _ => self.failing_bound(b, arg, span),
                        };
                    }
                }
            }
        }
        TyError::UnsatisfiedBound {
            trait_: trait_display(trait_),
            is_param: self.is_type_param(&ty),
            ty,
            span,
        }
    }

    /// Whether `ty` satisfies the bound `trait_`: structurally for `Eq` and
    /// `Print`; for any other trait, a type parameter of the current
    /// declaration carrying that bound, a built-in conformance, or an `impl`
    /// whose own bounds hold. A type still unknown is taken to satisfy it —
    /// nothing pins it, so nothing can violate it.
    pub(crate) fn bound_holds(&self, trait_: &TraitRef, ty: &Ty) -> bool {
        let ty = self.resolve(ty);
        if matches!(ty.kind, TyKind::Meta(_)) {
            return true;
        }
        match trait_.name.as_str() {
            "Eq" => return self.admits_eq(&ty, &mut HashSet::new()),
            "Print" => return self.printable(&ty),
            _ => {}
        }
        if let TyKind::Named(tvar, args) = &ty.kind {
            if args.is_empty()
                && self.type_bounds.get(tvar).is_some_and(|b| {
                    b.name == trait_.name
                        && b.args.len() == trait_.args.len()
                        && b.args
                            .iter()
                            .zip(&trait_.args)
                            .all(|(x, y)| *x == self.resolve(y))
                })
            {
                return true;
            }
        }
        // Until M8 the bounded prelude functions are native over the built-in
        // types only, so a user `impl` is not evidence the runtime can honor.
        #[cfg(not(feature = "m8"))]
        {
            builtin_conforms(&trait_.name, head_key(&ty).as_deref())
        }
        // A call through the bound runs on the receiver's head alone, which
        // cannot tell two instances of the trait apart — so a type whose head
        // implements the trait at several instances meets no bound, whether
        // the bound is a call site's or an impl's own (as evidence).
        #[cfg(feature = "m8")]
        {
            if self.instances(&trait_.name, &ty) > 1 {
                return false;
            }
            self.has_impl(trait_, &ty)
        }
    }

    /// Convenience for tests and experiments: check one expression in `ctx`
    /// against `expected` (or infer, with `None`) using an empty program. It
    /// gathers only the obligations the expression itself raises (`Eq`,
    /// `Print`, a native's `Ord` / `Len`) and discharges them before returning.
    pub fn check_one(e: &Expr, ctx: &Ctx, expected: Option<&Ty>) -> Result<Ty, TyError> {
        let mut checker = Checker::new(&Program::default());
        let ty = checker.check_expr(e, ctx, expected)?;
        // The structural obligations (`==` on a type still being inferred).
        checker.discharge_bounds()?;
        Ok(checker.resolve(&ty))
    }
}

/// The trait methods the built-in conformances provide, by head-type key —
/// the checker's view of `prelude::builtin_method`.
const BUILTIN_METHODS: &[(&str, &str, &str)] = &[
    ("Int", "cmp", "Ord"),
    ("String", "cmp", "Ord"),
    ("Bool", "cmp", "Ord"),
    ("String", "length", "Len"),
    ("[]", "length", "Len"),
];

/// The built-in trait conformances (Appendix D, "Functions, methods, and
/// generics"): the prelude's ad-hoc overloading, which user code cannot
/// write. `key` is the type's head-type key.
pub(super) fn builtin_conforms(trait_: &str, key: Option<&str>) -> bool {
    matches!(
        (trait_, key),
        ("Ord", Some("Int" | "String" | "Bool")) | ("Len", Some("String" | "[]"))
    )
}

/// Match `pattern`, a written type whose names in `params` are variables,
/// against `ty`, recording what each variable stands for. Structural: a
/// variable matches anything (consistently), everything else must agree
/// node for node.
fn match_ty(pattern: &Ty, params: &[Name], ty: &Ty, sub: &mut HashMap<Name, Ty>) -> bool {
    match (&pattern.kind, &ty.kind) {
        (TyKind::Named(n, args), _) if args.is_empty() && params.contains(n) => match sub.get(n) {
            Some(prev) => prev == ty,
            None => {
                sub.insert(n.clone(), ty.clone());
                true
            }
        },
        (TyKind::Named(a, xs), TyKind::Named(b, ys)) => {
            a == b
                && xs.len() == ys.len()
                && xs
                    .iter()
                    .zip(ys.iter())
                    .all(|(x, y)| match_ty(x, params, y, sub))
        }
        (TyKind::Tuple(xs), TyKind::Tuple(ys)) => {
            xs.len() == ys.len()
                && xs
                    .iter()
                    .zip(ys.iter())
                    .all(|(x, y)| match_ty(x, params, y, sub))
        }
        (TyKind::List(x), TyKind::List(y)) | (TyKind::Ref(x), TyKind::Ref(y)) => {
            match_ty(x, params, y, sub)
        }
        (TyKind::Fn(xs, xr), TyKind::Fn(ys, yr)) => {
            xs.len() == ys.len()
                && xs
                    .iter()
                    .zip(ys.iter())
                    .all(|(x, y)| match_ty(x, params, y, sub))
                && match_ty(xr, params, yr, sub)
        }
        (TyKind::Int, TyKind::Int)
        | (TyKind::Bool, TyKind::Bool)
        | (TyKind::Str, TyKind::Str)
        | (TyKind::Unit, TyKind::Unit) => true,
        _ => false,
    }
}

/// A body checked against an *omitted* result type (`()` with no span) whose
/// value mismatches `()` is almost always a forgotten `-> T`; say so. Only
/// the body's value counts: a loop body or an assignment that is not `()`
/// is its own mismatch, which no `-> T` would mend.
fn missing_result_type(err: TyError, name: &str, ret: &Ty, body: &Expr) -> TyError {
    let mut value = Vec::new();
    value_spans(body, &mut value);
    return_operands(body, &mut value);
    match err {
        TyError::Mismatch {
            expected,
            found,
            span,
        } if ret.span.is_none() && expected == Ty::unit() && value.contains(&span) => {
            TyError::MissingResultType {
                name: name.to_string(),
                found,
                span,
            }
        }
        other => other,
    }
}

/// A method signature as it is written: `fn m(self, x: Int) -> T`.
fn sig_text(name: &str, has_self: bool, params: &[Ty], ret: &Ty) -> String {
    let mut parts: Vec<String> = Vec::new();
    if has_self {
        parts.push("self".to_string());
    }
    parts.extend(params.iter().map(|t| format!("_: {t}")));
    format!("fn {name}({}) -> {ret}", parts.join(", "))
}

/// The trait bound on each bounded type parameter of a declaration.
fn bounds_of(generics: &crate::ast::Generics) -> HashMap<Name, TraitRef> {
    generics
        .params
        .iter()
        .filter_map(|p| p.bound.as_ref().map(|b| (p.name.clone(), b.clone())))
        .collect()
}

/// `Tr` or `Tr<A, B>`, as written.
fn trait_display(tr: &TraitRef) -> String {
    if tr.args.is_empty() {
        tr.name.clone()
    } else {
        let args: Vec<String> = tr.args.iter().map(|a| a.to_string()).collect();
        format!("{}<{}>", tr.name, args.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctx_extends_and_shadows() {
        let root = Ctx::new(Some(Ty::int()), None);
        let a = root.extend("x".into(), Ty::bool());
        let b = a.extend("x".into(), Ty::str());
        assert_eq!(root.lookup("x"), None);
        assert_eq!(a.lookup("x"), Some(Ty::bool()));
        assert_eq!(b.lookup("x"), Some(Ty::str()));
        assert_eq!(b.ret(), Some(Ty::int()));
        assert_eq!(b.self_ty(), None);
    }

    #[test]
    fn metas_solve_and_resolve_transitively() {
        let mut c = Checker::new(&Program::default());
        let a = c.fresh_meta();
        let b = c.fresh_meta();
        let (TyKind::Meta(ia), TyKind::Meta(ib)) = (&a.kind, &b.kind) else {
            panic!()
        };
        c.solve(*ia, b.clone());
        c.solve(*ib, Ty::int());
        assert_eq!(c.resolve(&Ty::list(a)), Ty::list(Ty::int()));
    }

    #[test]
    fn types_compare_by_shape_not_span() {
        let written = Ty::at(TyKind::Int, Span::new(crate::ast::SrcId::SYNTHETIC, 3, 6));
        assert_eq!(written, Ty::int());
        assert_eq!(format!("{written:?}"), "Int");
    }
}

// ---- provided type-structure helpers (used by the driver above and by

// your `check.rs`; not part of the judgment you write) ----

/// A type's head-type key, for matching a value or receiver against an `impl`.
pub(super) fn head_key(ty: &Ty) -> Option<String> {
    match &ty.kind {
        TyKind::Named(n, _) => Some(n.clone()),
        TyKind::Int => Some("Int".to_string()),
        TyKind::Bool => Some("Bool".to_string()),
        TyKind::Str => Some("String".to_string()),
        TyKind::Unit => Some("()".to_string()),
        TyKind::List(_) => Some("[]".to_string()),
        _ => None,
    }
}

/// Whether `ty` still contains an inference variable.
pub(super) fn contains_meta(ty: &Ty) -> bool {
    match &ty.kind {
        TyKind::Meta(_) => true,
        TyKind::Tuple(ts) | TyKind::Named(_, ts) => ts.iter().any(contains_meta),
        TyKind::List(t) | TyKind::Ref(t) => contains_meta(t),
        TyKind::Fn(ps, r) => ps.iter().any(contains_meta) || contains_meta(r),
        _ => false,
    }
}

/// The spans where a body's value is produced: the body itself, a block's
/// tail, and the branches or arms of an `if` or `match` standing there — the
/// places where a mismatch against an omitted result type means the `-> T`
/// was forgotten.
pub(super) fn value_spans(e: &Expr, out: &mut Vec<Span>) {
    out.push(e.span());
    match e {
        Expr::Block(_, Some(tail), _) => value_spans(tail, out),
        Expr::If(_, then, els, _) => {
            value_spans(then, out);
            if let Some(e1) = els {
                value_spans(e1, out);
            }
        }
        Expr::Match(_, arms, _) => {
            for arm in arms {
                value_spans(&arm.body, out);
            }
        }
        _ => {}
    }
}

/// The operand spans of every `return` in `e` — also value positions of the
/// enclosing function — not descending into lambdas, whose `return` is their
/// own.
pub(super) fn return_operands(e: &Expr, out: &mut Vec<Span>) {
    let each = |es: &[Expr], out: &mut Vec<Span>| es.iter().for_each(|e1| return_operands(e1, out));
    match e {
        Expr::Return(e1, _) => {
            out.push(e1.span());
            return_operands(e1, out);
        }
        Expr::Lit(..) | Expr::Var(..) | Expr::Lambda(..) | Expr::Relation(..) => {}
        Expr::Unary(_, e1, _) | Expr::Proj(e1, _, _) | Expr::Field(e1, _, _) | Expr::Try(e1, _) => {
            return_operands(e1, out)
        }
        Expr::Binary(_, a, b, _)
        | Expr::Assign(a, b, _)
        | Expr::While(a, b, _)
        | Expr::For(_, a, b, _) => {
            return_operands(a, out);
            return_operands(b, out);
        }
        Expr::If(c, t, e2, _) => {
            return_operands(c, out);
            return_operands(t, out);
            if let Some(e2) = e2 {
                return_operands(e2, out);
            }
        }
        Expr::Block(stmts, tail, _) => {
            for s in stmts {
                match s {
                    Stmt::Let(_, _, e1, _) | Stmt::Expr(e1) => return_operands(e1, out),
                }
            }
            if let Some(t) = tail {
                return_operands(t, out);
            }
        }
        Expr::Call(f, args, _) => {
            return_operands(f, out);
            each(args, out);
        }
        Expr::Method(r, _, args, _) => {
            return_operands(r, out);
            each(args, out);
        }
        Expr::Match(s, arms, _) => {
            return_operands(s, out);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    return_operands(g, out);
                }
                return_operands(&arm.body, out);
            }
        }
        Expr::ForQuery(_, body, _) => return_operands(body, out),
        Expr::Tuple(es, _) | Expr::List(es, _) | Expr::Ctor(_, es, _) => each(es, out),
        Expr::Struct(_, fields, _) => fields.iter().for_each(|(_, e1)| return_operands(e1, out)),
    }
}

/// The number of nodes in a type.
fn ty_size(ty: &Ty) -> usize {
    1 + match &ty.kind {
        TyKind::Tuple(ts) | TyKind::Named(_, ts) => ts.iter().map(ty_size).sum(),
        TyKind::List(t) | TyKind::Ref(t) => ty_size(t),
        TyKind::Fn(ps, r) => ps.iter().map(ty_size).sum::<usize>() + ty_size(r),
        _ => 0,
    }
}

/// The two structural properties the checker asks of a type: whether it
/// admits equality (`==`, `contains`, relation columns) and whether it can be
/// printed (`print`, `println`, `to_string`). Both mean "data all the way
/// down"; they differ on a reference, which compares by identity but prints
/// its contents.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum DataUse {
    Eq,
    Print,
}

impl Checker {
    /// Whether `ty` is a bare type parameter of the declaration being checked.
    pub(super) fn is_type_param(&self, ty: &Ty) -> bool {
        matches!(&ty.kind, TyKind::Named(n, args) if args.is_empty() && self.type_params.contains(n))
    }

    /// T-Eq's side condition: the type is data. A struct or constructor type
    /// is looked through its declaration, generics instantiated, so a
    /// function anywhere inside is found. A type met again while its own
    /// components are being examined is taken to admit equality — a
    /// recursive type is data when everything else in it is. A reference
    /// compares by identity, whatever it holds. A type parameter admits
    /// equality under an `Eq` bound, or an `Ord` one (an `impl Ord` requires
    /// it).
    pub(super) fn admits_eq(&self, ty: &Ty, visiting: &mut HashSet<Name>) -> bool {
        self.is_data(ty, DataUse::Eq, visiting)
    }

    /// Whether a value of `ty` can be printed: as [`Checker::admits_eq`], but a
    /// reference prints what it holds, and a type parameter needs a `Print`
    /// bound.
    pub(super) fn printable(&self, ty: &Ty) -> bool {
        self.is_data(ty, DataUse::Print, &mut HashSet::new())
    }

    fn is_data(&self, ty: &Ty, use_: DataUse, visiting: &mut HashSet<Name>) -> bool {
        match &ty.kind {
            TyKind::Fn(..) => false,
            TyKind::Ref(t) => use_ == DataUse::Eq || self.is_data(t, use_, visiting),
            TyKind::Tuple(ts) => ts.iter().all(|t| self.is_data(t, use_, visiting)),
            TyKind::List(t) => self.is_data(t, use_, visiting),
            TyKind::Named(name, args) => match self.decl(name) {
                Some(Decl::Struct(generics, fields)) => {
                    let tys: Vec<Ty> = fields.iter().map(|f| f.ty.clone()).collect();
                    self.components_are_data(name, &generics.clone(), args, &tys, use_, visiting)
                }
                Some(Decl::Type(generics, variants)) => {
                    let tys: Vec<Ty> = variants.iter().flat_map(|v| v.fields.clone()).collect();
                    self.components_are_data(name, &generics.clone(), args, &tys, use_, visiting)
                }
                // A type parameter: only its bound can say.
                _ if args.is_empty() && self.type_params.contains(name) => matches!(
                    (use_, self.type_bounds.get(name).map(|b| b.name.as_str())),
                    (DataUse::Eq, Some("Eq" | "Ord")) | (DataUse::Print, Some("Print"))
                ),
                // A name that is neither declared nor a parameter names no
                // type (the validator rejects it); it is not data.
                _ => false,
            },
            _ => true,
        }
    }

    /// Whether every component type of `name`'s declaration is data once its
    /// generics are instantiated to `args`.
    fn components_are_data(
        &self,
        name: &Name,
        generics: &Generics,
        args: &[Ty],
        tys: &[Ty],
        use_: DataUse,
        visiting: &mut HashSet<Name>,
    ) -> bool {
        // Keyed by the *instantiated* type: `R<Int>` reaching `R<[Int]>` is
        // a new instance to examine, not "itself". A chain of instances that
        // never settles (a non-regular type) is cut off and taken not to be
        // data, which is the safe answer.
        // An instantiation that keeps growing (`Nest<(T, T)>` doubling at each
        // level) never settles; give up once the arguments are large.
        let args: Vec<Ty> = args.iter().map(|a| self.resolve(a)).collect();
        if args.iter().map(ty_size).sum::<usize>() > 256 || visiting.len() > 512 {
            return false;
        }
        let key = Ty::named(name.clone(), args.clone()).to_string();
        if !visiting.insert(key.clone()) {
            return true;
        }
        let map: HashMap<Name, Ty> = generics
            .params
            .iter()
            .map(|p| p.name.clone())
            .zip(args)
            .collect();
        let ok = tys
            .iter()
            .all(|t| self.is_data(&t.substitute(&map), use_, visiting));
        visiting.remove(&key);
        ok
    }
}
