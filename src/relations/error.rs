//! Errors in a program's rules  [frozen — do not edit]  (comes alive at M6)
//!
//! Raised by the static checks on rule bodies (`relations/check.rs`), which
//! `Interpreter::check` runs after loading and before `main`. Each carries the
//! [`Span`] of the conjunct, term, or head it blames.

use crate::ast::{Name, Span};
use thiserror::Error;

/// A rule the checks reject.
#[derive(Debug, Error)]
pub enum RuleError {
    #[error("`{name}` is not a declared relation")]
    NotARelation { name: Name, span: Span },

    #[error("relation `{name}` takes {expected} argument(s), given {found}")]
    Arity {
        name: Name,
        expected: usize,
        found: usize,
        span: Span,
    },

    #[error("a relation's argument in a rule body must be a variable or a literal")]
    ArgumentNotATerm { span: Span },

    #[error("logic variable `{var}` is not bound by any generator in this rule")]
    Unbound { var: Name, span: Span },

    /// `witness` is the chain of calls from the filter to the effect it
    /// reaches, outermost first — functions and locals by name, methods as
    /// `H.m` or `.m`, last the impure primitive or queried relation; empty
    /// when the effect is a form of the filter's own (`add`, `clear`,
    /// `solutions`, `for q`, `while`, `ref`, `deref`, `:=`, an applied lambda).
    #[error("this filter is not pure")]
    Impure { span: Span, witness: Vec<Name> },

    #[error("`{name}` is recursive, so a filter calling it might not terminate")]
    Recursive { name: Name, span: Span },

    #[error("`{name}` takes a function-typed parameter, which a filter or guard cannot follow")]
    HigherOrder { name: Name, span: Span },

    /// A `match` guard that is not pure (M7); `witness` as for [`RuleError::Impure`].
    #[error("this match guard is not pure")]
    ImpureGuard { span: Span, witness: Vec<Name> },
}

impl RuleError {
    /// Where the error is: the span of the node it blames.
    pub fn span(&self) -> Span {
        match self {
            RuleError::NotARelation { span, .. }
            | RuleError::Arity { span, .. }
            | RuleError::ArgumentNotATerm { span, .. }
            | RuleError::Unbound { span, .. }
            | RuleError::Impure { span, .. }
            | RuleError::Recursive { span, .. }
            | RuleError::HigherOrder { span, .. }
            | RuleError::ImpureGuard { span, .. } => *span,
        }
    }
}

impl RuleError {
    /// A suggestion to print under the message, for the errors that have one.
    pub fn help(&self) -> Option<String> {
        Some(match self {
            RuleError::NotARelation { name, .. } => {
                format!("declare it first, `relation {name} : (…);`, or define `fn {name}`")
            }
            RuleError::Unbound { .. } => {
                "a rule sees only the variables its generators bind and the top-level \
                 names; a filter tests, it binds nothing"
                    .to_string()
            }
            RuleError::Impure { witness, .. } | RuleError::ImpureGuard { witness, .. } => {
                let path = if witness.is_empty() {
                    "the effect is in this expression itself".to_string()
                } else {
                    format!("the effect is reached through `{}`", witness.join("` → `"))
                };
                format!(
                    "{path}; only reads of its variables, top-level constants, and calls \
                     to pure functions are allowed here"
                )
            }
            RuleError::ArgumentNotATerm { .. } => {
                "a generator's arguments are logic variables or literals; compute with a \
                 filter, `… and x == y + 1` binds nothing but tests"
                    .to_string()
            }
            _ => return None,
        })
    }
}
