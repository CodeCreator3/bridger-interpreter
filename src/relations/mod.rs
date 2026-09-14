//! The relations sublanguage  [frozen — do not edit]  (comes alive at M6)
//!
//! Everything the Datalog fragment (Part VII) needs beside the evaluator, in
//! the same shape as the other passes — provided plumbing, and the files you
//! write:
//!
//!  - [`RelationDb`] (`db.rs`): the dynamic facts `add` inserts and `clear`
//!    removes. Provided.
//!  - [`Term`] and [`Subst`] (`term.rs`): the terms a rule body is matched
//!    with, and the bindings found so far. Provided.
//!  - [`Subst::unify`] (`unify.rs`): matching one generator against one fact.
//!    **Yours.**
//!  - [`Interpreter::solutions`](crate::interp::Interpreter::solutions)
//!    (`engine.rs`): the least-fixpoint engine that answers a query. **Yours.**
//!  - the static checks (`check.rs`): generators name relations over terms,
//!    range restriction, and filter purity. **Yours**; [`check_rules`] here
//!    runs them over every rule from `Interpreter::check`, before `main` runs.

pub mod check;
pub mod db;
pub mod engine;
pub mod error;
pub mod term;
pub mod unify;

pub use db::{RelationDb, Tuple};
pub use error::RuleError;
pub use term::{Subst, Term};

use crate::ast::Program;

/// Check every `match` guard in `program` for purity (M7). `Interpreter::check`
/// runs this from M7; see [`check::check_guards`].
pub fn check_guards(program: &Program) -> Result<(), RuleError> {
    check::check_guards(program)
}

/// Run the three static checks over every rule of `program`, stopping at the
/// first failure. `Interpreter::check` runs this (from M6) before `main`
/// runs; a program whose rules fail never starts.
pub fn check_rules(program: &Program) -> Result<(), RuleError> {
    for rule in &program.rules {
        let generators = check::check_generators(program, rule)?;
        check::check_range_restriction(program, rule, &generators)?;
        check::check_pure(program, rule, &generators)?;
    }
    Ok(())
}
