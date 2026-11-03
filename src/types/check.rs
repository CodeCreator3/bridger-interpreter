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

use super::{Checker, Ctx, TyError};
use crate::{
    ast::{BinOp, Decl::Fn, Expr, Lit, Rule, Span, Stmt, Ty, TyKind, UnOp},
    types::{HashMap, TraitRef, MAX_NESTING},
};

impl Checker {
    /// The type of `e` in context `ctx`, checked against `expected` when one
    /// is given. Records the result at `e`'s span.
    pub fn check_expr(
        &mut self,
        e: &Expr,
        ctx: &Ctx,
        expected: Option<&Ty>,
    ) -> Result<Ty, TyError> {
        if self.depth >= MAX_NESTING {
            return Err(TyError::TooDeep {
                limit: MAX_NESTING,
                span: e.span(),
            });
        }
        self.depth += 1;
        let result = (|| match e {
            // ---- M5: the checker over M1–M4's expressions ----
            Expr::Lit(lit, _span) => Ok(match *lit {
                Lit::Int(_) => Ty::int(),
                Lit::Bool(_) => Ty::bool(),
                Lit::Str(_) => Ty::str(),
                Lit::Unit => Ty::unit(),
            }),
            Expr::Var(name, span) => {
                let optional = ctx.lookup(name);

                if let Some(optional_value) = optional {
                    self.record(*span, optional_value.clone());
                    Ok(optional_value)
                } else {
                    let native = self.native_fn_type(name, *span);
                    if let Some(native_value) = native {
                        self.record(*span, native_value.clone());
                        Ok(native_value)
                    } else {
                        let global = self.global_type(name);
                        if let Some(global_value) = global {
                            global_value
                        } else {
                            let decl = self.decl(name).cloned();
                            if let Some(decl_value) = decl {
                                match decl_value {
                                    Fn(gen, params, val, _expr) => {
                                        let mut replacements = HashMap::new();
                                        for p in gen.clone().params {
                                            let meta = self.fresh_meta();
                                            if let Some(bound) = &p.bound {
                                                let bound = TraitRef {
                                                    args: bound
                                                        .args
                                                        .iter()
                                                        .map(|t| t.substitute(&replacements))
                                                        .collect(),
                                                    ..bound.clone()
                                                };
                                                self.require_trait(bound, meta.clone(), *span);
                                            }
                                            replacements.insert(p.name, meta);
                                        }
                                        let ret = Ty::func(
                                            (*params)
                                                .iter()
                                                .map(|p| p.ty.substitute(&replacements).clone())
                                                .collect(),
                                            val.clone().substitute(&replacements),
                                        );
                                        self.record(*span, ret.clone());
                                        Ok(ret)
                                    }
                                    _ => panic!(),
                                }
                            } else {
                                if self.in_dead_code(*span) {
                                    return Ok(self.fresh_meta());
                                }
                                Err(TyError::UnboundVariable {
                                    name: name.clone(),
                                    span: *span,
                                })
                            }
                        }
                    }
                }
            }
            Expr::Unary(un_op, expr, span) => match *un_op {
                UnOp::Neg => {
                    let val1 = self.check_expr(expr, ctx, Some(&Ty::int()))?;
                    self.unify(&val1, &Ty::int(), *span)?;
                    self.record(*span, Ty::int());
                    Ok(Ty::int())
                }
                UnOp::Not => {
                    let val1 = self.check_expr(expr, ctx, Some(&Ty::bool()))?;
                    self.unify(&val1, &Ty::bool(), *span)?;
                    self.record(*span, Ty::bool());
                    Ok(Ty::bool())
                }
                UnOp::Ref => {
                    let val;
                    if expected.clone().is_some() {
                        match expected.unwrap().kind {
                            TyKind::Ref(ref r) => {
                                val = Ty::reference(self.check_expr(expr, ctx, Some(r))?);
                                self.unify(&val, expected.unwrap(), *span)?;
                            }
                            _ => {
                                val = Ty::reference(self.check_expr(expr, ctx, None)?);
                            }
                        }
                    } else {
                        val = Ty::reference(self.check_expr(expr, ctx, None)?);
                    }

                    self.record(*span, val.clone());
                    Ok(val)
                }

                UnOp::Deref => {
                    let mut val = self.check_expr(expr, ctx, None)?;
                    val = self.resolve(&val);
                    match val.kind {
                        TyKind::Ref(r) => {
                            self.record(*span, r.as_ref().clone());
                            Ok(r.as_ref().clone())
                        }
                        ty => Err(TyError::Mismatch {
                            expected: Ty::reference(self.fresh_meta()),
                            found: Ty::from(ty),
                            span: *span,
                        }),
                    }
                }
            },
            Expr::Binary(bin_op, expr1, expr2, span) => match *bin_op {
                BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                    let val1 = self.check_expr(expr1, ctx, Some(&Ty::int()))?;
                    let val2 = self.check_expr(expr2, ctx, Some(&Ty::int()))?;
                    self.unify(&val1, &Ty::int(), *span)?;
                    self.unify(&val2, &Ty::int(), *span)?;
                    self.record(*span, Ty::int());
                    Ok(Ty::int())
                }
                BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                    let val1 = self.check_expr(expr1, ctx, Some(&Ty::int()))?;
                    let val2 = self.check_expr(expr2, ctx, Some(&Ty::int()))?;
                    self.unify(&val1, &Ty::int(), *span)?;
                    self.unify(&val2, &Ty::int(), *span)?;
                    self.record(*span, Ty::bool());
                    Ok(Ty::bool())
                }
                BinOp::Eq | BinOp::Ne => {
                    let val1 = self.check_expr(expr1, ctx, Option::None)?;
                    let val2 = self.check_expr(expr2, ctx, Option::None)?;
                    self.unify(&val1, &val2, *span)?;
                    self.require_bound("Eq".to_string(), val1, *span);
                    self.record(*span, Ty::bool());
                    Ok(Ty::bool())
                }
                BinOp::And | BinOp::Or => {
                    let val1 = self.check_expr(expr1, ctx, Some(&Ty::bool()))?;
                    let val2 = self.check_expr(expr2, ctx, Some(&Ty::bool()))?;
                    self.unify(&val1, &Ty::bool(), *span)?;
                    self.unify(&val2, &Ty::bool(), *span)?;
                    self.record(*span, Ty::bool());
                    Ok(Ty::bool())
                }
                BinOp::Concat => {
                    let val1 = self.check_expr(expr1, ctx, Option::None)?;
                    let val2 = self.check_expr(expr2, ctx, Option::None)?;
                    match val1.clone().kind {
                        TyKind::List(_) => (),
                        TyKind::Str => (),
                        TyKind::Meta(_) => {
                            return self.undetermined(*span);
                        }
                        _ => {
                            return Err(TyError::Mismatch {
                                expected: Ty::str(),
                                found: val1,
                                span: expr1.span(),
                            });
                        }
                    }
                    match val2.clone().kind {
                        TyKind::List(_) => (),
                        TyKind::Str => (),
                        TyKind::Meta(_) => {
                            return self.undetermined(*span);
                        }
                        _ => {
                            return Err(TyError::Mismatch {
                                expected: Ty::str(),
                                found: val2,
                                span: expr2.span(),
                            });
                        }
                    }
                    self.unify(&val1, &val2, *span)?;
                    self.record(*span, val1.clone());
                    Ok(val1)
                }
                BinOp::Cons => {
                    let mut val2 = self.check_expr(expr2, ctx, Option::None)?;
                    val2 = self.resolve(&val2);
                    if let TyKind::List(list) = val2.clone().kind {
                        let val1 = self.check_expr(expr1, ctx, Some(&list.as_ref().clone()))?;
                        self.unify(&val1, &list.as_ref().clone(), *span)?;
                    }
                    self.record(*span, val2.clone());
                    Ok(val2)
                }
            },
            Expr::Tuple(vec_expr, span) => {
                if !vec_expr.is_empty() {
                    let vec_val: Vec<_> = (*vec_expr)
                        .iter()
                        .map(|expr| self.check_expr(expr, ctx, None))
                        .collect::<Result<_, _>>()?;
                    self.record(*span, Ty::tuple(vec_val.clone()));
                    Ok(Ty::tuple(vec_val))
                } else {
                    self.record(*span, Ty::tuple(vec![]));
                    Ok(Ty::tuple(vec![]))
                }
            }
            Expr::List(vec_expr, span) => {
                let vec_val: Vec<_> = (*vec_expr)
                    .iter()
                    .map(|expr| self.check_expr(expr, ctx, None))
                    .collect::<Result<_, _>>()?;
                if !vec_val.is_empty() {
                    let prev_val = vec_val[0].clone();
                    for val in vec_val.clone() {
                        self.unify(&val, &prev_val, *span)?;
                    }
                    self.record(*span, vec_val[0].clone());
                    Ok(Ty::list(vec_val[0].clone()))
                } else {
                    let ty: Ty = Ty::list(self.fresh_meta());
                    self.record(*span, ty.clone());
                    Ok(ty)
                }
            }
            Expr::Proj(expr, index, span) => {
                let mut val = self.check_expr(expr, ctx, Option::None)?;
                val = self.resolve(&val);
                match val.kind {
                    TyKind::Tuple(val_vec) => {
                        if val_vec.len() <= *index as usize {
                            return Err(TyError::NoSuchComponent {
                                index: *index,
                                arity: val_vec.len(),
                                span: *span,
                            });
                        }
                        self.record(*span, val_vec[*index as usize].clone());
                        Ok(val_vec[*index as usize].clone())
                    }
                    TyKind::Meta(id) => {
                        if self.in_dead_code(*span) {
                            return Ok(self.fresh_meta());
                        }
                        Err(TyError::NotATuple {
                            index: *index,
                            found: Ty::meta(id),
                            span: *span,
                        })
                    }
                    ty => Err(TyError::NotATuple {
                        index: *index,
                        found: Ty::from(ty),
                        span: *span,
                    }),
                }
            }
            Expr::Block(vec_stmt, expr, span) => {
                let mut cx = ctx.clone();
                let mut leaves = false;
                for stmt in vec_stmt.clone() {
                    // T-Let / T-Seq: check the statement, extending `cx` for a `let`.
                    // `e` is the statement's expression or the `let`'s initializer,
                    // `at` is the statement's span.
                    let (e, at) = match stmt {
                        Stmt::Let(name, val, expr, span) => {
                            if val.as_ref().is_some() {
                                self.annotation(val.as_ref().unwrap(), ctx, span)?;
                            }
                            let inferred = self.check_expr(&expr, &cx, val.as_ref())?;
                            if val.as_ref().is_some() {
                                self.unify(&inferred, val.as_ref().unwrap(), span)?;
                            }
                            cx = cx.extend(name, val.unwrap_or(inferred));
                            (expr, span)
                        }
                        Stmt::Expr(expr) => {
                            self.check_expr(&expr, &cx, None)?;
                            (expr.clone(), expr.span())
                        }
                    };
                    // The first statement that leaves makes the rest of the block dead;
                    // the call comes before the remaining statements are checked.
                    if !leaves && self.is_diverging(e.span()) {
                        leaves = true;
                        self.dead_after(at, *span);
                    }
                }
                let ty = match expr {
                    Some(e) => self.check_expr(e, &cx, expected)?,
                    None if leaves => self.fresh_meta(), // a block that leaves fits anywhere
                    None => Ty::unit(),
                };
                if expr.as_ref().is_some_and(|e| self.is_diverging(e.span())) {
                    self.record_diverging(*span);
                }
                // Chk-Sub, then record: the end of every arm
                if let Some(exp) = expected {
                    self.unify(&ty, exp, *span)?;
                }
                let ty = self.resolve(&ty);
                self.record(*span, ty.clone());
                Ok(ty)
            }
            Expr::If(expr1, expr2, expr3, span) => {
                // the condition, checked against Bool
                self.check_expr(expr1, ctx, Some(&Ty::bool()))?;
                if self.is_diverging(expr1.span()) {
                    self.dead_after(expr1.span(), *span); // both branches are dead
                }

                // the `then` branch, checked against `expected` when there is an `else`
                // and against `None` when there is not; its type `t` is the `if`'s
                let t = match *expr3 {
                    Some(_) => {
                        let val2 = self.check_expr(expr2, ctx, expected)?;
                        let val3 =
                            self.check_expr(expr3.clone().unwrap().as_ref(), ctx, expected)?;
                        self.unify(&val2, &val3, *span)?;
                        val2
                    }
                    None => self.check_expr(expr2, ctx, Option::None)?,
                };
                match expr3 {
                    Some(e) => {
                        // the `else` branch, against `t`
                        if self.is_diverging(expr2.span()) && self.is_diverging(e.span()) {
                            self.record_diverging(*span); // every path leaves
                        }
                    }
                    None => {
                        match self.unify(&t, &Ty::unit(), *span) {
                            Ok(_) => (),
                            Err(_) => {
                                return Err(TyError::IfWithoutElse {
                                    found: t,
                                    span: *span,
                                })
                            }
                        } /* `t` must unify with (), or it is IfWithoutElse */
                    }
                }
                // Chk-Sub, then record: the end of every arm
                if let Some(exp) = expected {
                    self.unify(exp, &t, *span)?;
                }
                let t = self.resolve(&t);
                self.record(*span, t.clone());
                Ok(t)
            }
            Expr::While(expr1, expr2, span) => {
                let val1 = self.check_expr(expr1, ctx, Some(&Ty::bool()))?;
                self.unify(&val1, &Ty::bool(), *span)?;
                let val2 = self.check_expr(expr2, ctx, Some(&Ty::unit()))?;
                self.unify(&val2, &Ty::unit(), *span)?;
                self.record(*span, Ty::unit());
                Ok(Ty::unit())
            }
            Expr::For(name, expr1, expr2, span) => {
                let val1 = self.check_expr(expr1, ctx, Some(&Ty::list(Ty::self_ty())))?;
                self.unify(&val1, &Ty::list(Ty::self_ty()), *span)?;
                let cx = match val1.kind {
                    TyKind::List(ele) => ctx.extend(name.clone(), (*ele).clone()),
                    _ => ctx.clone(),
                };
                let val2 = self.check_expr(expr2, &cx, Some(&Ty::unit()))?;
                self.unify(&val2, &Ty::unit(), *span)?;
                self.record(*span, Ty::unit());
                Ok(Ty::unit())
            }
            Expr::Assign(expr1, expr2, span) => {
                let inner = self.fresh_meta();
                let expected = Ty::reference(inner.clone());
                let val1 = self.check_expr(expr1, ctx, Some(&expected))?;
                self.unify(&val1, &expected, expr1.span())?;
                let val2 = self.check_expr(expr2, ctx, Some(&inner.clone()))?;
                self.unify(&val2, &inner, expr2.span())?;
                self.record(*span, Ty::unit());
                Ok(Ty::unit())
            }
            Expr::Return(expr, span) => {
                if ctx.ret().is_none() && !ctx.in_lambda() {
                    return Err(TyError::ReturnOutsideFunction { span: *span });
                }
                let ret = ctx.ret().unwrap_or_else(|| self.fresh_meta());
                let val = self.check_expr(expr, ctx, Some(&ret))?;
                self.unify(&val, &ret, *span)?;
                self.diverging.insert(*span);
                let result_ty = self.fresh_meta();
                self.record(*span, result_ty.clone());
                Ok(result_ty)
            }
            Expr::Lambda(params, expr, span) => {
                let mut cx = ctx.enter(None, None);
                let mut param_tys = vec![];
                for param in params {
                    let param_type = param.clone().ty.unwrap_or_else(|| self.fresh_meta());
                    cx = cx.extend(param.clone().name, param_type.clone());
                    param_tys.push(param_type);
                }
                let val = match expected {
                    Some(expected_ty) => match &expected_ty.kind {
                        TyKind::Fn(_, ret) => self.check_expr(expr, &cx, Some(ret))?,
                        _ => self.check_expr(expr, &cx, None)?,
                    },
                    None => self.check_expr(expr, &cx, None)?,
                };

                let lam_type = Ty::func(param_tys, val);
                self.record(*span, lam_type.clone());
                Ok(lam_type)
            }
            Expr::Call(expr, vec_expr, span) => {
                let (val, bounds) = self.check_callee(expr, ctx)?;

                match val.clone().kind {
                    TyKind::Fn(params, ret) => {
                        if params.len() != vec_expr.len() {
                            match expr.as_ref() {
                                Expr::Var(name, span) => {
                                    return Err(TyError::ArityMismatch {
                                        callee: name.clone(),
                                        expected: params.len(),
                                        found: vec_expr.len(),
                                        span: *span,
                                    });
                                }
                                Expr::Lambda(_, _, span) => {
                                    return Err(TyError::ArityMismatch {
                                        callee: "anonymous function".to_string(),
                                        expected: params.len(),
                                        found: vec_expr.len(),
                                        span: *span,
                                    });
                                }
                                _ => (),
                            }
                        }

                        let mut param_tys = vec![];
                        for (param, arg) in params.iter().zip(vec_expr) {
                            let arg_val = self.check_expr(arg, ctx, Some(param))?;
                            self.unify(param, &arg_val, arg.span())?;
                            param_tys.push(param.clone());
                        }
                        self.finish_call(bounds, &param_tys, vec_expr, *span);
                        self.record(*span, ret.as_ref().clone());
                        Ok(ret.as_ref().clone())
                    }
                    TyKind::Meta(_id) => {
                        let mut params = vec![];
                        for _expr in vec_expr {
                            params.push(self.fresh_meta());
                        }
                        let ret: Ty = self.fresh_meta();
                        let candadite = Ty::func(params.clone(), ret.clone());
                        self.unify(&val, &candadite, *span)?;
                        let mut param_tys = vec![];
                        for (param, arg) in params.clone().iter().zip(vec_expr) {
                            let arg_val = self.check_expr(arg, ctx, Some(param))?;
                            self.unify(param, &arg_val, *span)?;
                            param_tys.push(arg_val);
                        }
                        self.finish_call(bounds, &param_tys, vec_expr, *span);
                        self.record(*span, ret.clone());
                        Ok(ret)
                    }
                    ty => Err(TyError::NotAFunction {
                        found: Ty::from(ty),
                        span: expr.span(),
                    }),
                }
            }

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
        })();
        let result = result.and_then(|ty| {
            if let Some(expected) = expected {
                self.unify(expected, &ty, e.span())?;
            }
            Ok(ty)
        });
        self.depth -= 1;
        result
    }

    /// Make `a` and `b` the same type, or fail with a `Mismatch` at `span`.
    /// While every type is known this is a structural comparison; once
    /// inference variables exist it also *solves* them (`Checker::solve`),
    /// with an occurs check so no type contains itself.
    pub fn unify(&mut self, a: &Ty, b: &Ty, span: Span) -> Result<(), TyError> {
        let a = self.resolve(a);
        let b = self.resolve(b);

        match (a.clone().kind, b.clone().kind) {
            (TyKind::Meta(id1), TyKind::Meta(id2)) => {
                if id1 != id2 {
                    self.solve(id1, b.clone());
                }
                Ok(())
            }
            (TyKind::Meta(id), _) => {
                if self.meta_in_meta(id, &b.clone()) {
                    return Err(TyError::InfiniteType { span });
                }
                self.solve(id, b.clone());
                Ok(())
            }
            (_, TyKind::Meta(id)) => {
                if self.meta_in_meta(id, &a.clone()) {
                    return Err(TyError::InfiniteType { span });
                }
                self.solve(id, a.clone());
                Ok(())
            }
            (TyKind::List(l1), TyKind::List(l2)) => self.unify(l1.as_ref(), l2.as_ref(), span),
            (TyKind::Tuple(l1), TyKind::Tuple(l2)) => {
                if l1.len() == l2.len() {
                    for (ty_l1, ty_l2) in l1.iter().zip(l2.iter()) {
                        self.unify(ty_l1, ty_l2, span)?;
                    }
                    Ok(())
                } else {
                    Err(TyError::Mismatch {
                        expected: a,
                        found: b,
                        span,
                    })
                }
            }
            (TyKind::Ref(left), TyKind::Ref(right)) => self.unify(&left, &right, span),
            (TyKind::Fn(params1, ret1), TyKind::Fn(params2, ret2)) => {
                self.unify(&ret1, &ret2, span)?;
                if params1.len() == params2.len() {
                    for (param1, param2) in params1.iter().zip(params2.iter()) {
                        self.unify(param1, param2, span)?;
                    }
                    Ok(())
                } else {
                    Err(TyError::Mismatch {
                        expected: a,
                        found: b,
                        span,
                    })
                }
            }
            (TyKind::Named(name1, args1), TyKind::Named(name2, args2)) => {
                if args1.len() == args2.len() && name1 == name2 {
                    for (arg1, arg2) in args1.iter().zip(args2.iter()) {
                        self.unify(arg1, arg2, span)?;
                    }
                    Ok(())
                } else {
                    Err(TyError::Mismatch {
                        expected: a,
                        found: b,
                        span,
                    })
                }
            }
            _ => {
                if a != b {
                    Err(TyError::Mismatch {
                        expected: a.clone(),
                        found: b.clone(),
                        span,
                    })
                } else {
                    Ok(())
                }
            }
        }
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

/// Provided helpers (Milestone M5): plumbing that no typing rule describes.
/// Paste this module at the end of the file, unchanged.
mod helpers {
    use crate::ast::{Decl, Expr, Name, Span, Ty, TyKind};
    use crate::types::{Checker, Ctx, TyError};
    use std::ops::Range;

    type Res<T> = Result<T, TyError>;
    type Bounds = Range<usize>;

    impl Checker {
        /// A type annotation written inside a body (`let x: T`, `|x: T|`): `Self`
        /// resolved to the enclosing `impl`'s type, and every name it uses checked
        /// to be a declared type or a type parameter in scope (`Self` outside an
        /// `impl` names no type).
        pub fn annotation(&self, ty: &Ty, ctx: &Ctx, span: Span) -> Res<Ty> {
            let ty = match ctx.self_ty() {
                Some(self_ty) => ty.with_self(&self_ty),
                None => ty.clone(),
            };
            self.validate_type(&ty, span)?;
            Ok(ty)
        }

        /// The type of a native prelude function (Appendix C, "Functions").
        /// `print` / `println` / `to_string` carry a `Print` bound and `contains`
        /// an `Eq` bound; until M8 `len` carries `Len` and `min` / `max` /
        /// `minimum` / `maximum` carry `Ord`, which the built-in types satisfy.
        /// `None` for a name that is not a native primitive.
        pub fn native_fn_type(&mut self, name: &str, span: Span) -> Option<Ty> {
            let opt = |t: Ty| Ty::named("Option".to_string(), vec![t]);
            let sig = match name {
                "print" | "println" => {
                    let t = self.fresh_meta();
                    self.require_bound("Print".to_string(), t.clone(), span);
                    Ty::func(vec![t], Ty::unit())
                }
                "to_string" => {
                    let t = self.fresh_meta();
                    self.require_bound("Print".to_string(), t.clone(), span);
                    Ty::func(vec![t], Ty::str())
                }
                "abs" => Ty::func(vec![Ty::int()], Ty::int()),
                "even" | "odd" => Ty::func(vec![Ty::int()], Ty::bool()),
                "range" => Ty::func(vec![Ty::int(), Ty::int()], Ty::list(Ty::int())),
                #[cfg(not(feature = "m8"))]
                "len" => {
                    let t = self.fresh_meta();
                    self.require_bound("Len".to_string(), t.clone(), span);
                    Ty::func(vec![t], Ty::int())
                }
                #[cfg(not(feature = "m8"))]
                "min" | "max" => {
                    let t = self.fresh_meta();
                    self.require_bound("Ord".to_string(), t.clone(), span);
                    Ty::func(vec![t.clone(), t.clone()], t)
                }
                #[cfg(not(feature = "m8"))]
                "minimum" | "maximum" => {
                    let t = self.fresh_meta();
                    self.require_bound("Ord".to_string(), t.clone(), span);
                    Ty::func(vec![Ty::list(t.clone())], opt(t))
                }
                "contains" => {
                    let t = self.fresh_meta();
                    self.require_bound("Eq".to_string(), t.clone(), span);
                    Ty::func(vec![Ty::list(t.clone()), t], Ty::bool())
                }
                "head" => {
                    let t = self.fresh_meta();
                    Ty::func(vec![Ty::list(t.clone())], opt(t))
                }
                "tail" => {
                    let t = self.fresh_meta();
                    Ty::func(vec![Ty::list(t.clone())], opt(Ty::list(t)))
                }
                "unwrap_or" => {
                    let t = self.fresh_meta();
                    Ty::func(vec![opt(t.clone()), t.clone()], t)
                }
                "map" => {
                    let (a, b) = (self.fresh_meta(), self.fresh_meta());
                    Ty::func(
                        vec![Ty::list(a.clone()), Ty::func(vec![a], b.clone())],
                        Ty::list(b),
                    )
                }
                "filter" => {
                    let a = self.fresh_meta();
                    let keep = Ty::func(vec![a.clone()], Ty::bool());
                    Ty::func(vec![Ty::list(a.clone()), keep], Ty::list(a))
                }
                "fold" => {
                    let (a, b) = (self.fresh_meta(), self.fresh_meta());
                    Ty::func(
                        vec![
                            Ty::list(a.clone()),
                            b.clone(),
                            Ty::func(vec![b.clone(), a], b.clone()),
                        ],
                        b,
                    )
                }
                "is_some" => {
                    let t = self.fresh_meta();
                    Ty::func(vec![opt(t)], Ty::bool())
                }
                "read_int" => Ty::func(vec![], Ty::int()),
                "read_bool" => Ty::func(vec![], Ty::bool()),
                "read_line" => Ty::func(vec![], Ty::str()),
                "read_int_opt" => Ty::func(vec![], opt(Ty::int())),
                "read_bool_opt" => Ty::func(vec![], opt(Ty::bool())),
                "read_line_opt" => Ty::func(vec![], opt(Ty::str())),
                _ => return None,
            };
            Some(sig)
        }

        /// The type of the top-level global `x`: its annotation, or the type
        /// the driver recorded for its initializer. `None` if `x` is not a
        /// global.
        pub fn global_type(&mut self, x: &Name) -> Option<Res<Ty>> {
            let (annot, init) = match self.decl(x) {
                Some(Decl::Global(annot, init)) => (annot.clone(), init.clone()),
                _ => return None,
            };
            Some(match (annot, self.global_types.get(x)) {
                (Some(ty), _) => Ok(ty),
                (None, Some(ty)) => Ok(ty.clone()),
                (None, None) => self.check_expr(&init, &Ctx::new(None, None), None),
            })
        }

        /// Synthesize the type of a call's callee, resolved, and note what the
        /// call's bookkeeping needs: a named callee is exempt from the
        /// determination rule, and the bounds the callee's type carries come
        /// back as a range to pass to `finish_call`.
        pub fn check_callee(&mut self, f: &Expr, ctx: &Ctx) -> Res<(Ty, Bounds)> {
            let start = self.obligations.len();
            let ty = self.check_expr(f, ctx, None)?;
            if matches!(f, Expr::Var(..)) {
                self.note_callee(f.span());
            }
            Ok((self.resolve(&ty), start..self.obligations.len()))
        }

        /// Call after checking a call's arguments against the parameter types
        /// `param_tys`: a bound the callee placed on a type parameter is
        /// reported at the argument whose parameter mentions it, or at the
        /// whole call if none does.
        pub fn finish_call(
            &mut self,
            bounds: Bounds,
            param_tys: &[Ty],
            args: &[Expr],
            call_span: Span,
        ) {
            for i in bounds {
                let want = &self.obligations[i].1;
                let blamed = param_tys.iter().position(|p| contains_ty(p, want));
                self.obligations[i].2 = match blamed {
                    Some(j) => args[j].span(),
                    None => call_span,
                };
            }
        }

        /// An operand whose type is still unknown where its rule needs it: in
        /// code that never runs nothing is required of it; elsewhere its type
        /// is undetermined.
        pub fn undetermined(&mut self, span: Span) -> Res<Ty> {
            if self.in_dead_code(span) {
                Ok(self.fresh_meta())
            } else {
                Err(TyError::Ambiguous { span })
            }
        }

        pub fn meta_in_meta(&mut self, id: u32, ty: &Ty) -> bool {
            contains_ty(ty, &Ty::meta(id))
        }
    }

    /// Whether `needle` occurs in `hay` (structurally, spans aside).
    fn contains_ty(hay: &Ty, needle: &Ty) -> bool {
        if hay == needle {
            return true;
        }
        let any = |ts: &[Ty]| ts.iter().any(|t| contains_ty(t, needle));
        match &hay.kind {
            TyKind::Tuple(ts) | TyKind::Named(_, ts) => any(ts),
            TyKind::List(t) | TyKind::Ref(t) => contains_ty(t, needle),
            TyKind::Fn(ps, r) => any(ps) || contains_ty(r, needle),
            _ => false,
        }
    }
}
