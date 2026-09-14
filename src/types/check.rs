//! The type checker — YOUR file.  \[student\]  (Milestone M5, growing after)
//!
//! [`check_expr`](Checker::check_expr) is to the typing rules what `eval_expr`
//! is to the evaluation rules: one arm per rule, over the same `Expr`. It is
//! **bidirectional**. With an `expected` type the arm *checks* `e` against it
//! (the mode a lambda needs, where the parameter types come from the expected
//! function type); with `None` it *infers* the type from `e` alone. Either way
//! it returns `e`'s type and records it with [`Checker::record`].
//!
//! Every arm is a milestone-tagged hole. The checker arrives at M5 with the
//! expression language of M1–M4; each later milestone's forms get their typing
//! rules when the milestone lands. To list a milestone's holes:
//!
//! ```text
//! grep -rn 'todo_m5!' src
//! ```
//!
//! Where a rule needs two types to agree, call [`Checker::unify`] rather than
//! comparing with `==`: an unannotated `let`, a lambda parameter, an empty
//! list, or `deref` of a not-yet-known reference gets a
//! [`TyKind::Meta`](crate::ast::TyKind::Meta), and the same `unify` call that
//! compares two known types solves it.
//!
//! A few choices the checker makes cannot be recovered at run time, so it hands
//! them to the evaluator through `Checker::record_conversion`, keyed by an
//! expression's span: the `from` a `?` converts through when the error types
//! differ, and the impl chosen when a method call's receiver leaves it
//! ambiguous (both M8). Record at the same span the evaluator looks up, or the
//! choice is silently lost.

use crate::ast::{Expr, Rule, Span, Ty};
use super::{Checker, Ctx, TyError};

impl Checker {
    /// The type of `e` in context `ctx`, checked against `expected` when one
    /// is given. Records the result at `e`'s span.
    pub fn check_expr(
        &mut self,
        e: &Expr,
        ctx: &Ctx,
        expected: Option<&Ty>,
    ) -> Result<Ty, TyError> {
        // `ctx` and `expected` are read by the arms you write; this line only
        // keeps the unused-variable lint quiet until then.
        let _ = (ctx, expected);
        match e {
            // ---- M5: the checker over M1–M4's expressions ----
            Expr::Lit(..) => todo_m5!("T-Int / T-Bool / T-Str / T-Unit"),
            Expr::Var(..) => todo_m5!("T-Var / T-Self"),
            Expr::Unary(..) => todo_m5!("T-Neg / T-Not / T-Ref / T-Deref"),
            Expr::Binary(..) => todo_m5!("T-Arith / T-Ord / T-Eq / T-And / T-Or / T-Concat / T-Cons"),
            Expr::Tuple(..) => todo_m5!("T-Tuple"),
            Expr::List(..) => todo_m5!("T-List"),
            Expr::Proj(..) => todo_m5!("T-Proj"),
            Expr::Block(..) => todo_m5!("T-Block / T-Let / T-Seq"),
            Expr::If(..) => todo_m5!("T-If"),
            Expr::While(..) => todo_m5!("T-While"),
            Expr::For(..) => todo_m5!("T-For"),
            Expr::Assign(..) => todo_m5!("T-Assign"),
            Expr::Return(..) => todo_m5!("T-Return (reads ctx.ret())"),
            Expr::Lambda(..) => todo_m5!("T-Lam (parameter types from `expected`, the annotation, or a fresh meta)"),
            Expr::Call(..) => todo_m5!("T-App (first-order matching of generics at the call)"),

            // ---- M6: relations ----
            Expr::Relation(..) => todo_m6!("T-Add / T-Clear / T-Solutions / T-Query"),
            Expr::ForQuery(..) => todo_m6!("T-ForQuery"),

            // ---- M7: algebraic data types ----
            Expr::Ctor(..) => todo_m7!("T-Con"),
            Expr::Match(..) => todo_m7!("T-Match (patterns, then exhaustiveness)"),
            Expr::Try(..) => todo_m7!("T-Try"),

            // ---- M8: objects ----
            Expr::Struct(..) => todo_m8!("T-Struct"),
            Expr::Field(..) => todo_m8!("T-Field"),
            Expr::Method(..) => todo_m8!("T-Method (dispatch on the receiver's type; bounds)"),
        }
    }

    /// Make `a` and `b` the same type, or fail with a `Mismatch` at `span`.
    /// While every type is known this is a structural comparison; once
    /// inference variables exist it also *solves* them (`Checker::solve`),
    /// with an occurs check so no type contains itself.
    pub fn unify(&mut self, a: &Ty, b: &Ty, span: Span) -> Result<(), TyError> {
        let _ = (a, b, span);
        todo_m5!("unify (structural agreement; solves TyKind::Meta with an occurs check)")
    }

    /// Type-check one rule (Appendix D, "Relations"): bind each generator's
    /// logic variables at their relation's column types, check the head's
    /// terms against the head relation's columns, and check every filter
    /// against `Bool`. A distinct judgment from `check_expr`, which it calls
    /// for the filters; the driver runs it over each rule once relations
    /// arrive.
    pub fn check_rule(&mut self, rule: &Rule) -> Result<(), TyError> {
        let _ = rule;
        todo_m6!("check_rule (generators bind logic variables; head and filters typed)")
    }
}
