//! Terms and substitutions for the relations sublanguage  [frozen — do not edit]
//! (comes alive at M6)
//!
//! A relation's facts are ground tuples of values; a rule body or a query
//! names them through **terms**: logic variables and constants. Matching a
//! term against a fact either extends a [`Subst`] — the bindings found so
//! far — or fails. You write the matching in `unify.rs`; the data lives here.

use crate::ast::Name;
use crate::interp::Value;
use std::collections::HashMap;

/// A term in a rule atom or a query: a logic variable, or a ground value.
#[derive(Debug, Clone, PartialEq)]
pub enum Term {
    /// a logic variable, named as in the rule (`x`) or the query hole (`?x`)
    Var(Name),
    /// a constant: a literal in a rule, or an evaluated argument in a query
    Val(Value),
}

/// A substitution: the values the logic variables have been bound to. Grows
/// as a rule body's atoms are matched left to right; a fresh one per firing.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Subst {
    bindings: HashMap<Name, Value>,
}

impl Subst {
    /// The empty substitution.
    pub fn new() -> Self {
        Self::default()
    }

    /// The value bound to `x`, if any.
    pub fn get(&self, x: &str) -> Option<&Value> {
        self.bindings.get(x)
    }

    /// Bind `x` to `v`. Overwrites; check first if the variable may already be
    /// bound and the two values must agree.
    pub fn bind(&mut self, x: Name, v: Value) {
        self.bindings.insert(x, v);
    }

    /// Apply the substitution to `t`: a bound variable becomes its value, an
    /// unbound one stays a variable.
    pub fn apply(&self, t: &Term) -> Term {
        match t {
            Term::Var(x) => match self.bindings.get(x) {
                Some(v) => Term::Val(v.clone()),
                None => t.clone(),
            },
            Term::Val(_) => t.clone(),
        }
    }

    /// The bound variables, with their values, in no particular order.
    pub fn iter(&self) -> impl Iterator<Item = (&Name, &Value)> {
        self.bindings.iter()
    }

    /// The number of bound variables.
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// Whether nothing is bound.
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }
}
