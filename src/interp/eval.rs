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
use crate::ast::UnOp;
use crate::interp::value::List;
use crate::interp::Rc;
use crate::interp::RuntimeError;
use crate::interp::Ty;
use crate::interp::TyKind;

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
            Expr::Binary(op, expr1, expr2, span) => self.binary(op, expr1, expr2, span),
            Expr::Tuple(exprs, span) => self.tuple(exprs, span),
            Expr::List(exprs, span) => self.list(exprs, span),
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
    fn literal(lit: &Lit, _span: &Span) -> Result<Value, Control> {
        Ok(match lit {
            Lit::Int(n) => Value::Int(*n),
            Lit::Bool(b) => Value::Bool(*b),
            Lit::Str(s) => Value::Str(s.clone().into()),
            Lit::Unit => Value::Unit,
        })
    }
    fn unary(op: &UnOp, expr: &Expr, span: &Span) -> Result<Value, Control> {
        match op {
            UnOp::Neg => Self::neg(expr, span),
            UnOp::Not => Self::not(expr, span),
            _ => panic!(),
        }
    }
    fn neg(expr: &Expr, span: &Span) -> Result<Value, Control> {
        let val = match expr {
            Expr::Lit(lit, _s) => match lit {
                Lit::Bool(b) => Value::Bool(*b),
                Lit::Int(i) => Value::Int(*i),
                Lit::Str(s) => Value::Str(s.clone().into()),
                _ => panic!(),
            },
            Expr::Unary(op, expr, span) => Self::unary(op, expr, span)?,
            _ => panic!(),
        };
        match val {
            Value::Int(n) => Ok(Value::Int(-n)),
            Value::Bool(_b) => Err(Control::Raise(RuntimeError::TypeError {
                expected: Ty {
                    kind: TyKind::Int,
                    span: None,
                },
                found: Ty {
                    kind: TyKind::Bool,
                    span: None,
                },
                span: *span,
            })),
            _ => panic!(),
        }
    }
    fn not(expr: &Expr, span: &Span) -> Result<Value, Control> {
        match expr {
            Expr::Lit(lit, _s) => match lit {
                Lit::Bool(b) => Ok(Value::Bool(!b)),
                Lit::Int(_n) => Err(Control::Raise(RuntimeError::TypeError {
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
            },
            _ => panic!(),
        }
    }
    fn binary(
        &mut self,
        op: &BinOp,
        expr1: &Expr,
        expr2: &Expr,
        span: &Span,
    ) -> Result<Value, Control> {
        match op {
            BinOp::Add => self.arith(op, expr1, expr2, span),
            BinOp::Sub => self.arith(op, expr1, expr2, span),
            BinOp::Mul => self.arith(op, expr1, expr2, span),
            BinOp::Div => Self::divormod(op, expr1, expr2, span),
            BinOp::Mod => Self::divormod(op, expr1, expr2, span),
            BinOp::Eq => self.comp(op, expr1, expr2, span),
            BinOp::Ne => self.comp(op, expr1, expr2, span),
            BinOp::Lt => self.comp(op, expr1, expr2, span),
            BinOp::Le => self.comp(op, expr1, expr2, span),
            BinOp::Gt => self.comp(op, expr1, expr2, span),
            BinOp::Ge => self.comp(op, expr1, expr2, span),
            BinOp::And => self.comp(op, expr1, expr2, span),
            BinOp::Or => self.comp(op, expr1, expr2, span),
            BinOp::Concat => self.concat(expr1, expr2, span),
            BinOp::Cons => self.cons(expr1, expr2, span),
        }
    }
    fn arith(
        &mut self,
        op: &BinOp,
        expr1: &Expr,
        expr2: &Expr,
        span: &Span,
    ) -> Result<Value, Control> {
        let left = match expr1 {
            Expr::Lit(lit, _) => match lit {
                Lit::Int(n) => Value::Int(*n),
                Lit::Bool(_) => {
                    return Err(Control::Raise(RuntimeError::TypeError {
                        expected: Ty {
                            kind: TyKind::Int,
                            span: None,
                        },
                        found: Ty {
                            kind: TyKind::Bool,
                            span: None,
                        },
                        span: *span,
                    }))
                }
                Lit::Str(_) => {
                    return Err(Control::Raise(RuntimeError::TypeError {
                        expected: Ty {
                            kind: TyKind::Int,
                            span: None,
                        },
                        found: Ty {
                            kind: TyKind::Str,
                            span: None,
                        },
                        span: *span,
                    }))
                }
                _ => panic!(),
            },
            Expr::Binary(op2, l, r, s) => self.binary(op2, l, r, s)?,
            _ => panic!(),
        };

        let right = match expr2 {
            Expr::Lit(lit, _) => match lit {
                Lit::Int(n) => Value::Int(*n),
                Lit::Bool(_) => {
                    return Err(Control::Raise(RuntimeError::TypeError {
                        expected: Ty {
                            kind: TyKind::Int,
                            span: None,
                        },
                        found: Ty {
                            kind: TyKind::Bool,
                            span: None,
                        },
                        span: *span,
                    }))
                }
                _ => panic!(),
            },
            Expr::Binary(op2, l, r, s) => self.binary(op2, l, r, s)?,
            _ => panic!(),
        };

        match (left, right) {
            (Value::Int(a), Value::Int(b)) => match op {
                BinOp::Add => Ok(Value::Int(a.wrapping_add(b))),
                BinOp::Sub => Ok(Value::Int(a.wrapping_sub(b))),
                BinOp::Mul => Ok(Value::Int(a.wrapping_mul(b))),
                _ => panic!(),
            },
            _ => panic!(),
        }
    }

    fn divormod(op: &BinOp, expr1: &Expr, expr2: &Expr, span: &Span) -> Result<Value, Control> {
        match (expr1, expr2) {
            (Expr::Lit(lit1, _span1), Expr::Lit(lit2, _span2)) => match (lit1, lit2) {
                (Lit::Int(n1), Lit::Int(n2)) => match n2 {
                    0 => Err(Control::Raise(RuntimeError::DivByZero { span: *span })),
                    _ => match *op {
                        BinOp::Div => Ok(Value::Int(n1 / n2)),
                        BinOp::Mod => Ok(Value::Int(n1 % n2)),
                        _ => panic!(),
                    },
                },
                (Lit::Int(_n1), Lit::Bool(_b2)) => Err(Control::Raise(RuntimeError::TypeError {
                    expected: Ty {
                        kind: TyKind::Int,
                        span: None,
                    },
                    found: Ty {
                        kind: TyKind::Bool,
                        span: None,
                    },
                    span: *span,
                })),
                (Lit::Bool(_b1), Lit::Int(_n2)) => Err(Control::Raise(RuntimeError::TypeError {
                    expected: Ty {
                        kind: TyKind::Int,
                        span: None,
                    },
                    found: Ty {
                        kind: TyKind::Bool,
                        span: None,
                    },
                    span: *span,
                })),
                (Lit::Bool(_b1), Lit::Bool(_b2)) => Err(Control::Raise(RuntimeError::TypeError {
                    expected: Ty {
                        kind: TyKind::Int,
                        span: None,
                    },
                    found: Ty {
                        kind: TyKind::Bool,
                        span: None,
                    },
                    span: *span,
                })),
                (_, _) => panic!(),
            },
            (_, _) => panic!(),
        }
    }
    fn comp(
        &mut self,
        op: &BinOp,
        expr1: &Expr,
        expr2: &Expr,
        span: &Span,
    ) -> Result<Value, Control> {
        match (expr1, expr2) {
            (Expr::Lit(Lit::Bool(left), _), _) => {
                if matches!(op, BinOp::Or) && *left {
                    return Ok(Value::Bool(true));
                }
                if matches!(op, BinOp::And) && !*left {
                    return Ok(Value::Bool(false));
                }
                match (expr1, expr2) {
                    (Expr::Lit(Lit::Bool(left), _), Expr::Lit(Lit::Bool(right), _)) => match *op {
                        BinOp::And => Ok(Value::Bool(*left && *right)),
                        BinOp::Or => Ok(Value::Bool(*left || *right)),
                        _ => panic!(),
                    },
                    (Expr::Lit(Lit::Int(_), _), _) => {
                        Err(Control::Raise(RuntimeError::TypeError {
                            expected: Ty {
                                kind: TyKind::Bool,
                                span: None,
                            },
                            found: Ty {
                                kind: TyKind::Int,
                                span: None,
                            },
                            span: *span,
                        }))
                    }
                    (_, Expr::Lit(Lit::Int(_), _)) => {
                        Err(Control::Raise(RuntimeError::TypeError {
                            expected: Ty {
                                kind: TyKind::Bool,
                                span: None,
                            },
                            found: Ty {
                                kind: TyKind::Int,
                                span: None,
                            },
                            span: *span,
                        }))
                    }
                    (_, Expr::Lit(Lit::Str(_), _)) => {
                        Err(Control::Raise(RuntimeError::TypeError {
                            expected: Ty {
                                kind: TyKind::Bool,
                                span: None,
                            },
                            found: Ty {
                                kind: TyKind::Str,
                                span: None,
                            },
                            span: *span,
                        }))
                    }
                    _ => panic!(),
                }
            }
            _ => {
                let left = match expr1 {
                    Expr::Lit(lit, _) => match lit {
                        Lit::Bool(b) => Value::Bool(*b),
                        Lit::Int(n) => Value::Int(*n),
                        Lit::Str(s) => Value::Str(Rc::from(s.clone())),
                        Lit::Unit => Value::Unit,
                    },
                    Expr::Binary(op2, l, r, s) => self.binary(op2, l, r, s)?,
                    Expr::Tuple(t, _) => self.tuple(t, span)?,
                    _ => panic!(),
                };

                let right = match expr2 {
                    Expr::Lit(lit, _) => match lit {
                        Lit::Bool(b) => Value::Bool(*b),
                        Lit::Int(n) => Value::Int(*n),
                        Lit::Str(s) => Value::Str(Rc::from(s.clone())),
                        Lit::Unit => Value::Unit,
                    },
                    Expr::Binary(op2, l, r, s) => self.binary(op2, l, r, s)?,
                    Expr::Tuple(t, _) => self.tuple(t, span)?,
                    _ => panic!(),
                };
                match (left, right) {
                    (Value::Int(n1), Value::Int(n2)) => match *op {
                        BinOp::Eq => Ok(Value::Bool(n1 == n2)),
                        BinOp::Ne => Ok(Value::Bool(n1 != n2)),
                        BinOp::Lt => Ok(Value::Bool(n1 < n2)),
                        BinOp::Le => Ok(Value::Bool(n1 <= n2)),
                        BinOp::Gt => Ok(Value::Bool(n1 > n2)),
                        BinOp::Ge => Ok(Value::Bool(n1 >= n2)),
                        _ => panic!(),
                    },
                    (Value::Str(s1), Value::Str(s2)) => match *op {
                        BinOp::Eq => Ok(Value::Bool(s1 == s2)),
                        BinOp::Ne => Ok(Value::Bool(s1 != s2)),
                        _ => Err(Control::Raise(RuntimeError::TypeError {
                            expected: Ty {
                                kind: TyKind::Int,
                                span: None,
                            },
                            found: Ty {
                                kind: TyKind::Str,
                                span: None,
                            },
                            span: *span,
                        })),
                    },
                    (Value::Unit, Value::Unit) => Ok(Value::Bool(true)),
                    (Value::Tuple(left), Value::Tuple(right)) => {
                        if left.len() != right.len() {
                            return Ok(Value::Bool(false));
                        }
                        for (x, y) in left.iter().zip(right.iter()) {
                            if !Self::value_equal(x, y) {
                                return Ok(Value::Bool(false));
                            }
                        }
                        Ok(Value::Bool(true))
                    }
                    (Value::Int(_i), _) => match *op {
                        BinOp::Eq => Ok(Value::Bool(false)),
                        BinOp::Ne => Ok(Value::Bool(true)),
                        _ => Err(Control::Raise(RuntimeError::TypeError {
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
                    },
                    (_, _) => Err(Control::Raise(RuntimeError::TypeError {
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
                }
            }
        }
    }
    fn value_equal(a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::Int(x), Value::Int(y)) => x == y,
            (Value::Bool(x), Value::Bool(y)) => x == y,
            (Value::Str(x), Value::Str(y)) => x == y,
            (Value::Unit, Value::Unit) => true,
            _ => true,
        }
    }
    fn cons(&mut self, expr1: &Expr, expr2: &Expr, span: &Span) -> Result<Value, Control> {
        let tail_val = match expr2 {
            Expr::List(es, _) => self.list(es, span)?,
            Expr::Binary(op2, l, r, s) => self.binary(op2, l, r, s)?,
            _ => Value::Unit,
        };
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
        match (expr1, tail_list) {
            (Expr::Lit(lit, _span2), tail_list) => match lit {
                Lit::Int(int) => Ok(Value::List(List::cons(Value::Int(*int), tail_list))),
                Lit::Bool(bool) => Ok(Value::List(List::cons(Value::Bool(*bool), tail_list))),
                Lit::Str(str) => Ok(Value::List(List::cons(
                    Value::Str(Rc::from(str.clone())),
                    tail_list,
                ))),
                _ => panic!(),
            },
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
    fn concat(&mut self, expr1: &Expr, expr2: &Expr, span: &Span) -> Result<Value, Control> {
        match (expr1, expr2) {
            (Expr::List(vec1, span1), Expr::List(vec2, span2)) => {
                let Value::List(list1) = self.list(vec1, span1)? else {
                    panic!()
                };
                let Value::List(list2) = self.list(vec2, span2)? else {
                    panic!()
                };
                Ok(Value::List(list1.concat(&list2)))
            }
            (Expr::Lit(Lit::Str(s1), _), Expr::Lit(Lit::Str(s2), _)) => {
                Ok(Value::Str(format!("{s1}{s2}").into()))
            }
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
    fn tuple(&mut self, exprs: &Vec<Expr>, _span: &Span) -> Result<Value, Control> {
        let mut list = Vec::new();
        for expr in exprs {
            match expr {
                Expr::Lit(lit, _span) => match lit {
                    Lit::Bool(b) => list.push(Value::Bool(*b)),
                    Lit::Int(i) => list.push(Value::Int(*i)),
                    _ => panic!(),
                },
                Expr::Binary(_op, _expr1, _expr2, _span) => {
                    let v = self.eval_expr(expr, &Env::new());
                    list.push(v?);
                }
                Expr::List(vec, span) => list.push(self.list(vec, span)?),
                _ => panic!(),
            }
        }
        Ok(Value::Tuple(Rc::from(list)))
    }
    fn list(&mut self, exprs: &[Expr], _span: &Span) -> Result<Value, Control> {
        let mut list = List::nil();
        for e in exprs.iter().rev() {
            let val = match e {
                Expr::Lit(lit, _span) => match lit {
                    Lit::Bool(b) => Value::Bool(*b),
                    Lit::Int(i) => Value::Int(*i),
                    _ => panic!(),
                },
                Expr::Binary(op, expr1, expr2, span) => self.binary(op, expr1, expr2, span)?,
                _ => panic!(),
            };
            list = List::cons(val, list);
        }
        Ok(Value::List(list))
    }
    fn proj(expr: &Expr, num: &u32, span: &Span) -> Result<Value, Control> {
        let ind = *num as usize;
        match expr {
            Expr::Tuple(list, _span) => match num {
                n if *n >= list.len().try_into().expect("usize exceeds u32 max value") => {
                    Err(Control::Raise(RuntimeError::NoSuchField {
                        field: n.to_string().clone(),
                        span: *span,
                    }))
                }
                _ => {
                    let val = &list[ind];
                    let Expr::Lit(lit, _span) = val else { panic!() };
                    match lit {
                        Lit::Bool(b) => Ok(Value::Bool(*b)),
                        Lit::Int(i) => Ok(Value::Int(*i)),
                        Lit::Str(s) => Ok(Value::Str(Rc::from(s.clone()))),
                        _ => panic!(),
                    }
                }
            },
            _ => Err(Control::Raise(RuntimeError::NoSuchField {
                field: num.to_string().clone(),
                span: *span,
            })),
        }
    }
}
