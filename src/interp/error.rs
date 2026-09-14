//! Errors and control flow  [frozen — do not edit]
//!
//! `eval_expr` returns `Result<Value, Control>`. The `Err` channel is not only
//! for errors: it carries the *two* ways evaluation leaves a rule early, so a
//! single `?` threads both outward with no per-arm bookkeeping.

use crate::ast::{Name, Span, Ty};
use crate::interp::Value;
use thiserror::Error;

/// A non-local exit from evaluation.
///
/// `?` propagates either variant. A [`Control::Raise`] bubbles all the way to
/// the top level and halts the run; a [`Control::Return`] is caught at the
/// enclosing call boundary (the `Call` arm, M4), which unwraps it into the
/// call's result. Until `return` lands (M3) the only `Control` you build is a
/// `Raise` — and the [`From`] impl below lets you write that as `?` on a plain
/// [`RuntimeError`].
#[derive(Debug)]
pub enum Control {
    /// a stuck evaluation: no rule applies
    Raise(RuntimeError),
    /// an early `return e`, headed for the enclosing function
    Return(Value),
}

/// A stuck evaluation: one variant per way a Bridger program can get stuck, each
/// carrying the [`Span`] of the node that got stuck. Graded tests match the
/// **variant and its span**, never the rendered message, so the wording here is
/// for humans and free to change.
#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("unbound variable `{name}`{}", keyword_hint(.name))]
    UnboundVariable { name: Name, span: Span },

    #[error("type error: expected {expected}, found {found}")]
    TypeError { expected: Ty, found: Ty, span: Span },

    #[error("division by zero")]
    DivByZero { span: Span },

    #[error("no such field `{field}`")]
    NoSuchField { field: String, span: Span },

    #[error("struct literal is missing the field `{field}`")]
    MissingField { field: String, span: Span },

    #[error("no such method `{method}`")]
    NoSuchMethod { method: String, span: Span },

    /// `main` is missing, is not a function, or takes parameters or type
    /// parameters; checked before anything runs.
    #[error("{reason}")]
    Main { reason: String, span: Span },

    /// A query form (`add`, `clear`, `solutions`, `for`, a holed query) whose
    /// name is bound to something other than a relation.
    #[error("`{name}` is not a relation here")]
    NotARelation { name: Name, span: Span },

    /// An associated function (no `self`) called on a value.
    #[error("`{method}` takes no `self`; call it on its type, `{ty}.{method}(…)`")]
    NotAMethod {
        method: String,
        ty: String,
        span: Span,
    },

    /// A method (with `self`) called on a type.
    #[error("`{method}` takes `self`; call it on a value of the type")]
    NoReceiver { method: String, span: Span },

    /// A rule filter asked a query while the fixpoint was being computed —
    /// a relation call the purity check should have rejected.
    #[error("the relation database was read or changed inside a rule filter")]
    QueryInFilter { span: Span },

    /// a `return` (or `?`) with no function to return from — in a rule
    /// filter or a global initializer; a static error from M5, a stuck state
    /// when unchecked
    #[error("`return` outside a function")]
    ReturnOutsideFunction { span: Span },

    /// a struct literal or pattern naming something that is not a struct
    #[error("`{name}` is not a struct")]
    NotAStruct { name: Name, span: Span },

    /// more nested calls than the interpreter allows (runaway recursion)
    #[error("call depth exceeded {limit} frames")]
    StackOverflow { limit: usize, span: Span },

    /// an expression nested more deeply than the interpreter can evaluate by
    /// recursion — rejected before the program runs. From M5 the type checker
    /// reaches the same tree first and reports it as [`crate::types::TyError`]
    /// `TooDeep`; this is the unchecked milestones' guard, at the same limit.
    #[error("this expression is nested more deeply than {limit} levels")]
    TooDeep { limit: usize, span: Span },

    /// a `range` that would build more elements than the interpreter allows
    #[error("range of {len} elements is too large (the limit is {limit})")]
    RangeTooLarge { len: i128, limit: usize, span: Span },

    #[error("struct literal names the field `{field}` twice")]
    DuplicateField { field: String, span: Span },

    #[error("a value of type {found} is not a function")]
    NotAFunction { found: Ty, span: Span },

    #[error("takes {expected} argument(s), given {found}")]
    ArityMismatch {
        expected: usize,
        found: usize,
        span: Span,
    },

    #[error("no match arm applied")]
    NonExhaustiveMatch { span: Span },

    #[error("type {found} admits no equality")]
    NotComparable { found: Ty, span: Span },

    /// `cmp` on a built-in `Ord` receiver (`Int`, `String`, `Bool`) with an
    /// argument of another type — until M8 raised by the native `min`/`max`/
    /// `minimum`/`maximum`, from M8 by the `cmp` they call.
    #[error("type {found} has no order")]
    NotOrdered { found: Ty, span: Span },

    /// `add r(?x)`: a fact is ground; a hole belongs in a query.
    #[error("`add` takes a ground fact; a hole `?` belongs in a query")]
    HoleInAdd { span: Span },

    #[error("type {found} cannot be printed")]
    NotPrintable { found: Ty, span: Span },

    #[error("{}", input_error_text(.expected, .found.as_deref()))]
    InputError {
        expected: String,
        /// the next token, or `None` at end of input
        found: Option<String>,
        span: Span,
    },

    #[error("global initializers form a cycle: {}", crate::ast::cycle_text(.names))]
    InitializationCycle { names: Vec<Name>, span: Span },
}

/// Lets an arm build a plain [`RuntimeError`] and `?` it (or `return
/// Err(e.into())`): the error is lifted into [`Control::Raise`].
impl From<RuntimeError> for Control {
    fn from(e: RuntimeError) -> Self {
        Control::Raise(e)
    }
}

/// A hint when a contextual keyword was used as a name in the wrong shape.
fn keyword_hint(name: &str) -> &'static str {
    crate::parser::keyword_hint(name)
}

impl RuntimeError {
    /// Where the error is: the span of the node it blames.
    pub fn span(&self) -> Span {
        match self {
            RuntimeError::UnboundVariable { span, .. }
            | RuntimeError::TypeError { span, .. }
            | RuntimeError::DivByZero { span, .. }
            | RuntimeError::NoSuchField { span, .. }
            | RuntimeError::MissingField { span, .. }
            | RuntimeError::NoSuchMethod { span, .. }
            | RuntimeError::Main { span, .. }
            | RuntimeError::NotARelation { span, .. }
            | RuntimeError::NotAMethod { span, .. }
            | RuntimeError::NoReceiver { span, .. }
            | RuntimeError::QueryInFilter { span, .. }
            | RuntimeError::ReturnOutsideFunction { span, .. }
            | RuntimeError::NotAStruct { span, .. }
            | RuntimeError::StackOverflow { span, .. }
            | RuntimeError::TooDeep { span, .. }
            | RuntimeError::RangeTooLarge { span, .. }
            | RuntimeError::DuplicateField { span, .. }
            | RuntimeError::NotAFunction { span, .. }
            | RuntimeError::ArityMismatch { span, .. }
            | RuntimeError::NonExhaustiveMatch { span, .. }
            | RuntimeError::NotComparable { span, .. }
            | RuntimeError::NotOrdered { span, .. }
            | RuntimeError::HoleInAdd { span, .. }
            | RuntimeError::NotPrintable { span, .. }
            | RuntimeError::InputError { span, .. }
            | RuntimeError::InitializationCycle { span, .. } => *span,
        }
    }
}

fn input_error_text(expected: &str, found: Option<&str>) -> String {
    match found {
        None => format!("input ended; expected {expected}"),
        Some(tok) => format!("the next input token `{tok}` is not {expected}"),
    }
}

impl RuntimeError {
    /// A suggestion to print under the message, for the errors that have one.
    pub fn help(&self) -> Option<String> {
        Some(match self {
            RuntimeError::StackOverflow { limit, .. } => format!(
                "the recursion has no base case, or is deeper than the interpreter's \
                 limit of {limit} frames; an accumulating loop needs no stack"
            ),
            RuntimeError::InputError { found: None, .. } => {
                "the program reads more input than it was given".to_string()
            }
            RuntimeError::DivByZero { .. } => {
                "test the divisor first, or return an `Option`/`Result`".to_string()
            }
            _ => return None,
        })
    }
}
