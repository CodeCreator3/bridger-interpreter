
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
use crate::ast::Lit;
use crate::ast::UnOp;
use crate::ast::BinOp;
use crate::ast::Span;
use crate::interp::value::List;
use crate::interp::RuntimeError;
use crate::interp::Ty;
use crate::interp::TyKind;
use crate::interp::Rc;

impl Interpreter {
    /// Evaluate `e` in environment `env`.
    pub fn eval_expr(&mut self, e: &Expr, env: &Env) -> Result<Value, Control> {
        // `env` goes unused until M2 (E-Var, E-Block). Delete this line once you
        // read from `env`; it only keeps the unused-variable lint quiet for now.
        let _ = env;
        match e {
            // ---- M1: expressions ----
            Expr::Lit(lit, span) => Self::literal(lit, span),
            Expr::Unary(op, expr, span) => Self::unary(op, expr, span),
            Expr::Binary(op, expr1, expr2, span) => Self::binary(op, expr1, expr2, span),
            Expr::Tuple(exprs, span) => Self::tuple(exprs, span),
            Expr::List(exprs, span) => Self::list(exprs, span),
            Expr::Proj(expr, num, span) => Self::proj(expr, num, span),

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
    fn literal(lit: &Lit, span: &Span) -> Result<Value, Control> {
        Ok(match lit {
            Lit::Int(n)  => Value::Int(*n),
            Lit::Bool(b) => Value::Bool(*b),
            Lit::Str(s)  => Value::Str(s.clone().into()),
            Lit::Unit    => Value::Unit,
        })
    }
    fn unary(op: &UnOp, expr: &Box<Expr>, span: &Span) -> Result<Value, Control> {
        match op {
            UnOp::Neg => Self::neg(**expr, span),
            UnOp::Not => Self::not(**expr, span),
        }
    }
    fn neg(expr: Expr, span: &Span) -> Result<Value, Control>{
        match expr {
            Expr::Lit(lit, s) => match lit{
                Lit::Int(n) => Ok(Value::Int(-n)),
                Lit::Bool(b) => Err(Control::Raise(RuntimeError::TypeError{expected: Ty{kind: TyKind::Int, span: None}, found: Ty{kind: TyKind::Bool,span: None},span: *span})),
                _ => panic!(),
                },
	    _ => panic!(),
	}
    }
    fn not(expr: Expr, span: &Span) -> Result<Value, Control>{
        match expr {
            Expr::Lit(lit, s) => match lit{
                Lit::Bool(b) => Ok(Value::Bool(!b)),
                Lit::Int(n) => Err(Control::Raise(RuntimeError::TypeError{expected: Ty{kind: TyKind::Bool, span: None}, found: Ty{kind: TyKind::Int,span: None},span: *span})),
                _ => panic!(),
                },
	    _ => panic!(),
	}
    }
    fn binary(op: &BinOp, expr1: &Box<Expr>, expr2: &Box<Expr>, span: &Span) -> Result<Value, Control> {
        match op {
	    BinOp::Add => Self::arith(op, expr1, expr2, span),
	    BinOp::Sub => Self::arith(op, expr1, expr2, span),
	    BinOp::Mul => Self::arith(op, expr1, expr2, span),
	    BinOp::Div => Self::div(expr1, expr2, span),
	    BinOp::Mod => Self::arith(op, expr1, expr2, span),
	    BinOp::Eq => Self::comp(op, expr1, expr2, span),
	    BinOp::Ne => Self::comp(op, expr1, expr2, span),
	    BinOp::Lt => Self::comp(op, expr1, expr2, span),
	    BinOp::Le => Self::comp(op, expr1, expr2, span),
	    BinOp::Gt => Self::comp(op, expr1, expr2, span),
	    BinOp::Ge => Self::comp(op, expr1, expr2, span),
	    BinOp::And => Self::comp(op, expr1, expr2, span),
	    BinOp::Or => Self::comp(op, expr1, expr2, span),
	    BinOp::Concat => Self::concat(expr1, expr2, span),
	    BinOp::Cons => Self::cons(expr1, expr2, span),
	}
    }
    fn arith(op: &BinOp, expr1: &Box<Expr>, expr2: &Box<Expr>, span: &Span) -> Result<Value, Control>{
        match (**expr1, **expr2){
	    (Expr::Lit(lit1, span1), Expr::Lit(lit2, span2)) => 
                match (lit1, lit2){
                    (Lit::Int(n1), Lit::Int(n2)) => 
                        Ok(match *op{
                            BinOp::Add => Value::Int(n1 + n2),
                            BinOp::Sub => Value::Int(n1 - n2),
                            BinOp::Mul => Value::Int(n1 * n2),
                        }),
                    (Lit::Int(n1), Lit::Bool(b2)) => Err(Control::Raise(RuntimeError::TypeError{expected: Ty{kind: TyKind::Int, span: None}, found: Ty{kind: TyKind::Bool, span: None}, span: *span})),
                    (Lit::Bool(b1), Lit::Int(n2)) => Err(Control::Raise(RuntimeError::TypeError{expected: Ty{kind: TyKind::Int, span: None}, found: Ty{kind: TyKind::Bool, span: None}, span: *span})),
                    (Lit::Bool(b1), Lit::Bool(b2)) => Err(Control::Raise(RuntimeError::TypeError{expected: Ty{kind: TyKind::Int, span: None}, found: Ty{kind: TyKind::Bool, span: None}, span: *span})),
                    (_, _) => panic!(), 
                }
	    (_, _) => panic!(),
        }
    }
    fn div(expr1: &Box<Expr>, expr2: &Box<Expr>, span: &Span) -> Result<Value, Control>{
        match (**expr1, **expr2){
	    (Expr::Lit(lit1, span1), Expr::Lit(lit2, span2)) => 
                match (lit1, lit2){
                    (Lit::Int(n1), Lit::Int(n2)) =>
                        match n2{
	                    0 => Err(Control::Raise(RuntimeError::DivByZero{span: *span})),
	                    _ => Ok(Value::Int(n1/n2)),
                        }
                    (Lit::Int(n1), Lit::Bool(b2)) => Err(Control::Raise(RuntimeError::TypeError{expected: Ty{kind: TyKind::Int, span: None}, found: Ty{kind: TyKind::Bool, span: None}, span: *span})),
                    (Lit::Bool(b1), Lit::Int(n2)) => Err(Control::Raise(RuntimeError::TypeError{expected: Ty{kind: TyKind::Int, span: None}, found: Ty{kind: TyKind::Bool, span: None}, span: *span})),
                    (Lit::Bool(b1), Lit::Bool(b2)) => Err(Control::Raise(RuntimeError::TypeError{expected: Ty{kind: TyKind::Int, span: None}, found: Ty{kind: TyKind::Bool, span: None}, span: *span})),                  
                    (_, _) => panic!(),  
                },
	    (_, _) => panic!(),
        }
    }
    fn comp(op: &BinOp, expr1: &Box<Expr>, expr2: &Box<Expr>, span: &Span) -> Result<Value, Control>{
        match (**expr1, **expr2){
            (Expr::Lit(lit1, span1), Expr::Lit(lit2, span2)) =>
                match(lit1, lit2){
                    (Lit::Int(n1), Lit::Int(n2)) => 
                        match *op{
                            BinOp::Eq => Ok(Value::Bool(n1==n2)),
                            BinOp::Ne => Ok(Value::Bool(n1!=n2)),
                            BinOp::Lt => Ok(Value::Bool(n1<n2)),
                            BinOp::Le => Ok(Value::Bool(n1<=n2)),
                            BinOp::Gt => Ok(Value::Bool(n1>n2)),
                            BinOp::Ge => Ok(Value::Bool(n1>=n2)),
                        },
                    (Lit::Bool(b1), Lit::Bool(b2)) => 
                        match *op{
                            BinOp::And => Ok(Value::Bool(b1&&b2)),
                            BinOp::Or => Ok(Value::Bool(b1||b2)), 
                        },
                    (_, _) => panic!(),
                }
            (_, _) => panic!(),
        }
    }
    fn cons(expr1: &Box<Expr>, expr2: &Box<Expr>, span: &Span) -> Result<Value, Control>{
        match(**expr1, **expr2){
            (Expr::List(vec, span1), Expr::Lit(lit, span2)) => {
		let list_val = Self::list(&vec, &span)?;
    		let Value::List(list) = list_val else {
		    panic!()
		};
		match lit{
         	    Lit::Int(int) => Ok(Value::List(List::cons(Value::Int(int), list))),
         	    Lit::Bool(bool) => Ok(Value::List(List::cons(Value::Bool(bool), list))),
		    _ => panic!(),
		}
		},
	    (_, _) => panic!(),
        }
    }
    fn concat(expr1: &Box<Expr>, expr2: &Box<Expr>, span: &Span) -> Result<Value, Control>{
        match(**expr1, **expr2){
	    (Expr::List(vec1, span1), Expr::List(vec2, span2)) => {
	        let Value::List(list1) = Self::list(&vec1, &span1)? else {panic!()};
	        let Value::List(list2) = Self::list(&vec2, &span2)? else {panic!()};
		Ok(Value::List(list1.concat(&list2)))
	    },
	    (_, _) => panic!(),
	}
    }
    fn tuple(exprs: &Vec<Expr>, span: &Span) -> Result<Value, Control> {
        Ok(Value::Tuple(Rc::new(*exprs)))
    }
    fn list(exprs: &Vec<Expr>, span: &Span) -> Result<Value, Control> {
        let list = List::nil;
        for e in *exprs {list.concat(e);}
        Ok(Value::List(list))
    }
    fn proj(expr: &Box<Expr>, num: &u32, span: &Span) -> Result<Value, Control> {
        match num{
	    n if n < 0 || n > expr.length => Err("index out of bounds"),
	    _ => Ok(expr.num),
        }
    }
}
