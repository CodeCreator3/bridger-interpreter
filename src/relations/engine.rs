//! The relations engine — YOUR file.  [student]  (Milestone M6)
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
//! base facts. The naive loop — fire every rule against all the facts derived
//! so far, add what is new, stop when a round adds nothing — is the
//! reference shape; this implementation refines it so each rule fires through
//! one generator that reads only the facts the previous round added, which
//! derives the same facts in far less time on long derivations.
//!
//! To list this milestone's holes:
//!
//! ```text
//! grep -rn 'todo_m6!' src
//! ```
//!
//! [`RelationDb`]: super::RelationDb

use super::{Subst, Term, Tuple};
use crate::ast::{Decl, Expr, Lit, Name, QArg, Query, Rule, Span, Ty};
use crate::interp::value::compare;
use crate::interp::{type_of, Control, Env, Interpreter, RuntimeError, Value};
use std::collections::{HashMap, HashSet};

/// The facts derived so far, with a set for O(1) duplicate rejection. A
/// `Value` may hold a closure, whose environment is a `RefCell`; clippy warns
/// that such a key could change, but a closure hashes by its pointer, which
/// never does (see `RelationDb` in `db.rs`).
#[allow(clippy::mutable_key_type)]
struct Pool {
    all: HashMap<Name, Vec<Tuple>>,
    seen: HashMap<Name, HashSet<Tuple>>,
}

impl Pool {
    fn new() -> Self {
        Pool {
            all: HashMap::new(),
            seen: HashMap::new(),
        }
    }

    /// Add `t` to `name`'s facts; `true` when it was new.
    #[allow(clippy::mutable_key_type)]
    fn insert(&mut self, name: &Name, t: &Tuple) -> bool {
        let s = self.seen.entry(name.clone()).or_default();
        if s.contains(t) {
            return false;
        }
        s.insert(t.clone());
        self.all.entry(name.clone()).or_default().push(t.clone());
        true
    }
}

fn lit_value(l: &Lit) -> Value {
    match l {
        Lit::Int(i) => Value::Int(*i),
        Lit::Bool(b) => Value::Bool(*b),
        Lit::Str(s) => Value::Str(s.clone().into()),
        Lit::Unit => Value::Unit,
    }
}

/// Ground a rule head whose terms are all literals (a bodiless rule's fact).
fn ground_terms(terms: &[crate::ast::Term]) -> Option<Tuple> {
    let mut t = Vec::with_capacity(terms.len());
    for term in terms {
        match term {
            crate::ast::Term::Lit(l) => t.push(lit_value(l)),
            crate::ast::Term::Var(_) => return None,
        }
    }
    Some(t)
}

impl Interpreter {
    /// Every distinct assignment to the named holes of `q` under which `q`
    /// holds — facts that differ only at a bound argument or an anonymous
    /// hole give one solution — in canonical order (`compare` over the hole
    /// values, first hole first).
    /// Bound arguments of `q` are evaluated in `env` first. A hole-free query
    /// yields either one empty substitution or none, which is how the
    /// boolean-position query reads. `span` locates the query for diagnostics.
    pub fn solutions(&mut self, q: &Query, span: Span, env: &Env) -> Result<Vec<Subst>, Control> {
        // A query asked while a fixpoint is being computed came from a rule
        // filter; report it stuck rather than recurse.
        if self.materializing {
            return Err(Control::Raise(RuntimeError::QueryInFilter { span }));
        }

        // The relation's name, resolved through the environment: a local in
        // the way is not a relation, a missing name is unbound.
        let rel_name = match env.lookup(q.name.as_str()) {
            Some(Value::Relation(name)) => name,
            Some(_) => {
                return Err(Control::Raise(RuntimeError::NotARelation {
                    name: q.name.clone(),
                    span,
                }))
            }
            None => {
                return Err(Control::Raise(RuntimeError::UnboundVariable {
                    name: q.name.clone(),
                    span,
                }))
            }
        };

        // The query's pattern: each bound argument is evaluated once in `env`;
        // a hole is a variable, an anonymous one under a fresh name no
        // identifier can spell.
        let mut terms: Vec<Term> = Vec::with_capacity(q.args.len());
        let mut anon = 0;
        for arg in &q.args {
            match arg {
                QArg::Expr(e) => terms.push(Term::Val(self.eval_expr(e, env)?)),
                QArg::Hole(Some(name)) => terms.push(Term::Var(name.clone())),
                QArg::Hole(None) => {
                    terms.push(Term::Var(format!("\0anon\0{}", anon)));
                    anon += 1;
                }
            }
        }

        // The arity.
        let Some(Decl::Relation(cols)) = self.program.decls.get(&rel_name) else {
            return Err(Control::Raise(RuntimeError::NotARelation {
                name: q.name.clone(),
                span,
            }));
        };
        if terms.len() != cols.len() {
            return Err(Control::Raise(RuntimeError::ArityMismatch {
                expected: cols.len(),
                found: terms.len(),
                span,
            }));
        }

        // The least fixpoint. The starting facts are the dynamic ones in
        // `self.db` and the head of every bodiless rule; the rules with
        // bodies are then fired in rounds until a round adds nothing. The
        // closure is remembered in the database until an `add`, `clear`, or
        // further load invalidates it.
        let rules = self.program.rules.clone();
        let mut pool = Pool::new();
        let mut memoized = false;
        if let Some(d) = self.db.derived() {
            for (n, ts) in d {
                for t in ts {
                    pool.insert(n, t);
                }
            }
            memoized = true;
        }
        if !memoized {
            let mut delta: HashMap<Name, Vec<Tuple>> = HashMap::new();
            for name in self.db.relations() {
                let name = name.to_string();
                for t in self.db.facts(&name) {
                    if pool.insert(&name, t) {
                        delta.entry(name.clone()).or_default().push(t.clone());
                    }
                }
            }
            for rule in &rules {
                if rule.body.is_empty() {
                    if let Some(t) = ground_terms(&rule.head.terms) {
                        if pool.insert(&rule.head.name, &t) {
                            delta.entry(rule.head.name.clone()).or_default().push(t);
                        }
                    }
                }
            }

            // Set `materializing` before the rounds and clear it before any
            // error from them is returned, so one stuck filter does not leave
            // every later query stuck.
            self.materializing = true;
            let rounds = self.run_rounds(&rules, env, &mut pool, &mut delta);
            self.materializing = false;
            rounds?;
            self.db.set_derived(pool.all.clone());
        }

        // The answer: the query's pattern matched against every fact of its
        // relation, cut down to the named holes — distinct, in order of first
        // appearance — with duplicates removed and the rest sorted in
        // canonical order.
        let mut named: Vec<Name> = Vec::new();
        for arg in &q.args {
            if let QArg::Hole(Some(n)) = arg {
                if !named.iter().any(|x| x == n) {
                    named.push(n.clone());
                }
            }
        }
        let mut sols: Vec<Subst> = Vec::new();
        if let Some(facts) = pool.all.get(&rel_name) {
            for fact in facts {
                if let Some(s) = Subst::new().unify(&terms, fact) {
                    let mut cut = Subst::new();
                    for h in &named {
                        if let Some(v) = s.get(h) {
                            cut.bind(h.clone(), v.clone());
                        }
                    }
                    sols.push(cut);
                }
            }
        }
        sols.sort_by(|a, b| {
            for h in &named {
                if let (Some(va), Some(vb)) = (a.get(h), b.get(h)) {
                    let o = compare(va, vb);
                    if o != std::cmp::Ordering::Equal {
                        return o;
                    }
                }
            }
            std::cmp::Ordering::Equal
        });
        sols.dedup();
        Ok(sols)
    }

    /// Fire the rules in rounds: each rule joins through one **active**
    /// generator — the first of its generators whose relation gained facts
    /// the previous round — reading the other generators' facts in full, then
    /// evaluates its filters. Stops when a round adds nothing.
    fn run_rounds(
        &mut self,
        rules: &[Rule],
        env: &Env,
        pool: &mut Pool,
        delta: &mut HashMap<Name, Vec<Tuple>>,
    ) -> Result<(), Control> {
        loop {
            let mut next_delta: HashMap<Name, Vec<Tuple>> = HashMap::new();
            for rule in rules {
                if rule.body.is_empty() {
                    continue;
                }
                // A conjunct is a generator when it is a call whose callee
                // names a declared relation, over variables and literals.
                let mut gens: Vec<Option<Name>> = Vec::with_capacity(rule.body.len());
                for conjunct in &rule.body {
                    gens.push(match conjunct {
                        Expr::Call(callee, args, _) => match callee.as_ref() {
                            Expr::Var(name, _)
                                if args
                                    .iter()
                                    .all(|a| matches!(a, Expr::Var(_, _) | Expr::Lit(_, _)))
                                    && matches!(
                                        self.program.decls.get(name),
                                        Some(Decl::Relation(_))
                                    ) =>
                            {
                                Some(name.clone())
                            }
                            _ => None,
                        },
                        _ => None,
                    });
                }
                // Every generator joins before any filter runs: a filter may
                // read a variable bound by a generator written after it, so
                // order generators and filters separately (both in source
                // order). Semi-naive: fire the rule through each generator
                // position whose relation gained facts this round, scanning
                // the delta there and the full relation elsewhere.
                let active_positions: Vec<usize> = (0..rule.body.len())
                    .filter(|&i| {
                        matches!(&gens[i], Some(n) if delta.get(n).is_some_and(|v| !v.is_empty()))
                    })
                    .collect();
                // No generator saw new facts: the rule's output is unchanged.
                if active_positions.is_empty() {
                    continue;
                }
                for active in active_positions {
                    // The join: one empty substitution, extended through each
                    // generator in source order.
                    let mut subs = vec![Subst::new()];
                    for (i, conjunct) in rule.body.iter().enumerate() {
                        if subs.is_empty() {
                            break;
                        }
                        let Some(gname) = &gens[i] else {
                            continue; // a filter; handled below
                        };
                        let args = match conjunct {
                            Expr::Call(_, args, _) => args,
                            _ => unreachable!(),
                        };
                        let terms: Vec<Term> = args
                            .iter()
                            .map(|a| match a {
                                Expr::Var(n, _) => Term::Var(n.clone()),
                                Expr::Lit(l, _) => Term::Val(lit_value(l)),
                                _ => unreachable!(),
                            })
                            .collect();
                        let empty: Vec<Tuple> = Vec::new();
                        let scan = if i == active {
                            delta.get(gname).unwrap()
                        } else {
                            pool.all.get(gname).unwrap_or(&empty)
                        };
                        let mut out = Vec::new();
                        for fact in scan {
                            for sub in &subs {
                                if let Some(u) = sub.unify(&terms, fact) {
                                    out.push(u);
                                }
                            }
                        }
                        subs = out;
                    }
                    // The filters, left to right, over the joined
                    // substitutions.
                    for (i, conjunct) in rule.body.iter().enumerate() {
                        if subs.is_empty() {
                            break;
                        }
                        if gens[i].is_some() {
                            continue; // a generator; handled above
                        }
                        let mut out = Vec::new();
                        for sub in &subs {
                            if self.filter_bool(conjunct, sub, env)? {
                                out.push(sub.clone());
                            }
                        }
                        subs = out;
                    }

                    // Each surviving substitution grounds the head.
                    for sub in subs {
                        let mut t = Vec::with_capacity(rule.head.terms.len());
                        let mut ok = true;
                        for term in &rule.head.terms {
                            match term {
                                crate::ast::Term::Var(n) => match sub.get(n) {
                                    Some(v) => t.push(v.clone()),
                                    None => {
                                        ok = false;
                                        break;
                                    }
                                },
                                crate::ast::Term::Lit(l) => t.push(lit_value(l)),
                            }
                        }
                        if ok && pool.insert(&rule.head.name, &t) {
                            next_delta
                                .entry(rule.head.name.clone())
                                .or_default()
                                .push(t);
                        }
                    }
                }
            }
            if next_delta.values().all(|v| v.is_empty()) {
                return Ok(());
            }
            *delta = next_delta;
        }
    }

    /// Evaluate one filter under `sub` over the root environment: `true`
    /// keeps the substitution, `false` drops it, anything else is stuck.
    fn filter_bool(&mut self, e: &Expr, sub: &Subst, env: &Env) -> Result<bool, Control> {
        let mut fenv = env.root();
        for (name, val) in sub.iter() {
            fenv = fenv.extend(name.clone(), val.clone());
        }
        match self.eval_expr(e, &fenv) {
            Ok(Value::Bool(b)) => Ok(b),
            Ok(other) => Err(Control::Raise(RuntimeError::TypeError {
                expected: Ty::bool(),
                found: type_of(&other),
                span: e.span(),
            })),
            // A `return` escaping a filter has no function to return from.
            Err(Control::Return(_)) => Err(Control::Raise(RuntimeError::ReturnOutsideFunction {
                span: e.span(),
            })),
            Err(ctrl) => Err(ctrl),
        }
    }
}
