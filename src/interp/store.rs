//! The store  [frozen — do not edit]  (comes alive at M3)
//!
//! Mutable state in Bridger lives here, not in Rust variables. `ref e` allocates
//! a fresh cell, `deref e` reads one, `e := e` overwrites one. The store is a
//! monotonic arena: allocation only ever hands back a *fresh* location (this is
//! what `E-Ref` requires), and there is deliberately no way to free a cell.

use super::value::Value;

/// A location in the store — a small `Copy` handle, not a Rust pointer. You get
/// one from [`Store::alloc`] and pass it to [`Store::read`] / [`Store::write`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Loc(usize);

/// The store: an arena of mutable cells, each holding one [`Value`].
#[derive(Debug, Default)]
pub struct Store {
    cells: Vec<Value>,
}

impl Store {
    /// An empty store.
    pub fn new() -> Self {
        Store { cells: Vec::new() }
    }

    /// Allocate a fresh cell holding `v` and return its location.
    pub fn alloc(&mut self, v: Value) -> Loc {
        let i = self.cells.len();
        self.cells.push(v);
        Loc(i)
    }

    /// Read the value in `l`, cloning it out. `Value` is cheap to clone, so this
    /// sidesteps the read-then-write borrow conflict a `&Value` would create.
    pub fn read(&self, l: Loc) -> Value {
        self.cells[l.0].clone()
    }

    /// Overwrite the value in `l`. Panics only on a location this store never
    /// issued — which the evaluator never produces.
    pub fn write(&mut self, l: Loc, v: Value) {
        self.cells[l.0] = v;
    }
}
