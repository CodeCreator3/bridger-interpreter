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
use crate::ast::{BinOp, Expr, Lit, Rule, Span, Stmt, Ty, TyKind, UnOp};

impl Checker {
    /// The type of `e` in context `ctx`, checked against `expected` when one
    /// is given. Records the result at `e`'s span.
    pub fn check_expr(
        &mut self,
        e: &Expr,
        ctx: &Ctx,
        expected: Option<&Ty>,
    ) -> Result<Ty, TyError> {
        match e {
            // ---- M5: the checker over M1–M4's expressions ----
            Expr::Lit(lit, _span) => Ok(match *lit {
                Lit::Int(_) => Ty::int(),
                Lit::Bool(_) => Ty::bool(),
                Lit::Str(_) => Ty::str(),
                Lit::Unit => Ty::unit(),
            }),
            Expr::Var(name, span) => {
                let optional = ctx.lookup(name);
                return if optional.is_none() {
                    Err(TyError::UnboundVariable {
                        name: name.clone(),
                        span: *span,
                    })
                } else {
                    Ok(optional.unwrap())
                };
            }
            Expr::Unary(un_op, expr, span) => match *un_op {
                UnOp::Neg => {
                    self.check_expr(expr, ctx, Some(&Ty::int()))?;
                    Ok(Ty::int())
                }
                UnOp::Not => {
                    self.check_expr(expr, ctx, Some(&Ty::bool()))?;
                    Ok(Ty::bool())
                }
                UnOp::Ref => Ok(self.check_expr(expr, ctx, None)?),

                UnOp::Deref => {
                    let val = self.check_expr(expr, ctx, None)?;
                    match val.kind {
                        TyKind::Ref(r) => Ok(r.as_ref().clone()),
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
                    Ok(Ty::int())
                }
                BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                    let val1 = self.check_expr(expr1, ctx, Some(&Ty::int()))?;
                    let val2 = self.check_expr(expr2, ctx, Some(&Ty::int()))?;
                    self.unify(&val1, &Ty::int(), *span)?;
                    self.unify(&val2, &Ty::int(), *span)?;
                    Ok(Ty::bool())
                }
                BinOp::Eq | BinOp::Ne => {
                    let val1 = self.check_expr(expr1, ctx, Option::None)?;
                    let val2 = self.check_expr(expr2, ctx, Option::None)?;
                    self.unify(&val1, &val2, *span)?;
                    Ok(Ty::bool())
                }
                BinOp::And | BinOp::Or => {
                    let val1 = self.check_expr(expr1, ctx, Some(&Ty::bool()))?;
                    let val2 = self.check_expr(expr2, ctx, Some(&Ty::bool()))?;
                    self.unify(&val1, &Ty::bool(), *span)?;
                    self.unify(&val2, &Ty::bool(), *span)?;
                    Ok(Ty::bool())
                }
                BinOp::Concat => {
                    let val1 = self.check_expr(expr1, ctx, Option::None)?;
                    let val2 = self.check_expr(expr2, ctx, Option::None)?;
                    self.unify(&val1, &val2, *span)?;
                    Ok(val1)
                }
                BinOp::Cons => {
                    let val2 = self.check_expr(expr2, ctx, Option::None)?;
                    match val2.clone().kind {
                        TyKind::List(list) => {
                            let val1 = self.check_expr(expr1, ctx, Some(&list.as_ref().clone()))?;
                            self.unify(&val1, &list.as_ref().clone(), *span)?;
                        }
                        _ => ()
                    }
                    Ok(val2)
                }
            },
            Expr::Tuple(vec_expr, _span) => {
                if vec_expr.len() > 0 {
                    let vec_val: Vec<_> = (*vec_expr)
                        .iter()
                        .map(|expr| self.check_expr(expr, ctx, None))
                        .collect::<Result<_, _>>()?;
                    Ok(Ty::tuple(vec_val))
                } else {
                    Ok(Ty::tuple(vec![]))
                }
            }
            Expr::List(vec_expr, span) => {
                let vec_val: Vec<_> = (*vec_expr)
                    .iter()
                    .map(|expr| self.check_expr(expr, ctx, None))
                    .collect::<Result<_, _>>()?;
                let prev_val = vec_val[0].clone();
                for val in vec_val.clone() {
                    self.unify(&val, &prev_val, *span)?;
                }
                Ok(Ty::list(vec_val[0].clone()))
            }
            Expr::Proj(expr, index, span) => {
                let val = self.check_expr(expr, ctx, Option::None)?;
                match val.kind {
                    TyKind::Tuple(val_vec) => {
                        if val_vec.len() != *index as usize {
                            return Err(TyError::NoSuchComponent {
                                index: *index,
                                arity: val_vec.len(),
                                span: *span,
                            });
                        }
                        Ok(val_vec[*index as usize].clone())
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
                            cx = cx.extend(name, val.unwrap_or(self.fresh_meta()));
                            (expr, span)
                        }
                        Stmt::Expr(expr) => (expr, *span),
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
                if self.is_diverging(expr1.span()) {
                    self.dead_after(expr1.span(), *span); // both branches are dead
                }
                self.check_expr(expr1, ctx, Some(&Ty::bool()))?;
                // the `then` branch, checked against `expected` when there is an `else`
                // and against `None` when there is not; its type `t` is the `if`'s
                let t = match *expr3 {
                    Some(_) => {
                        let val2 = self.check_expr(expr2, ctx, expected)?;
                        let val3 = self.check_expr(expr3.clone().unwrap().as_ref(), ctx, expected)?;
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
                    self.unify(&t, exp, *span)?;
                }
                let t = self.resolve(&t);
                self.record(*span, t.clone());
                Ok(t)
            }
            Expr::While(expr1, expr2, span) => {
                let val1 = self.check_expr(expr1, ctx, Some(&Ty::bool()))?;
                self.unify(&val1, &Ty::bool(), *span)?;
                let val2 = self.check_expr(expr2, ctx, Some(&Ty::unit()))?;
                self.unify(&val2, &Ty::bool(), *span)?;
                Ok(Ty::unit())
            }
            Expr::For(_, expr1, expr2, span) => {
                let val1 = self.check_expr(expr1, ctx, Some(&Ty::list(Ty::self_ty())))?;
                self.unify(&val1, &Ty::list(Ty::self_ty()), *span)?;
                let val2 = self.check_expr(expr2, ctx, Some(&Ty::unit()))?;
                self.unify(&val2, &Ty::unit(), *span)?;
                Ok(Ty::unit())
            }
            Expr::Assign(expr1, expr2, span) => {
                let inner = self.fresh_meta();
                let expected = Ty::reference(inner);
                let val1 = self.check_expr(expr1, ctx, Some(&expected))?;
                self.unify(&val1, &expected, *span)?;
                let val2 = self.check_expr(expr2, ctx, Some(&val1))?;
                self.unify(&val2, &val1, *span)?;
                Ok(Ty::unit())
            }
            Expr::Return(expr, _) => {
                let ret = ctx.ret().unwrap();
                self.check_expr(expr, ctx, Some(&ret))?;
                Ok(ret)
            }
            Expr::Lambda(_, expr, _) => {
                self.check_expr(expr, ctx, expected)?;
                Ok(Ty::unit())
            }
            Expr::Call(expr, vec_expr, span) => {
                let val = self.check_expr(expr, ctx, None)?;
                match val.kind {
                    TyKind::Fn(params, ret) => {
                        for (param, arg) in params.iter().zip(vec_expr) {
                            self.check_expr(arg, ctx, Some(&param))?;
                        }
                        Ok(ret.as_ref().clone())
                    }
                    ty => Err(TyError::NotAFunction {
                        found: Ty::from(ty),
                        span: *span,
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
        }
    }

    /// Make `a` and `b` the same type, or fail with a `Mismatch` at `span`.
    /// While every type is known this is a structural comparison; once
    /// inference variables exist it also *solves* them (`Checker::solve`),
    /// with an occurs check so no type contains itself.
    pub fn unify(&mut self, a: &Ty, b: &Ty, span: Span) -> Result<(), TyError> {
        let a = self.resolve(a);
        let b = self.resolve(b);
        match a.kind {
            TyKind::Meta(id) => {
                if self.solution(id) == None {
                    self.solve(id, b.clone())
                }
            }
            _ => (),
        }
        match b.kind {
            TyKind::Meta(id) => {
                if self.solution(id) == None {
                    self.solve(id, a.clone())
                }
            }
            _ => (),
        }

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
