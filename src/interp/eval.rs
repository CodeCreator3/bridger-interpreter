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
use crate::ast::BinOp;
use crate::ast::Expr;
use crate::ast::Lit;
use crate::ast::Span;
use crate::ast::Stmt;
use crate::ast::UnOp;
use crate::interp::value::List;
use crate::interp::RuntimeError;
use crate::interp::Ty;
use crate::interp::TyKind;
use crate::interp::{type_of, Rc};

impl Interpreter {
    /// Evaluate `e` in environment `env`.
    pub fn eval_expr(&mut self, e: &Expr, env: &Env) -> Result<Value, Control> {
        match e {
            // ---- M1: expressions ----
            Expr::Lit(lit, span) => Self::literal(lit, span, env),
            Expr::Unary(op, expr, span) => self.unary(op, expr, span, env),
            Expr::Binary(op, expr1, expr2, span) => self.binary(op, expr1, expr2, span, env),
            Expr::Tuple(exprs, span) => self.tuple(exprs, span, env),
            Expr::List(exprs, span) => self.list(exprs, span, env),
            Expr::Proj(expr, num, span) => self.proj(expr, num, span, env),

            // ---- M2: binding ----
            Expr::Var(name, span) => self.var(name, span, env),
            Expr::Block(vec, option, span) => self.block(vec, option, span, env),

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
    fn literal(lit: &Lit, _span: &Span, _env: &Env) -> Result<Value, Control> {
        Ok(match lit {
            Lit::Int(n) => Value::Int(*n),
            Lit::Bool(b) => Value::Bool(*b),
            Lit::Str(s) => Value::Str(s.clone().into()),
            Lit::Unit => Value::Unit,
        })
    }
    fn unary(&mut self, op: &UnOp, expr: &Expr, span: &Span, env: &Env) -> Result<Value, Control> {
        match op {
            UnOp::Neg => self.neg(expr, span, env),
            UnOp::Not => self.not(expr, span, env),
            _ => panic!(),
        }
    }
    fn neg(&mut self, expr: &Expr, span: &Span, env: &Env) -> Result<Value, Control> {
        let val = self.eval_expr(expr, env)?;
        match val {
            Value::Int(n) => Ok(Value::Int(n.wrapping_neg())),
            val => Err(Control::Raise(RuntimeError::TypeError {
                expected: Ty {
                    kind: TyKind::Int,
                    span: None,
                },
                found: type_of(&val),
                span: *span,
            })),
        }
    }
    fn not(&mut self, expr: &Expr, span: &Span, env: &Env) -> Result<Value, Control> {
        let val = self.eval_expr(expr, env)?;
        match val {
            Value::Bool(b) => Ok(Value::Bool(!b)),
            Value::Int(_n) => Err(Control::Raise(RuntimeError::TypeError {
                expected: Ty {
                    kind: TyKind::Bool,
                    span: None,
                },
                found: Ty {
                    kind: TyKind::Int,
                    span: None,
                },
                span: *span,
            })),
            _ => panic!(),
        }
    }
    fn binary(
        &mut self,
        op: &BinOp,
        expr1: &Expr,
        expr2: &Expr,
        span: &Span,
        env: &Env,
    ) -> Result<Value, Control> {
        match op {
            BinOp::Add => self.arith(op, expr1, expr2, span, env),
            BinOp::Sub => self.arith(op, expr1, expr2, span, env),
            BinOp::Mul => self.arith(op, expr1, expr2, span, env),
            BinOp::Div => self.divormod(op, expr1, expr2, span, env),
            BinOp::Mod => self.divormod(op, expr1, expr2, span, env),
            BinOp::Eq => self.comp(op, expr1, expr2, span, env),
            BinOp::Ne => self.comp(op, expr1, expr2, span, env),
            BinOp::Lt => self.comp(op, expr1, expr2, span, env),
            BinOp::Le => self.comp(op, expr1, expr2, span, env),
            BinOp::Gt => self.comp(op, expr1, expr2, span, env),
            BinOp::Ge => self.comp(op, expr1, expr2, span, env),
            BinOp::And => self.logical_op(op, expr1, expr2, span, env),
            BinOp::Or => self.logical_op(op, expr1, expr2, span, env),
            BinOp::Concat => self.concat(expr1, expr2, span, env),
            BinOp::Cons => self.cons(expr1, expr2, span, env),
        }
    }
    fn arith(
        &mut self,
        op: &BinOp,
        expr1: &Expr,
        expr2: &Expr,
        span: &Span,
        env: &Env,
    ) -> Result<Value, Control> {
        let left = self.eval_expr(expr1, env)?;
        let right = self.eval_expr(expr2, env)?;

        match (left, right) {
            (Value::Int(a), Value::Int(b)) => match op {
                BinOp::Add => Ok(Value::Int(a.wrapping_add(b))),
                BinOp::Sub => Ok(Value::Int(a.wrapping_sub(b))),
                BinOp::Mul => Ok(Value::Int(a.wrapping_mul(b))),
                _ => panic!(),
            },
            (left, right) => {
                let offender = if matches!(left, Value::Int(_)) {
                    &right
                } else {
                    &left
                };

                Err(Control::Raise(RuntimeError::TypeError {
                    expected: Ty {
                        kind: TyKind::Int,
                        span: None,
                    },

                    found: type_of(offender),

                    span: *span,
                }))
            }
        }
    }

    fn divormod(
        &mut self,
        op: &BinOp,
        expr1: &Expr,
        expr2: &Expr,
        span: &Span,
        env: &Env,
    ) -> Result<Value, Control> {
        let val1 = self.eval_expr(expr1, env)?;
        let val2 = self.eval_expr(expr2, env)?;
        match (val1, val2) {
            (Value::Int(n1), Value::Int(n2)) => {
                println!("divormod: op={:?} n1={} n2={} span={:?}", op, n1, n2, span);
                match n2 {
                    0 => Err(Control::Raise(RuntimeError::DivByZero { span: *span })),
                    _ => match *op {
                        BinOp::Div => Ok(Value::Int(n1.wrapping_div(n2))),
                        BinOp::Mod => Ok(Value::Int(n1.wrapping_rem(n2))),
                        _ => panic!(),
                    },
                }
            }
            (left, right) => {
                let offender = if matches!(left, Value::Int(_)) {
                    &right
                } else {
                    &left
                };

                Err(Control::Raise(RuntimeError::TypeError {
                    expected: Ty {
                        kind: TyKind::Int,
                        span: None,
                    },

                    found: type_of(offender),

                    span: *span,
                }))
            }
        }
    }
    fn logical_op(
        &mut self,
        op: &BinOp,
        expr1: &Expr,
        expr2: &Expr,
        span: &Span,
        env: &Env,
    ) -> Result<Value, Control> {
        let val1 = self.eval_expr(expr1, env)?;
        match val1 {
            Value::Bool(left) => {
                if matches!(op, BinOp::Or) && left {
                    return Ok(Value::Bool(true));
                }
                if matches!(op, BinOp::And) && !left {
                    return Ok(Value::Bool(false));
                }
                let val2 = self.eval_expr(expr2, env)?;
                match val2 {
                    Value::Bool(right) => match *op {
                        BinOp::And => Ok(Value::Bool(left && right)),
                        BinOp::Or => Ok(Value::Bool(left || right)),
                        _ => panic!(),
                    },
                    val2 => Err(Control::Raise(RuntimeError::TypeError {
                        expected: Ty {
                            kind: TyKind::Bool,
                            span: None,
                        },
                        found: type_of(&val2),
                        span: *span,
                    })),
                }
            }
            val1 => Err(Control::Raise(RuntimeError::TypeError {
                expected: Ty {
                    kind: TyKind::Bool,
                    span: None,
                },
                found: type_of(&val1),
                span: *span,
            })),
        }
    }
    fn comp(
        &mut self,
        op: &BinOp,
        expr1: &Expr,
        expr2: &Expr,
        span: &Span,
        env: &Env,
    ) -> Result<Value, Control> {
        let val1 = self.eval_expr(expr1, env)?;
        let val2 = self.eval_expr(expr2, env)?;
        match (val1, val2) {
            (Value::Int(n1), Value::Int(n2)) => match *op {
                BinOp::Eq => Ok(Value::Bool(n1 == n2)),
                BinOp::Ne => Ok(Value::Bool(n1 != n2)),
                BinOp::Lt => Ok(Value::Bool(n1 < n2)),
                BinOp::Le => Ok(Value::Bool(n1 <= n2)),
                BinOp::Gt => Ok(Value::Bool(n1 > n2)),
                BinOp::Ge => Ok(Value::Bool(n1 >= n2)),
                _ => panic!(),
            },
            (val1, val2) => match *op {
                BinOp::Eq => Ok(Value::Bool(val1 == val2)),
                BinOp::Ne => Ok(Value::Bool(val1 != val2)),
                _ => {
                    let offender = if matches!(val1, Value::Int(_)) {
                        &val2
                    } else {
                        &val1
                    };

                    Err(Control::Raise(RuntimeError::TypeError {
                        expected: Ty {
                            kind: TyKind::Int,
                            span: None,
                        },

                        found: type_of(offender),

                        span: *span,
                    }))
                }
            },
        }
    }
    fn cons(
        &mut self,
        expr1: &Expr,
        expr2: &Expr,
        span: &Span,
        env: &Env,
    ) -> Result<Value, Control> {
        let val1 = self.eval_expr(expr1, env)?;
        let tail_val = self.eval_expr(expr2, env)?;
        let Value::List(tail_list) = tail_val else {
            return Err(Control::Raise(RuntimeError::TypeError {
                expected: Ty {
                    kind: TyKind::List(Rc::new(Ty {
                        kind: TyKind::Int,
                        span: None,
                    })),
                    span: None,
                },
                found: Ty {
                    kind: TyKind::Int,
                    span: None,
                },
                span: *span,
            }));
        };
        match val1 {
            Value::Int(int) => Ok(Value::List(List::cons(Value::Int(int), tail_list))),
            Value::Bool(bool) => Ok(Value::List(List::cons(Value::Bool(bool), tail_list))),
            Value::Str(str) => Ok(Value::List(List::cons(Value::Str(str.clone()), tail_list))),
            _ => panic!(),
        }
    }
    fn concat(
        &mut self,
        expr1: &Expr,
        expr2: &Expr,
        span: &Span,
        env: &Env,
    ) -> Result<Value, Control> {
        let val1 = self.eval_expr(expr1, env)?;
        let val2 = self.eval_expr(expr2, env)?;
        match (val1, val2) {
            (Value::List(list1), Value::List(list2)) => Ok(Value::List(list1.concat(&list2))),
            (Value::Str(s1), Value::Str(s2)) => Ok(Value::Str(format!("{s1}{s2}").into())),
            (_, _) => Err(Control::Raise(RuntimeError::TypeError {
                expected: Ty {
                    kind: TyKind::List(Rc::new(Ty {
                        kind: TyKind::Int,
                        span: None,
                    })),
                    span: None,
                },
                found: Ty {
                    kind: TyKind::Int,
                    span: None,
                },
                span: *span,
            })),
        }
    }
    fn tuple(&mut self, exprs: &Vec<Expr>, _span: &Span, env: &Env) -> Result<Value, Control> {
        let mut list = Vec::new();
        for expr in exprs {
            list.push(self.eval_expr(expr, env)?)
        }
        Ok(Value::Tuple(Rc::from(list)))
    }
    fn list(&mut self, exprs: &[Expr], _span: &Span, env: &Env) -> Result<Value, Control> {
        let mut list = List::nil();
        for e in exprs.iter().rev() {
            let val = self.eval_expr(e, env)?;
            list = List::cons(val, list);
        }
        Ok(Value::List(list))
    }
    fn proj(&mut self, expr: &Expr, num: &u32, span: &Span, env: &Env) -> Result<Value, Control> {
        let ind = *num as usize;
        let val = self.eval_expr(expr, env)?;
        match val {
            Value::Tuple(list) => match num {
                n if *n >= list.len().try_into().expect("usize exceeds u32 max value") => {
                    Err(Control::Raise(RuntimeError::NoSuchField {
                        field: n.to_string().clone(),
                        span: *span,
                    }))
                }
                _ => Ok(list[ind].clone()),
            },
            _ => Err(Control::Raise(RuntimeError::NoSuchField {
                field: num.to_string().clone(),
                span: *span,
            })),
        }
    }
    fn var(&mut self, name: &String, span: &Span, env: &Env) -> Result<Value, Control> {
        let result = env.lookup(name);
        if result.is_none() {
            Err(Control::Raise(RuntimeError::NoSuchField {
                field: name.clone(),
                span: *span,
            }))
        } else {
            Ok(result.unwrap())
        }
    }
    fn block(
        &mut self,
        vec: &Vec<Stmt>,
        _option: &Option<Box<Expr>>,
        _span: &Span,
        env: &Env,
    ) -> Result<Value, Control> {
        let mut output = Ok(Value::Unit);
        for stmt in vec {
            output = match stmt {
                Stmt::Let(name, option, expr, span) => self.e_let(name, option, expr, span, env),
                Stmt::Expr(expr) => self.eval_expr(&expr, env),
            }
        }
        output
    }

    fn e_let(
        &mut self,
        name: &String,
        _option: &Option<Ty>,
        expr: &Expr,
        _span: &Span,
        env: &Env,
    ) -> Result<Value, Control> {
        let value = self.eval_expr(&expr, env)?;
        env.extend(name.clone(), value);
        Ok(Value::Unit)
    }
}
