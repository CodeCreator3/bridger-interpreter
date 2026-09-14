//! Abstract syntax  [frozen — do not edit]
//!
//! The tree the parser hands you and the evaluator walks. Its shape follows the
//! book's *Abstract Syntax* chapter and Appendix D; every `Expr` variant is one
//! evaluation rule. You never construct these by hand in normal use — the parser
//! does — but the graded tests build them directly to exercise one rule at a time.
//!
//! The parser (`crate::parser`) resolves a few surface forms before you see them:
//! `ref x = e;` is `let x = ref e;`, a negated integer literal is a literal, a
//! call whose arguments contain a hole (`?x`) is a relation query, and a
//! rule body's `and`-chain is split into its conjuncts.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::rc::Rc;

/// Identifies one loaded source — the `prelude.brg`, a user file, or a string
/// parsed on its own. It indexes the interpreter's list of loaded sources, in
/// load order (the prelude is 0); [`SrcId::SYNTHETIC`] tags a span with no
/// source behind it — a hand-built test node, or one the runtime generates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SrcId(pub u32);

impl SrcId {
    /// A span drawn from no loaded source.
    pub const SYNTHETIC: SrcId = SrcId(u32::MAX);
}

/// A range `[start, end)` of bytes within a particular source ([`SrcId`]), for
/// diagnostics. Two spans are equal only when they name the same bytes of the
/// same source, so a span is a unique key for the node it came from even across
/// files — which the type checker's span-keyed side table (M5) relies on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub src: SrcId,
    pub start: usize,
    pub end: usize,
}

impl Span {
    /// A span over `[start, end)` in source `src`.
    pub fn new(src: SrcId, start: usize, end: usize) -> Span {
        Span { src, start, end }
    }
}

/// An identifier: a variable, function, field, type, or relation name.
pub type Name = String;

/// A literal, the same set the lexer produces.
#[derive(Debug, Clone, PartialEq)]
pub enum Lit {
    Int(i64),
    Bool(bool),
    Str(String),
    Unit,
}

/// A prefix operator: `-e`, `not e`, `ref e`, `deref e`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
    Ref,
    Deref,
}

/// A binary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod, // arithmetic
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge, // comparison
    And,
    Or,     // boolean
    Concat, // ++
    Cons,   // ::  (prepend to a list)
}

/// An expression. Every variant carries a [`Span`]; the evaluator's job is to
/// turn one of these into a `Value` (or a stuck error).
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// a literal value: `5`, `true`, `"hi"`, `()`
    Lit(Lit, Span),
    /// a variable reference, or the receiver `self`
    Var(Name, Span),
    /// a prefix operator: `-e`, `not e`, `ref e`, `deref e`
    Unary(UnOp, Box<Expr>, Span),
    /// a binary operator: `e + e`, `e == e`, `e and e`, `e ++ e`, `e :: e`
    Binary(BinOp, Box<Expr>, Box<Expr>, Span),
    /// a conditional: `if e { … } else { … }`, the else optional
    If(Box<Expr>, Box<Expr>, Option<Box<Expr>>, Span),
    /// a block: statements and an optional trailing value
    Block(Vec<Stmt>, Option<Box<Expr>>, Span),
    /// assignment through a reference: `e := e`
    Assign(Box<Expr>, Box<Expr>, Span),
    /// a function call: `e(e, …)`
    Call(Box<Expr>, Vec<Expr>, Span),
    /// a method call: `e.m(e, …)`
    Method(Box<Expr>, Name, Vec<Expr>, Span),
    /// a lambda: `|x, …| e`, each parameter optionally annotated `x: T`
    Lambda(Vec<LambdaParam>, Box<Expr>, Span),
    /// a pattern match: `match e { arm, … }`
    Match(Box<Expr>, Vec<Arm>, Span),
    /// a while loop: `while e { … }`
    While(Box<Expr>, Box<Expr>, Span),
    /// a for loop over a list: `for x in e { … }`
    For(Name, Box<Expr>, Box<Expr>, Span),
    /// a for loop over a relation's solutions: `for R(a, ?x) { … }`
    ForQuery(Query, Box<Expr>, Span),
    /// an early return from a function: `return e`
    Return(Box<Expr>, Span),
    /// error propagation: `e?` — sugar over `match` and `return` (Part VIII)
    Try(Box<Expr>, Span),
    /// a tuple: `(e, e, …)`, two or more elements
    Tuple(Vec<Expr>, Span),
    /// a list: `[e, …]`
    List(Vec<Expr>, Span),
    /// struct field access: `e.f`
    Field(Box<Expr>, Name, Span),
    /// tuple projection by index: `e.0`
    Proj(Box<Expr>, u32, Span),
    /// constructor application: `C(e, …)`
    Ctor(Name, Vec<Expr>, Span),
    /// struct literal: `S { f: e, … }`
    Struct(Name, Vec<(Name, Expr)>, Span),
    /// a relations operation: `add q` / `clear R` / `solutions q` / a query
    Relation(Rel, Span),
}

/// A statement inside a block.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// `let x = e`   (or   `let x: T = e`); the span covers the whole binding
    Let(Name, Option<Ty>, Expr, Span),
    /// an expression evaluated for effect
    Expr(Expr),
}

/// One arm of a `match`.
#[derive(Debug, Clone, PartialEq)]
pub struct Arm {
    /// the pattern this arm matches
    pub pat: Pattern,
    /// an optional `if` guard
    pub guard: Option<Expr>,
    /// evaluated when the arm is taken
    pub body: Expr,
}

/// A pattern, as it appears in a `match` arm. Every variant carries a [`Span`].
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    /// `_`
    Wild(Span),
    /// a literal: `5`, `true`, `"hi"`
    Lit(Lit, Span),
    /// binds the matched value to a name
    Var(Name, Span),
    /// a constructor: `C(p, …)`
    Ctor(Name, Vec<Pattern>, Span),
    /// `(p, p, …)`, two or more
    Tuple(Vec<Pattern>, Span),
    /// `[p, …]`, with an optional `...rest`
    List(Vec<Pattern>, Option<Name>, Span),
    /// `head :: tail`
    Cons(Box<Pattern>, Box<Pattern>, Span),
    /// `S { f: p, … }`
    Struct(Name, Vec<(Name, Pattern)>, Span),
    /// `p | p`
    Or(Box<Pattern>, Box<Pattern>, Span),
}

impl Pattern {
    /// The source range of this pattern.
    pub fn span(&self) -> Span {
        match self {
            Pattern::Wild(s)
            | Pattern::Lit(_, s)
            | Pattern::Var(_, s)
            | Pattern::Ctor(_, _, s)
            | Pattern::Tuple(_, s)
            | Pattern::List(_, _, s)
            | Pattern::Cons(_, _, s)
            | Pattern::Struct(_, _, s)
            | Pattern::Or(_, _, s) => *s,
        }
    }
}

/// A relations operation (Part VII).
#[derive(Debug, Clone, PartialEq)]
pub enum Rel {
    /// `add R(a, …)`
    Add(Query),
    /// `clear R`
    Clear(Name),
    /// `solutions R(a, …)`
    Solutions(Query),
    /// a bare query `R(a, …)`
    Query(Query),
}

/// A query atom `R(a, …)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    pub name: Name,
    pub args: Vec<QArg>,
}

/// One argument of a [`Query`].
#[derive(Debug, Clone, PartialEq)]
pub enum QArg {
    /// a ground argument
    Expr(Expr),
    /// a hole: `?x` (named) or `?` (anonymous)
    Hole(Option<Name>),
}

/// A whole program: top-level declarations plus the relation and impl tables.
/// The top level is order-free with unique names, so declarations are a map.
/// One namespace: a function and a relation cannot share a name.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Program {
    /// `fn`, a top-level `let`/`ref` (`Decl::Global`), `type`, `struct`,
    /// `trait`, `relation`
    pub decls: HashMap<Name, Decl>,
    /// where each declaration in `decls` was written, for diagnostics
    pub spans: HashMap<Name, Span>,
    /// anonymous impl blocks: keyed by the type they extend
    pub impls: Vec<Impl>,
    /// rules: any number define one relation
    pub rules: Vec<Rule>,
}

/// A top-level declaration.
#[derive(Debug, Clone, PartialEq)]
pub enum Decl {
    /// `fn f<T: C>(x: T, …) -> T = e`
    Fn(Generics, Vec<Param>, Ty, Box<Expr>),
    /// `let x = e`   (or   `let x: T = e`)
    Global(Option<Ty>, Box<Expr>),
    /// `type T<U> = C(U, …) | …`
    Type(Generics, Vec<Variant>),
    /// `struct S<T> { f: T, … }`
    Struct(Generics, Vec<Field>),
    /// `trait Tr<T> { fn m(…); … }`
    Trait(Generics, Vec<Sig>),
    /// `relation R : (T, …)`
    Relation(Vec<Ty>),
}

/// A generic parameter list `<T, U: Show, …>`; empty when absent.
#[derive(Debug, Clone, PartialEq)]
pub struct Generics {
    pub params: Vec<TyParam>,
}

/// One generic parameter, with an optional trait bound.
#[derive(Debug, Clone, PartialEq)]
pub struct TyParam {
    /// `T`
    pub name: Name,
    /// `: Show` — the trait bound, if any
    pub bound: Option<TraitRef>,
    /// where it is written, for diagnostics
    pub span: Span,
}

/// A function or method parameter `x: T`.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: Name,
    pub ty: Ty,
    pub span: Span,
}

/// A lambda parameter `x` or `x: T`. The annotation is optional: the checker
/// (Part VI) infers a lambda's parameter types from the expected function type
/// or from the parameter's uses, and the annotation covers what neither reaches.
#[derive(Debug, Clone, PartialEq)]
pub struct LambdaParam {
    pub name: Name,
    pub ty: Option<Ty>,
    pub span: Span,
}

/// A struct field `f: T`.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub name: Name,
    pub ty: Ty,
    pub span: Span,
}

/// A type variant `C(T, …)`, or `C` with no fields.
#[derive(Debug, Clone, PartialEq)]
pub struct Variant {
    pub name: Name,
    pub fields: Vec<Ty>,
    pub span: Span,
}

/// A method signature (no body), as it appears in a `trait`.
#[derive(Debug, Clone, PartialEq)]
pub struct Sig {
    pub name: Name,
    /// whether the method takes `self`
    pub has_self: bool,
    pub params: Vec<Param>,
    /// `()`, when the return type is omitted
    pub ret: Ty,
    pub span: Span,
}

/// A [`Sig`] with a body, as it appears in an `impl`.
#[derive(Debug, Clone, PartialEq)]
pub struct Method {
    pub name: Name,
    pub has_self: bool,
    pub params: Vec<Param>,
    pub ret: Ty,
    pub body: Expr,
    /// the whole method, body included
    pub span: Span,
    /// the signature alone, `fn name(…) -> T`, for diagnostics about it
    pub sig: Span,
}

/// An `impl` block: methods attached to a type, optionally for a trait.
#[derive(Debug, Clone, PartialEq)]
pub struct Impl {
    /// `impl<T> …`
    pub generics: Generics,
    /// the trait implemented, if any
    pub trait_: Option<TraitRef>,
    /// the type being extended
    pub ty: Ty,
    pub methods: Vec<Method>,
    /// the whole block
    pub span: Span,
    /// the header alone, `impl<T> Tr for Ty`, for diagnostics about it
    pub head: Span,
}

/// A reference to a trait: `Tr`, or `Tr<T, …>` when applied.
#[derive(Debug, Clone, PartialEq)]
pub struct TraitRef {
    pub name: Name,
    pub args: Vec<Ty>,
    /// where it is written, for diagnostics
    pub span: Span,
}

/// A Datalog rule `rule H :- B1 and …`. The body is the `and`-chain split
/// into its conjuncts, each an ordinary expression. Which of them are
/// **generators** (a call whose callee names a declared relation: `edge(x, y)`,
/// arguments variables or literals, binds logic variables) and which are
/// **filters** (any other pure test: `x != z + 1`, `is_ok(x)`) is decided by
/// name resolution when the rules are installed (Part VII), not by the parser —
/// `edge(x, y)` and `is_ok(x)` are the same syntax.
#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    pub head: Atom,
    pub body: Vec<Expr>,
    pub span: Span,
}

/// A rule head `R(t, …)`: a relation name over terms.
#[derive(Debug, Clone, PartialEq)]
pub struct Atom {
    pub name: Name,
    pub terms: Vec<Term>,
    pub span: Span,
}

/// A term in a rule head.
#[derive(Debug, Clone, PartialEq)]
pub enum Term {
    /// a logic variable (a lowercase name)
    Var(Name),
    /// a literal
    Lit(Lit),
}

/// A type: the shape, plus where it was written when it came from an
/// annotation. A type the checker computes has no span. Two types are equal
/// when their shapes are, wherever they came from — so a `Ty` can be compared
/// and matched as if it were just its [`TyKind`], and the span is there only
/// for a diagnostic to say "declared `Int` here".
#[derive(Clone)]
pub struct Ty {
    pub kind: TyKind,
    pub span: Option<Span>,
}

/// The shape of a type, as written in an annotation and inferred by the
/// checker (Part VI).
#[derive(Debug, Clone, PartialEq)]
pub enum TyKind {
    Int,
    Bool,
    Str,
    Unit,
    /// `(T, U, …)` — two or more
    Tuple(Rc<[Ty]>),
    /// `[T]`
    List(Rc<Ty>),
    /// `fn(T, …) -> T`
    Fn(Rc<[Ty]>, Rc<Ty>),
    /// `ref<T>`
    Ref(Rc<Ty>),
    /// `Self`, in a trait or impl
    SelfTy,
    /// `N`, or `N<T, …>` when applied
    Named(Name, Rc<[Ty]>),
    /// a type not yet known: an inference variable the checker solves by
    /// unification (Part VI). Never written in a program; the parser never
    /// produces one.
    Meta(u32),
}

impl PartialEq for Ty {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
    }
}

impl std::fmt::Debug for Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.kind.fmt(f)
    }
}

/// A type in Bridger's own syntax — `Int`, `[String]`, `fn(Int) -> Bool`,
/// `Option<Int>`, `ref<Int>` — for diagnostics. An unsolved inference
/// variable prints as `_`.
impl std::fmt::Display for Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fn list(f: &mut std::fmt::Formatter<'_>, ts: &[Ty]) -> std::fmt::Result {
            for (i, t) in ts.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{t}")?;
            }
            Ok(())
        }
        match &self.kind {
            TyKind::Int => write!(f, "Int"),
            TyKind::Bool => write!(f, "Bool"),
            TyKind::Str => write!(f, "String"),
            TyKind::Unit => write!(f, "()"),
            TyKind::Tuple(ts) => {
                write!(f, "(")?;
                list(f, ts)?;
                write!(f, ")")
            }
            TyKind::List(t) => write!(f, "[{t}]"),
            TyKind::Fn(ps, r) => {
                write!(f, "fn(")?;
                list(f, ps)?;
                write!(f, ") -> {r}")
            }
            TyKind::Ref(t) => write!(f, "ref<{t}>"),
            TyKind::SelfTy => write!(f, "Self"),
            TyKind::Named(n, args) if args.is_empty() => write!(f, "{n}"),
            TyKind::Named(n, args) => {
                write!(f, "{n}<")?;
                list(f, args)?;
                write!(f, ">")
            }
            TyKind::Meta(_) => write!(f, "_"),
        }
    }
}

impl From<TyKind> for Ty {
    /// A computed type: a shape with no source position.
    fn from(kind: TyKind) -> Ty {
        Ty { kind, span: None }
    }
}

/// Constructors for computed types, so the checker and the evaluator can write
/// `Ty::int()` or `Ty::list(t)` rather than spell out the struct.
impl Ty {
    /// A type as written in the source at `span`.
    pub fn at(kind: TyKind, span: Span) -> Ty {
        Ty {
            kind,
            span: Some(span),
        }
    }
    pub fn int() -> Ty {
        TyKind::Int.into()
    }
    pub fn bool() -> Ty {
        TyKind::Bool.into()
    }
    pub fn str() -> Ty {
        TyKind::Str.into()
    }
    pub fn unit() -> Ty {
        TyKind::Unit.into()
    }
    pub fn tuple(ts: Vec<Ty>) -> Ty {
        TyKind::Tuple(Rc::from(ts)).into()
    }
    pub fn list(t: Ty) -> Ty {
        TyKind::List(Rc::new(t)).into()
    }
    pub fn func(params: Vec<Ty>, ret: Ty) -> Ty {
        TyKind::Fn(Rc::from(params), Rc::new(ret)).into()
    }
    pub fn reference(t: Ty) -> Ty {
        TyKind::Ref(Rc::new(t)).into()
    }
    pub fn self_ty() -> Ty {
        TyKind::SelfTy.into()
    }
    pub fn named(name: Name, args: Vec<Ty>) -> Ty {
        TyKind::Named(name, Rc::from(args)).into()
    }
    pub fn meta(id: u32) -> Ty {
        TyKind::Meta(id).into()
    }
}

impl Ty {
    /// This type with every `Self` replaced by `self_ty` — what a method
    /// signature written inside `impl … for T` means with `T` for `Self`.
    pub fn with_self(&self, self_ty: &Ty) -> Ty {
        let kind = match &self.kind {
            TyKind::SelfTy => return self_ty.clone(),
            TyKind::Tuple(ts) => TyKind::Tuple(ts.iter().map(|t| t.with_self(self_ty)).collect()),
            TyKind::List(t) => TyKind::List(Rc::new(t.with_self(self_ty))),
            TyKind::Ref(t) => TyKind::Ref(Rc::new(t.with_self(self_ty))),
            TyKind::Fn(ps, r) => TyKind::Fn(
                ps.iter().map(|t| t.with_self(self_ty)).collect(),
                Rc::new(r.with_self(self_ty)),
            ),
            TyKind::Named(n, args) => TyKind::Named(
                n.clone(),
                args.iter().map(|t| t.with_self(self_ty)).collect(),
            ),
            _ => return self.clone(),
        };
        Ty {
            kind,
            span: self.span,
        }
    }
}

impl Ty {
    /// This type with each type parameter named in `map` replaced by its
    /// argument — a trait's `T` by the `impl Tr<Int>`'s `Int`, say.
    pub fn substitute(&self, map: &HashMap<Name, Ty>) -> Ty {
        let kind = match &self.kind {
            TyKind::Named(n, args) if args.is_empty() => match map.get(n) {
                Some(t) => return t.clone(),
                None => return self.clone(),
            },
            TyKind::Named(n, args) => {
                TyKind::Named(n.clone(), args.iter().map(|t| t.substitute(map)).collect())
            }
            TyKind::Tuple(ts) => TyKind::Tuple(ts.iter().map(|t| t.substitute(map)).collect()),
            TyKind::List(t) => TyKind::List(Rc::new(t.substitute(map))),
            TyKind::Ref(t) => TyKind::Ref(Rc::new(t.substitute(map))),
            TyKind::Fn(ps, r) => TyKind::Fn(
                ps.iter().map(|t| t.substitute(map)).collect(),
                Rc::new(r.substitute(map)),
            ),
            _ => return self.clone(),
        };
        Ty {
            kind,
            span: self.span,
        }
    }
}

/// A dependency cycle among global initializers: no order can initialize
/// `names` because each needs the next, and the last needs the first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitCycle {
    /// the globals on the cycle, each depending on the next (and the last on
    /// the first)
    pub names: Vec<Name>,
    /// where the first global on the cycle is declared
    pub span: Span,
}

/// The names a global initializer can reach, gathered by [`Program::refs_of`].
#[derive(Default)]
struct Refs {
    globals: BTreeSet<Name>,
    fns: BTreeSet<Name>,
    methods: BTreeSet<Name>,
    /// whether a query is evaluated, which runs every rule body
    queries: bool,
}

impl Program {
    /// The order in which the global initializers must run: a topological
    /// order of their dependency graph, where global `g` depends on every
    /// global its initializer can reach — directly, or through the body of any
    /// function it calls, any method of that name (dispatch is dynamic, so all
    /// are assumed reachable), or any rule body when it evaluates a query. The
    /// order is deterministic: among independent globals, alphabetical.
    ///
    /// A cycle means no order exists, and is the returned [`InitCycle`]. This
    /// is the static check behind the language's "definitions in any order":
    /// a global may name a later global, as long as no global (transitively)
    /// needs its own value.
    pub fn initialization_order(&self) -> Result<Vec<Name>, InitCycle> {
        let deps: BTreeMap<&Name, BTreeSet<Name>> = self
            .decls
            .iter()
            .filter(|(_, d)| matches!(d, Decl::Global(..)))
            .map(|(name, _)| (name, self.global_deps(name)))
            .collect();
        // Depth-first search; a global met again while still on the stack
        // closes a cycle, reported from its first occurrence on the stack.
        let mut done: HashSet<&Name> = HashSet::new();
        let mut stack: Vec<&Name> = Vec::new();
        let mut on_stack: HashSet<&Name> = HashSet::new();
        let mut order = Vec::new();
        for &g in deps.keys() {
            self.visit(g, &deps, &mut done, &mut stack, &mut on_stack, &mut order)?;
        }
        Ok(order)
    }

    fn visit<'a>(
        &'a self,
        g: &'a Name,
        deps: &BTreeMap<&'a Name, BTreeSet<Name>>,
        done: &mut HashSet<&'a Name>,
        stack: &mut Vec<&'a Name>,
        on_stack: &mut HashSet<&'a Name>,
        order: &mut Vec<Name>,
    ) -> Result<(), InitCycle> {
        if done.contains(g) {
            return Ok(());
        }
        // The set answers "on the stack?" at once; the stack itself is
        // scanned only to report a cycle, so a long chain sorts in linear time.
        if on_stack.contains(g) {
            let i = stack.iter().position(|n| *n == g).unwrap_or(0);
            let names: Vec<Name> = stack[i..].iter().map(|n| (*n).clone()).collect();
            let span =
                self.spans
                    .get(&names[0])
                    .copied()
                    .unwrap_or(Span::new(SrcId::SYNTHETIC, 0, 0));
            return Err(InitCycle { names, span });
        }
        stack.push(g);
        on_stack.insert(g);
        if let Some(ds) = deps.get(g) {
            for d in ds {
                // `d` is a global, so it is a key of `deps`
                let d = deps.get_key_value(d).map(|(k, _)| *k).unwrap_or(g);
                self.visit(d, deps, done, stack, on_stack, order)?;
            }
        }
        stack.pop();
        on_stack.remove(g);
        done.insert(g);
        order.push(g.clone());
        Ok(())
    }

    /// The span of the first top-level expression — a function body, a global
    /// initializer, a method body, or a rule body — nested more deeply than
    /// `limit`, if any.
    ///
    /// The interpreter evaluates by recursion and the passes over the tree
    /// (`Program::refs_of`, the type checker) recurse too, so a tree deeper
    /// than the machine stack allows would abort the process with no
    /// diagnostic. [`crate::interp::Interpreter::eval_program`] calls this
    /// first, at [`crate::types::MAX_NESTING`] — the same bound the type
    /// checker enforces from M5, so the limit is one number and every
    /// milestone agrees. The walk here is iterative, so measuring the depth
    /// cannot itself overflow.
    pub fn too_deeply_nested(&self, limit: usize) -> Option<Span> {
        let bodies = self
            .decls
            .values()
            .filter_map(|d| match d {
                Decl::Fn(_, _, _, body) | Decl::Global(_, body) => Some(&**body),
                _ => None,
            })
            .chain(
                self.impls
                    .iter()
                    .flat_map(|imp| imp.methods.iter().map(|m| &m.body)),
            )
            .chain(self.rules.iter().flat_map(|r| r.body.iter()));
        bodies
            .into_iter()
            .find(|root| root.nesting_exceeds(limit))
            .map(Expr::span)
    }

    /// Every global that global `g`'s initializer can reach (see
    /// [`Program::initialization_order`]).
    fn global_deps(&self, g: &Name) -> BTreeSet<Name> {
        let Some(Decl::Global(_, init)) = self.decls.get(g) else {
            return BTreeSet::new();
        };
        let mut refs = Refs::default();
        self.refs_of(init, &mut Vec::new(), &mut refs);
        let mut seen_fns: HashSet<Name> = HashSet::new();
        let mut seen_methods: HashSet<Name> = HashSet::new();
        let mut rules_done = false;
        loop {
            let fns = std::mem::take(&mut refs.fns);
            let methods = std::mem::take(&mut refs.methods);
            let run_rules = refs.queries && !rules_done;
            if fns.is_empty() && methods.is_empty() && !run_rules {
                return refs.globals;
            }
            for f in fns {
                if !seen_fns.insert(f.clone()) {
                    continue;
                }
                if let Some(Decl::Fn(_, params, _, body)) = self.decls.get(&f) {
                    let mut bound: Vec<Name> = params.iter().map(|p| p.name.clone()).collect();
                    self.refs_of(body, &mut bound, &mut refs);
                }
            }
            for m in methods {
                if !seen_methods.insert(m.clone()) {
                    continue;
                }
                for meth in self.impls.iter().flat_map(|imp| &imp.methods) {
                    if meth.name == m {
                        let mut bound: Vec<Name> =
                            meth.params.iter().map(|p| p.name.clone()).collect();
                        bound.push("self".to_string());
                        self.refs_of(&meth.body, &mut bound, &mut refs);
                    }
                }
            }
            if run_rules {
                rules_done = true;
                for e in self.rules.iter().flat_map(|r| &r.body) {
                    self.refs_of(e, &mut Vec::new(), &mut refs);
                }
            }
        }
    }

    /// Gather into `out` the top-level names `e` refers to, ignoring names
    /// bound locally (`bound` is the scope stack: `let`, parameters, patterns,
    /// loop variables, query holes).
    fn refs_of(&self, e: &Expr, bound: &mut Vec<Name>, out: &mut Refs) {
        match e {
            Expr::Lit(..) => {}
            Expr::Var(x, _) => {
                if !bound.contains(x) {
                    match self.decls.get(x) {
                        Some(Decl::Global(..)) => {
                            out.globals.insert(x.clone());
                        }
                        Some(Decl::Fn(..)) => {
                            out.fns.insert(x.clone());
                        }
                        _ => {}
                    }
                }
            }
            // `e?` converts through `From::from` on the way out, so a global
            // read inside any `impl From`'s `from` is a dependency too.
            Expr::Try(e, _) => {
                out.methods.insert("from".to_string());
                self.refs_of(e, bound, out)
            }
            Expr::Unary(_, e, _)
            | Expr::Return(e, _)
            | Expr::Field(e, _, _)
            | Expr::Proj(e, _, _) => self.refs_of(e, bound, out),
            Expr::Binary(_, a, b, _) | Expr::Assign(a, b, _) | Expr::While(a, b, _) => {
                self.refs_of(a, bound, out);
                self.refs_of(b, bound, out);
            }
            Expr::If(c, t, els, _) => {
                self.refs_of(c, bound, out);
                self.refs_of(t, bound, out);
                if let Some(e) = els {
                    self.refs_of(e, bound, out);
                }
            }
            Expr::Block(stmts, tail, _) => {
                let depth = bound.len();
                for s in stmts {
                    match s {
                        Stmt::Let(x, _, e, _) => {
                            self.refs_of(e, bound, out);
                            bound.push(x.clone());
                        }
                        Stmt::Expr(e) => self.refs_of(e, bound, out),
                    }
                }
                if let Some(t) = tail {
                    self.refs_of(t, bound, out);
                }
                bound.truncate(depth);
            }
            Expr::Call(f, args, _) => {
                // A call whose callee names a relation is a query in call
                // syntax (`path(0, 3)`), so it reads the rules.
                if let Expr::Var(name, _) = &**f {
                    if !bound.contains(name)
                        && matches!(self.decls.get(name), Some(Decl::Relation(_)))
                    {
                        out.queries = true;
                    }
                }
                self.refs_of(f, bound, out);
                for a in args {
                    self.refs_of(a, bound, out);
                }
            }
            Expr::Method(recv, m, args, _) => {
                out.methods.insert(m.clone());
                self.refs_of(recv, bound, out);
                for a in args {
                    self.refs_of(a, bound, out);
                }
            }
            Expr::Lambda(params, body, _) => {
                let depth = bound.len();
                bound.extend(params.iter().map(|p| p.name.clone()));
                self.refs_of(body, bound, out);
                bound.truncate(depth);
            }
            Expr::Match(scrut, arms, _) => {
                self.refs_of(scrut, bound, out);
                for arm in arms {
                    let depth = bound.len();
                    pattern_binders(&arm.pat, bound);
                    if let Some(g) = &arm.guard {
                        self.refs_of(g, bound, out);
                    }
                    self.refs_of(&arm.body, bound, out);
                    bound.truncate(depth);
                }
            }
            Expr::For(x, iter, body, _) => {
                self.refs_of(iter, bound, out);
                let depth = bound.len();
                bound.push(x.clone());
                self.refs_of(body, bound, out);
                bound.truncate(depth);
            }
            Expr::ForQuery(q, body, _) => {
                out.queries = true;
                let depth = bound.len();
                for a in &q.args {
                    match a {
                        QArg::Expr(e) => self.refs_of(e, bound, out),
                        QArg::Hole(Some(x)) => bound.push(x.clone()),
                        QArg::Hole(None) => {}
                    }
                }
                self.refs_of(body, bound, out);
                bound.truncate(depth);
            }
            Expr::Tuple(es, _) | Expr::List(es, _) | Expr::Ctor(_, es, _) => {
                for e in es {
                    self.refs_of(e, bound, out);
                }
            }
            Expr::Struct(_, fields, _) => {
                for (_, e) in fields {
                    self.refs_of(e, bound, out);
                }
            }
            Expr::Relation(rel, _) => {
                let q = match rel {
                    Rel::Clear(_) => return,
                    Rel::Add(q) => q,
                    Rel::Solutions(q) | Rel::Query(q) => {
                        out.queries = true;
                        q
                    }
                };
                for a in &q.args {
                    if let QArg::Expr(e) = a {
                        self.refs_of(e, bound, out);
                    }
                }
            }
        }
    }
}

impl Expr {
    /// The source range of this expression.
    pub fn span(&self) -> Span {
        match self {
            Expr::Lit(_, s)
            | Expr::Var(_, s)
            | Expr::Unary(_, _, s)
            | Expr::Binary(_, _, _, s)
            | Expr::If(_, _, _, s)
            | Expr::Block(_, _, s)
            | Expr::Assign(_, _, s)
            | Expr::Call(_, _, s)
            | Expr::Method(_, _, _, s)
            | Expr::Lambda(_, _, s)
            | Expr::Match(_, _, s)
            | Expr::While(_, _, s)
            | Expr::For(_, _, _, s)
            | Expr::ForQuery(_, _, s)
            | Expr::Return(_, s)
            | Expr::Try(_, s)
            | Expr::Tuple(_, s)
            | Expr::List(_, s)
            | Expr::Field(_, _, s)
            | Expr::Proj(_, _, s)
            | Expr::Ctor(_, _, s)
            | Expr::Struct(_, _, s)
            | Expr::Relation(_, s) => *s,
        }
    }

    /// Whether some root-to-leaf chain of sub-expressions is longer than
    /// `limit` nodes. Iterative (an explicit stack), so it cannot itself
    /// overflow on the very trees it is meant to catch. See
    /// [`Program::too_deeply_nested`].
    pub fn nesting_exceeds(&self, limit: usize) -> bool {
        // `depth` is the count of expression nodes above `node`.
        let mut stack: Vec<(&Expr, usize)> = vec![(self, 0)];
        while let Some((node, depth)) = stack.pop() {
            if depth > limit {
                return true;
            }
            node.push_children(&mut stack, depth + 1);
        }
        false
    }

    /// Push each immediate sub-expression onto `stack`, tagged with depth `d`.
    fn push_children<'a>(&'a self, stack: &mut Vec<(&'a Expr, usize)>, d: usize) {
        let mut push = |e: &'a Expr| stack.push((e, d));
        match self {
            Expr::Lit(..) | Expr::Var(..) => {}
            Expr::Unary(_, e, _)
            | Expr::Return(e, _)
            | Expr::Field(e, _, _)
            | Expr::Proj(e, _, _)
            | Expr::Try(e, _) => push(e),
            Expr::Binary(_, a, b, _) | Expr::Assign(a, b, _) | Expr::While(a, b, _) => {
                push(a);
                push(b);
            }
            Expr::If(c, t, els, _) => {
                push(c);
                push(t);
                if let Some(e) = els {
                    push(e);
                }
            }
            Expr::Block(stmts, tail, _) => {
                for s in stmts {
                    match s {
                        Stmt::Let(_, _, e, _) | Stmt::Expr(e) => push(e),
                    }
                }
                if let Some(t) = tail {
                    push(t);
                }
            }
            Expr::Call(f, args, _) => {
                push(f);
                args.iter().for_each(&mut push);
            }
            Expr::Method(recv, _, args, _) => {
                push(recv);
                args.iter().for_each(&mut push);
            }
            Expr::Lambda(_, body, _) => push(body),
            Expr::Match(scrut, arms, _) => {
                push(scrut);
                for arm in arms {
                    if let Some(g) = &arm.guard {
                        push(g);
                    }
                    push(&arm.body);
                }
            }
            Expr::For(_, iter, body, _) => {
                push(iter);
                push(body);
            }
            Expr::ForQuery(q, body, _) => {
                for a in &q.args {
                    if let QArg::Expr(e) = a {
                        push(e);
                    }
                }
                push(body);
            }
            Expr::Tuple(es, _) | Expr::List(es, _) | Expr::Ctor(_, es, _) => {
                es.iter().for_each(&mut push);
            }
            Expr::Struct(_, fields, _) => {
                for (_, e) in fields {
                    push(e);
                }
            }
            Expr::Relation(rel, _) => {
                let q = match rel {
                    Rel::Clear(_) => return,
                    Rel::Add(q) | Rel::Solutions(q) | Rel::Query(q) => q,
                };
                for a in &q.args {
                    if let QArg::Expr(e) = a {
                        push(e);
                    }
                }
            }
        }
    }
}

impl Pattern {
    /// Every name this pattern binds, in order of appearance.
    pub fn binders(&self) -> Vec<Name> {
        let mut out = Vec::new();
        pattern_binders(self, &mut out);
        out
    }
}

/// Push every name `pat` binds onto `bound`.
fn pattern_binders(pat: &Pattern, bound: &mut Vec<Name>) {
    match pat {
        Pattern::Wild(_) | Pattern::Lit(..) => {}
        Pattern::Var(x, _) => bound.push(x.clone()),
        Pattern::Ctor(_, ps, _) | Pattern::Tuple(ps, _) => {
            for p in ps {
                pattern_binders(p, bound);
            }
        }
        Pattern::List(ps, rest, _) => {
            for p in ps {
                pattern_binders(p, bound);
            }
            if let Some(r) = rest {
                bound.push(r.clone());
            }
        }
        Pattern::Cons(a, b, _) | Pattern::Or(a, b, _) => {
            pattern_binders(a, bound);
            pattern_binders(b, bound);
        }
        Pattern::Struct(_, fields, _) => {
            for (_, p) in fields {
                pattern_binders(p, bound);
            }
        }
    }
}

/// `a → b → a`, for a cycle's error message.
pub fn cycle_text(names: &[Name]) -> String {
    let mut parts: Vec<&str> = names.iter().map(String::as_str).collect();
    if let Some(first) = names.first() {
        parts.push(first);
    }
    parts.join(" → ")
}
