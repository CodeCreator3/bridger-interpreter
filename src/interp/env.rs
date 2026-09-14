//! The environment  [frozen — do not edit]
//!
//! The lexical environment maps names in scope to the values they are bound to.
//! You touch three operations:
//!
//!  - [`Env::lookup`] — find what a name is bound to (walking outward through
//!    enclosing scopes);
//!  - [`Env::extend`] — bind a name in a fresh child scope, leaving the parent
//!    untouched.
//!
//!  - [`Env::root`] — the program's root scope, which a method body (M8) and
//!    a rule's filters (M6) run over instead of the caller's locals.
//!
//! That is the whole student-facing surface. The environment is immutable from
//! your side: `extend` returns a new `Env` rather than mutating one, so nesting
//! scopes never fights the borrow checker. Values clone cheaply, so `lookup`
//! handing back an owned `Value` costs a pointer bump.
//!
//! The one mutable, recursion-tying piece — installing mutually recursive
//! top-level `fn`s so they can all see each other — lives entirely in provided
//! runtime code (`add_function`, called by `eval_program`). Student code never
//! reaches it, and never has to reason about the recursive knot.

use super::value::{Closure, Value};
use crate::ast::{Expr, Name};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// A chain of scopes. Cheap to clone (it is a reference-counted handle).
#[derive(Clone)]
pub struct Env(Rc<Scope>);

enum Scope {
    /// The top-level scope. Mutable, so the provided runtime can seed the
    /// prelude and tie mutually recursive `fn`s after the scope exists. Never
    /// exposed to student code.
    Root(RefCell<HashMap<Name, Value>>),
    /// One binding layered over a parent scope — what [`Env::extend`] builds.
    Frame { name: Name, val: Value, parent: Env },
}

impl Env {
    /// A fresh, empty top-level environment. The provided runtime builds the
    /// real starting environment (prelude + top-level `fn`s) on top of one of
    /// these; tests and book examples use it to set up a scope by hand.
    pub fn new() -> Env {
        Env(Rc::new(Scope::Root(RefCell::new(HashMap::new()))))
    }

    /// Look up `x`, searching this scope and then outward through its parents.
    /// Returns the bound value (cloned) or `None` if `x` is not in scope.
    pub fn lookup(&self, x: &str) -> Option<Value> {
        match &*self.0 {
            Scope::Frame { name, val, parent } => {
                if name == x {
                    Some(val.clone())
                } else {
                    parent.lookup(x)
                }
            }
            Scope::Root(map) => map.borrow().get(x).cloned(),
        }
    }

    /// Bind `x` to `v` in a fresh child scope and return it. `self` is unchanged.
    pub fn extend(&self, x: Name, v: Value) -> Env {
        Env(Rc::new(Scope::Frame {
            name: x,
            val: v,
            parent: self.clone(),
        }))
    }

    /// The root scope, where the prelude and the top-level `fn`s live. A rule's
    /// filters run here under the substitution (M6), and a method body runs
    /// here extended with `self` and its parameters (M8): both see the globals
    /// but not the caller's locals.
    pub fn root(&self) -> Env {
        match &*self.0 {
            Scope::Root(_) => self.clone(),
            Scope::Frame { parent, .. } => parent.root(),
        }
    }
}

impl Default for Env {
    fn default() -> Self {
        Self::new()
    }
}

// Provided runtime only — students never call these; they tie the recursive knot.
impl Env {
    /// Bind a name directly in the root scope (for prelude seeding). Panics if
    /// called on a non-root scope, which the runtime never does.
    pub(crate) fn bind_root(&self, name: Name, v: Value) {
        match &*self.0 {
            Scope::Root(map) => {
                map.borrow_mut().insert(name, v);
            }
            _ => panic!("bind_root called on a non-root scope"),
        }
    }

    /// Install a top-level `fn` as a Bridger closure that captures *this* root
    /// environment. Because every top-level `fn` captures the same root, and the
    /// root ends up holding all of them, they can all call one another — the
    /// mutual-recursion knot, tied here so no student code has to.
    pub(crate) fn add_function(&self, name: Name, params: Vec<Name>, body: Expr) {
        let closure = Value::Closure(Rc::new(Closure::Bridger {
            params,
            body,
            env: self.clone(),
        }));
        self.bind_root(name, closure);
    }
}

impl std::fmt::Debug for Env {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // An environment can be deep and cyclic (closures capture it), so we
        // never walk it here — a placeholder keeps `Value`'s `Debug` finite.
        write!(f, "<env>")
    }
}
