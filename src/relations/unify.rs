//! Matching terms against facts — YOUR file.  \[student\]  (Milestone M6)
//!
//! The one operation the relations engine repeats: does this atom's argument
//! list match this fact, given what the variables are already bound to? It is
//! unification with one side ground, so there is no occurs check to fear —
//! the same algorithm shape as the type checker's [`Checker::unify`], over a
//! different signature: values and named variables instead of types and
//! inference variables.
//!
//! To list this milestone's holes:
//!
//! ```text
//! grep -rn 'todo_m6!' src
//! ```
//!
//! [`Checker::unify`]: crate::types::Checker::unify

use super::{Subst, Term};
use crate::interp::Value;

impl Subst {
    /// Match `pattern` (a generator's arguments) against `fact` (a tuple of
    /// the relation) under this substitution. On success return the extended
    /// substitution: each variable met for the first time is bound to the
    /// fact's value in that position, and one already bound must agree with
    /// it; a constant must equal the fact's value. `None` if any position
    /// disagrees, or the lengths differ. `self` is unchanged either way.
    pub fn unify(&self, pattern: &[Term], fact: &[Value]) -> Option<Subst> {
        let _ = (pattern, fact);
        todo_m6!("Subst::unify: match terms against a ground tuple, extending the substitution")
    }
}
