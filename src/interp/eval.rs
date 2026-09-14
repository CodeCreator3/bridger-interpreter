//! The evaluator — YOUR file.  \[student\]
//!
//! [`eval_expr`](Interpreter::eval_expr) turns one [`Expr`] into a [`Value`], or
//! leaves evaluation early through [`Control`] (a stuck `Raise`, or a `Return`).
//! It is the heart of the interpreter and the one method you grow across the
//! milestones. Every `Expr` variant already has an arm; a variant you have not
//! reached yet is a milestone-tagged hole. To list a milestone's holes:
//!
//! ```text
//! grep -rn 'todo_m3!' src
//! ```
//!
//! To fill one in, replace its `todo_mN!(..)` with the rule's implementation,
//! destructuring the arm's `..` into the fields you need — for example
//! `Expr::Lit(lit, span) => …`. A block is just `Expr::Block`, so statement and
//! block evaluation live inside this one method; factor out a helper if you
//! like — the grader only ever calls `eval_expr`.
//!
//! Two contracts these arms must keep. A Bridger call — a function or method
//! body, and the `from` a `?` runs — is entered through
//! `Interpreter::enter_call` and left through `Interpreter::leave_call`,
//! balanced, so runaway recursion becomes a clean `RuntimeError::StackOverflow`
//! rather than aborting the process; this is the run-time call depth, separate
//! from the static nesting bound the checker enforces. And the `?` and
//! method-dispatch arms (M7, M8) read the type checker's choices from
//! `self.conversions`, keyed by an expression's span: look one up at the same
//! span the checker recorded it at (`Checker::record_conversion`), or a `?`
//! quietly passes its payload through unconverted.

use super::Interpreter;
use super::{Control, Env, Value};
use crate::ast::Expr;

impl Interpreter {
    /// Evaluate `e` in environment `env`.
    pub fn eval_expr(&mut self, e: &Expr, env: &Env) -> Result<Value, Control> {
        // `env` goes unused until M2 (E-Var, E-Block). Delete this line once you
        // read from `env`; it only keeps the unused-variable lint quiet for now.
        let _ = env;
        match e {
            // ---- M1: expressions ----
            Expr::Lit(..) => todo_m1!("E-Lit"),
            Expr::Unary(..) => todo_m1!("E-Neg / E-Not (ref & deref join at M3)"),
            Expr::Binary(..) => {
                todo_m1!("E-Arith / E-Ord / E-Eq / E-And / E-Or / E-Concat / E-Cons")
            }
            Expr::Tuple(..) => todo_m1!("E-Tuple"),
            Expr::List(..) => todo_m1!("E-List"),
            Expr::Proj(..) => todo_m1!("E-Proj (tuple projection)"),

            // ---- M2: binding ----
            Expr::Var(..) => todo_m2!("E-Var"),
            Expr::Block(..) => todo_m2!("E-Block / E-Let / E-Seq"),

            // ---- M3: state & control ----
            Expr::If(..) => todo_m3!("E-If"),
            Expr::While(..) => todo_m3!("E-While"),
            Expr::For(..) => todo_m3!("E-For"),
            Expr::Assign(..) => todo_m3!("E-Assign"),
            Expr::Return(..) => todo_m3!("E-Return"),

            // ---- M4: functions ----
            Expr::Lambda(..) => todo_m4!("E-Lam"),
            Expr::Call(..) => todo_m4!("E-App (dispatch Bridger vs. Native closure)"),

            // ---- M6: relations (queries go through `self.solutions`, engine.rs) ----
            Expr::Relation(..) => todo_m6!("E-Add / E-Clear / E-Solutions / E-Query"),
            Expr::ForQuery(..) => todo_m6!("E-ForQuery (iterate a query's solutions)"),

            // ---- M7: algebraic data types ----
            Expr::Ctor(..) => todo_m7!("E-Ctor"),
            Expr::Match(..) => todo_m7!("E-Match"),
            Expr::Try(..) => todo_m7!("E-Try (`?`: match + return)"),

            // ---- M8: objects ----
            Expr::Struct(..) => todo_m8!("E-Struct"),
            Expr::Field(..) => todo_m8!("E-Field (struct field access)"),
            Expr::Method(..) => todo_m8!("E-Method (head-type dispatch)"),
        }
    }
}
