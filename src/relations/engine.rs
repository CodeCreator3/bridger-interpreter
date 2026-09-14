//! The relations engine — YOUR file.  \[student\]  (Milestone M6)
//!
//! Answers a query by computing the **least fixpoint** of the program's rules
//! over the base facts and the dynamic facts in `self.db`, then matching the
//! query against the result. This is the evaluation the reference's
//! `E-Query` / `E-Solutions` / `E-ForQuery` rules describe; the `Relation`
//! and `ForQuery` arms of `eval_expr` — and the `Call` arm, for a relation
//! applied in call syntax — call [`Interpreter::solutions`] and read off the
//! substitutions.
//!
//! The pieces you build it from: [`Subst::unify`] to match a generator
//! against a fact, `self.eval_expr` (in an environment built from
//! the current substitution) to evaluate a filter, [`RelationDb`] for the
//! dynamic facts, and `self.program().rules` for the rules and the bodiless
//! base facts. The simplest correct shape is the **naive** loop: fire every
//! rule against all the facts derived so far, add what is new, stop when a
//! round adds nothing; `RelationDb::add` reports whether a tuple was new. A
//! semi-naive refinement — each round joining so that at least one generator
//! reads only the facts the previous round added — derives the same facts and
//! is not required; the naive loop passes the same tests.
//!
//! To list this milestone's holes:
//!
//! ```text
//! grep -rn 'todo_m6!' src
//! ```
//!
//! [`RelationDb`]: super::RelationDb

use super::Subst;
use crate::ast::{Query, Span};
use crate::interp::{Control, Env, Interpreter};

impl Interpreter {
    /// Every distinct assignment to the named holes of `q` under which `q`
    /// holds — facts that differ only at a bound argument or an anonymous
    /// hole give one solution — in canonical order (`compare` over the hole
    /// values, first hole first).
    /// Bound arguments of `q` are evaluated in `env` first. A hole-free query
    /// yields either one empty substitution or none, which is how the
    /// boolean-position query reads. `span` locates the query for diagnostics.
    pub fn solutions(&mut self, q: &Query, span: Span, env: &Env) -> Result<Vec<Subst>, Control> {
        let _ = (q, span, env);
        todo_m6!("solutions: least fixpoint of the rules over base + dynamic facts, then match q")
    }
}
