//! Static type errors  [frozen — do not edit]  (comes alive at M5)

use crate::ast::{Name, Span, Ty};
use thiserror::Error;

/// A static type error (Part VI): the checker rejected the program before it
/// ran. One variant per way a program can be ill-typed; each carries the
/// [`Span`] of the node it blames. As with [`RuntimeError`](crate::interp::RuntimeError), tests match the
/// variant and span, never the message.
#[derive(Debug, Error)]
pub enum TyError {
    #[error("type mismatch: expected {expected}, found {found}")]
    Mismatch { expected: Ty, found: Ty, span: Span },

    #[error("unbound variable `{name}`{}", keyword_hint(.name))]
    UnboundVariable { name: Name, span: Span },

    #[error("unknown type `{name}`")]
    UnknownType { name: Name, span: Span },

    #[error("unknown constructor `{name}`")]
    UnknownConstructor { name: Name, span: Span },

    /// a struct, type, or trait name used as a constructor
    #[error("`{name}` is a type, not a constructor")]
    NotAConstructor { name: Name, span: Span },

    /// a type or trait name used as a struct in a literal or pattern
    #[error("`{name}` is not a struct")]
    NotAStruct { name: Name, span: Span },

    #[error("no such field `{field}`")]
    NoSuchField { field: String, span: Span },

    /// `e.f` on a value that is not a struct
    #[error("`.{field}` reads a struct's field, but this has type {found}")]
    FieldOfNonStruct {
        field: String,
        found: Ty,
        span: Span,
    },

    /// `e.i` past the end of a tuple
    #[error("a tuple of {arity} components has no `.{index}`")]
    NoSuchComponent {
        index: u32,
        arity: usize,
        span: Span,
    },

    /// `e.i` on a value that is not a tuple
    #[error("`.{index}` selects a component of a tuple, but this has type {found}")]
    NotATuple { index: u32, found: Ty, span: Span },

    #[error("no such method `{method}`")]
    NoSuchMethod { method: String, span: Span },

    #[error("value of type {found} is not a function")]
    NotAFunction { found: Ty, span: Span },

    #[error("`{callee}` takes {expected} argument(s), given {found}")]
    ArityMismatch {
        callee: String,
        expected: usize,
        found: usize,
        span: Span,
    },

    /// `is_param`: the type is a bare type parameter of the declaration, so
    /// a bound would grant it
    #[error("type {ty} admits no equality")]
    NoEquality { ty: Ty, is_param: bool, span: Span },

    /// `fn main` declared with parameters or type parameters.
    #[error("`main` takes no parameters and no type parameters")]
    MainSignature { span: Span },

    #[error("`return` outside a function")]
    ReturnOutsideFunction { span: Span },

    #[error("match is not exhaustive")]
    NonExhaustiveMatch { span: Span },

    /// An expression nested past the checker's bound.
    #[error("the expression is nested more than {limit} levels deep")]
    TooDeep { limit: usize, span: Span },

    #[error("the type of this expression cannot be determined; add an annotation")]
    Ambiguous { span: Span },

    /// a query, `add`, `clear`, or rule names a relation no declaration
    /// introduces
    #[error("`{name}` is not a declared relation")]
    UndeclaredRelation { name: Name, span: Span },

    /// a `fn` or method with no `->` type, whose body is not `()`
    #[error("`{name}` declares no result type, so its body must be `()`, but it is {found}")]
    MissingResultType { name: Name, found: Ty, span: Span },

    /// an `if` with no `else` used for a value
    #[error("an `if` without `else` has type `()`, but this branch has type {found}")]
    IfWithoutElse { found: Ty, span: Span },

    /// `?` on something that is neither `Option` nor `Result`
    #[error("`?` applies to an `Option` or `Result`, but this has type {found}")]
    TryOnNonCarrier { found: Ty, span: Span },

    /// `?` inside a function whose result type cannot carry the error out
    #[error("`?` would return the failure from the enclosing function, but its result type {ret} cannot carry it")]
    TryInNonCarrierFunction { ret: Ty, span: Span },

    /// `e.f(…)` where `f` is a field of the struct, not a method
    #[error("`{field}` is a field of {ty}, not a method")]
    FieldNotMethod { field: Name, ty: String, span: Span },

    /// `?` in a lambda whose result type was being inferred: the `?` made
    /// it a carrier, and the body is not one
    #[error("`?` returns from the enclosing lambda, so its body must produce an `Option` or `Result`, but it produces {found}")]
    TryInLambda { found: Ty, span: Span },

    /// a trait applied to the wrong number of type arguments
    #[error("trait `{name}` takes {expected} type argument(s), given {found}")]
    TraitArity {
        name: Name,
        expected: usize,
        found: usize,
        span: Span,
    },

    /// a type applied to the wrong number of type arguments
    #[error("type `{name}` takes {expected} type argument(s), given {found}")]
    TypeArity {
        name: Name,
        expected: usize,
        found: usize,
        span: Span,
    },

    /// an `impl` of a trait a built-in type already satisfies by rule
    #[error("`{ty}` is `{trait_}` by a built-in rule and cannot be implemented again")]
    BuiltinImpl { trait_: Name, ty: Ty, span: Span },

    /// a method provided by more than one impl for the receiver's type (two
    /// instances of a generic trait), so a direct call cannot choose
    #[error("method `{method}` is provided by more than one impl for this type, and the arguments do not choose one")]
    AmbiguousMethod { method: Name, span: Span },

    /// two impls of one trait for one head whose instances are not told apart
    /// by the heads of their type arguments
    #[error("`impl {trait_} for {ty}` overlaps another impl of the trait for the same type")]
    OverlappingImpls { trait_: String, ty: Ty, span: Span },

    /// a bound met by a type that implements the trait at several instances,
    /// which a call through the bound could not choose between at run time
    #[error("{ty} implements `{trait_}` at more than one instance, so a call through the bound cannot choose one")]
    AmbiguousInstance { trait_: Name, ty: Ty, span: Span },

    #[error("cannot construct the infinite type")]
    InfiniteType { span: Span },

    #[error("relation `{name}` takes {expected} argument(s), given {found}")]
    RelationArity {
        name: Name,
        expected: usize,
        found: usize,
        span: Span,
    },

    /// An `impl` whose target has no head type to dispatch on — a type
    /// parameter, a tuple, a function, or a reference type — or of `Eq` or
    /// `Print`, which no `impl` provides.
    #[error("`impl` for {ty}: {reason}")]
    ImplTarget { ty: Ty, reason: String, span: Span },

    /// An associated function (no `self`) called on a value; `is_param`:
    /// the receiver's type is a type parameter, which has no type to call
    /// it on.
    #[error("`{method}` takes no `self`, so it is not called on a value")]
    NotAMethod {
        method: Name,
        ty: String,
        is_param: bool,
        span: Span,
    },

    /// A method (with `self`) called on a type.
    #[error("`{method}` takes `self`; call it on a value of the type")]
    NoReceiver { method: Name, span: Span },

    /// An associated call through a type parameter (`T.default()`), which the
    /// evaluator cannot dispatch: types are not passed at run time.
    #[error("`{name}` is a type parameter; an associated call names a concrete type")]
    TypeParamCall { name: Name, span: Span },

    /// A value of a type that cannot be printed (a function somewhere inside
    /// it) passed to `print`, `println`, or `to_string`.
    #[error("type {ty} cannot be printed")]
    NotPrintable { ty: Ty, is_param: bool, span: Span },

    /// A struct literal that omits a declared field.
    #[error("struct literal is missing the field `{field}`")]
    MissingField { field: String, span: Span },

    /// A relation column whose type does not admit equality (a function
    /// somewhere inside it): facts are deduplicated by equality, so a column
    /// must be data.
    #[error("relation `{name}` has a column of type {ty}, which does not admit equality")]
    RelationColumn { name: Name, ty: Ty, span: Span },

    /// `is_param`: the type is a bare type parameter of the declaration, so
    /// a bound would grant it
    #[error("no `impl {trait_} for {ty}`, so the bound is not satisfied")]
    UnsatisfiedBound {
        trait_: Name,
        ty: Ty,
        is_param: bool,
        span: Span,
    },

    #[error("global initializers form a cycle: {}", crate::ast::cycle_text(.names))]
    InitializationCycle { names: Vec<Name>, span: Span },

    #[error("field `{field}` is given more than once")]
    DuplicateField { field: String, span: Span },

    #[error("pattern binds `{name}` more than once")]
    NonLinearPattern { name: Name, span: Span },

    #[error("the alternatives of `|` must bind `{name}` alike, on both sides and at one type")]
    OrPatternBindings { name: Name, span: Span },

    #[error("unknown trait `{name}`")]
    UnknownTrait { name: Name, span: Span },

    /// a generic parameter with the name of a declared or built-in type
    #[error("type parameter `{name}` has the name of a type")]
    TypeParamShadows { name: Name, span: Span },

    #[error("`impl {trait_} for {ty}` is given more than once")]
    DuplicateImpl { trait_: Name, ty: Ty, span: Span },

    #[error("method `{method}` is defined more than once for {ty}")]
    DuplicateMethod { method: Name, ty: Ty, span: Span },

    #[error("`impl {trait_}` lacks the trait's method `{method}`")]
    MissingMethod {
        trait_: Name,
        method: Name,
        span: Span,
    },

    #[error("`{method}` has signature `{found}`, but the trait declares `{expected}`")]
    SignatureMismatch {
        method: Name,
        expected: String,
        found: String,
        span: Span,
    },

    #[error("trait `{trait_}` declares no method `{method}`")]
    NotInTrait {
        trait_: Name,
        method: Name,
        span: Span,
    },

    #[error("`add` takes a ground fact; a `?` hole belongs in a query")]
    HoleInAdd { span: Span },

    /// A query form names something bound to other than a relation (a local
    /// shadows the relation).
    #[error("`{name}` is not a relation here")]
    NotARelation { name: Name, span: Span },

    /// A relation name used where a value is expected.
    #[error("`{name}` is a relation, which is not a value")]
    RelationNotAValue { name: Name, span: Span },
}

/// A hint when a contextual keyword was used as a name in the wrong shape.
fn keyword_hint(name: &str) -> &'static str {
    crate::parser::keyword_hint(name)
}

/// Whether an `impl` may extend the type: a built-in head or a declared name,
/// as opposed to a tuple, a function, a reference, or a type parameter.
fn can_be_extended(ty: &Ty) -> bool {
    use crate::ast::TyKind;
    matches!(
        ty.kind,
        TyKind::Int
            | TyKind::Bool
            | TyKind::Str
            | TyKind::Unit
            | TyKind::List(_)
            | TyKind::Named(..)
    )
}

impl TyError {
    /// Where the error is: the span of the node it blames.
    pub fn span(&self) -> Span {
        match self {
            TyError::Mismatch { span, .. }
            | TyError::UnboundVariable { span, .. }
            | TyError::UnknownType { span, .. }
            | TyError::UnknownConstructor { span, .. }
            | TyError::NotAConstructor { span, .. }
            | TyError::NotAStruct { span, .. }
            | TyError::NoSuchField { span, .. }
            | TyError::FieldOfNonStruct { span, .. }
            | TyError::NoSuchComponent { span, .. }
            | TyError::NotATuple { span, .. }
            | TyError::NoSuchMethod { span, .. }
            | TyError::NotAFunction { span, .. }
            | TyError::ArityMismatch { span, .. }
            | TyError::NoEquality { span, .. }
            | TyError::ReturnOutsideFunction { span, .. }
            | TyError::MainSignature { span, .. }
            | TyError::NonExhaustiveMatch { span, .. }
            | TyError::Ambiguous { span, .. }
            | TyError::TooDeep { span, .. }
            | TyError::UndeclaredRelation { span, .. }
            | TyError::MissingResultType { span, .. }
            | TyError::IfWithoutElse { span, .. }
            | TyError::TryOnNonCarrier { span, .. }
            | TyError::TryInNonCarrierFunction { span, .. }
            | TyError::FieldNotMethod { span, .. }
            | TyError::TryInLambda { span, .. }
            | TyError::TraitArity { span, .. }
            | TyError::TypeArity { span, .. }
            | TyError::BuiltinImpl { span, .. }
            | TyError::AmbiguousMethod { span, .. }
            | TyError::OverlappingImpls { span, .. }
            | TyError::AmbiguousInstance { span, .. }
            | TyError::InfiniteType { span, .. }
            | TyError::RelationArity { span, .. }
            | TyError::ImplTarget { span, .. }
            | TyError::NotAMethod { span, .. }
            | TyError::NoReceiver { span, .. }
            | TyError::TypeParamCall { span, .. }
            | TyError::NotPrintable { span, .. }
            | TyError::MissingField { span, .. }
            | TyError::RelationColumn { span, .. }
            | TyError::UnsatisfiedBound { span, .. }
            | TyError::InitializationCycle { span, .. }
            | TyError::DuplicateField { span, .. }
            | TyError::NonLinearPattern { span, .. }
            | TyError::OrPatternBindings { span, .. }
            | TyError::UnknownTrait { span, .. }
            | TyError::TypeParamShadows { span, .. }
            | TyError::DuplicateImpl { span, .. }
            | TyError::DuplicateMethod { span, .. }
            | TyError::MissingMethod { span, .. }
            | TyError::SignatureMismatch { span, .. }
            | TyError::NotInTrait { span, .. }
            | TyError::HoleInAdd { span, .. }
            | TyError::NotARelation { span, .. }
            | TyError::RelationNotAValue { span, .. } => *span,
        }
    }

    /// A suggestion to print under the message, for the errors that have one.
    pub fn help(&self) -> Option<String> {
        Some(match self {
            TyError::Ambiguous { .. } => {
                "write the type on the binding, the parameter, or the literal, \
                 e.g. `let xs: [Int] = [];`"
                    .to_string()
            }
            TyError::UndeclaredRelation { name, .. } => {
                format!("declare it first: `relation {name} : (…);`")
            }
            TyError::MissingResultType { name, found, .. } => {
                format!("write `fn {name}(…) -> {found}`")
            }
            TyError::IfWithoutElse { .. } => {
                "add an `else` branch, or end this branch with a `;` so it is `()`".to_string()
            }
            TyError::TryInNonCarrierFunction { .. } => {
                "give the function a result type of the same kind, `Option` or `Result`, or \
                 handle the failure here with `match`"
                    .to_string()
            }
            TyError::NotAMethod {
                method,
                ty,
                is_param: false,
                ..
            } => format!("call it on the type, `{ty}.{method}(…)`"),
            TyError::NotAMethod { is_param: true, .. } => {
                "a type parameter has no type to call it on; call it on a concrete type at the \
                 call site and pass the result in"
                    .to_string()
            }
            TyError::FieldNotMethod { field, .. } => {
                format!("a function held in a field is called as `(e.{field})(…)`")
            }
            TyError::NoEquality {
                ty, is_param: true, ..
            } => format!("add the bound `{ty}: Eq` (or `{ty}: Ord`) to the declaration"),
            TyError::NotPrintable {
                ty, is_param: true, ..
            } => format!("add the bound `{ty}: Print` to the declaration"),
            TyError::UnsatisfiedBound {
                trait_,
                ty,
                is_param: true,
                ..
            } => format!("add the bound `{ty}: {trait_}` to the declaration"),
            TyError::UnsatisfiedBound { trait_, ty, .. }
                if cfg!(not(feature = "m8")) && can_be_extended(ty) =>
            {
                format!("before M8 an `impl {trait_}` of your own is not evidence for a bound")
            }
            // A bare constructor name: D, "a constructor name is not a value".
            TyError::ArityMismatch {
                callee, found: 0, ..
            } if callee.starts_with(|c: char| c.is_ascii_uppercase()) => format!(
                "a constructor is not a value; to pass one on, write a lambda, `|x| {callee}(x)`"
            ),
            TyError::NotAConstructor { name, .. } => format!(
                "a struct is built with braces, `{name} {{ … }}`; a `type` is built with one of \
                 its constructors"
            ),
            TyError::NotAStruct { name, .. } => {
                format!("`{name}` is a `type`: build or match it with one of its constructors")
            }
            TyError::UnknownType { name, .. } if name == "Self" => {
                "`Self` is available only inside an `impl` or `trait`".to_string()
            }
            TyError::TryInLambda { .. } => {
                "wrap the result, `|x| Some(x?)`, or handle the failure with `match`".to_string()
            }
            TyError::UnboundVariable { name, .. } if name == "self" => {
                "`self` is available only inside a method declared with `self`".to_string()
            }
            TyError::ReturnOutsideFunction { .. } => {
                "`return` and `?` need an enclosing `fn` or method to return from".to_string()
            }
            _ => return None,
        })
    }
}
