//! The relation database  [frozen — do not edit]  (comes alive at M6)
//!
//! The **dynamic facts** of a program: what `add R(v, …)` has inserted and
//! `clear R` has not yet removed. The reference calls this Δ, the mutable
//! overlay the evaluation judgment threads beside the store. The permanent
//! base — bodiless `rule R(…);` facts and the rules that derive new tuples —
//! lives in the loaded [`Program`](crate::ast::Program); the engine you write
//! in Part VII reads both and computes the least fixpoint per query.
//!
//! Each relation's facts are a **set** in insertion order: adding a tuple that
//! is already present changes nothing, and `add` says whether it did.
//! Membership is a hash lookup, so an `add` costs the same however many facts
//! the relation holds.
//!
//! The database also keeps room for the **derived closure** M(P, Δ) an engine
//! computes from it ([`RelationDb::derived`]): every query reads the same
//! closure until the next `add` or `clear` (or a further `load_program`, which
//! may add rules), so an engine may store the closure
//! it computed and answer the next query from it. Storing it is optional — an
//! engine that recomputes per query is correct, only slower.

use crate::ast::Name;
use crate::interp::Value;
use std::collections::{HashMap, HashSet};

/// One dynamic fact: the values of a relation's columns, in order.
pub type Tuple = Vec<Value>;

/// The facts of one relation: a set, kept in insertion order. (A `Value` may
/// hold a closure, whose environment is a `RefCell`; clippy warns that such a
/// key could change, but a closure hashes by its pointer, which never does.)
#[allow(clippy::mutable_key_type)]
#[derive(Debug, Default)]
struct FactSet {
    rows: Vec<Tuple>,
    seen: HashSet<Tuple>,
}

/// The dynamic-fact store, keyed by relation name.
#[derive(Debug, Default)]
pub struct RelationDb {
    facts: HashMap<Name, FactSet>,
    /// The derived closure of the current facts, if an engine stored one
    /// since the last change.
    derived: Option<HashMap<Name, Vec<Tuple>>>,
}

impl RelationDb {
    /// An empty database.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert `tuple` into `relation`. Returns `true` if it was not already
    /// present, so an `add` can tell whether Δ changed. Derived facts do not
    /// go here — this is Δ, which `clear` resets — the engine keeps its
    /// closure apart and may park it with [`RelationDb::set_derived`].
    pub fn add(&mut self, relation: &str, tuple: Tuple) -> bool {
        let facts = self.facts.entry(relation.to_string()).or_default();
        if facts.seen.contains(&tuple) {
            false
        } else {
            facts.seen.insert(tuple.clone());
            facts.rows.push(tuple);
            self.derived = None;
            true
        }
    }

    /// Remove every dynamic fact of `relation`. Base facts and rules in the
    /// program are untouched: `clear` resets what `add` built, nothing else.
    pub fn clear(&mut self, relation: &str) {
        if let Some(facts) = self.facts.get_mut(relation) {
            if !facts.rows.is_empty() {
                facts.rows.clear();
                facts.seen.clear();
                self.derived = None;
            }
        }
    }

    /// The dynamic facts of `relation`, in insertion order; empty if none.
    pub fn facts(&self, relation: &str) -> &[Tuple] {
        self.facts
            .get(relation)
            .map(|f| f.rows.as_slice())
            .unwrap_or(&[])
    }

    /// The relations that hold at least one dynamic fact.
    pub fn relations(&self) -> impl Iterator<Item = &str> {
        self.facts
            .iter()
            .filter(|(_, facts)| !facts.rows.is_empty())
            .map(|(name, _)| name.as_str())
    }

    /// The derived closure an engine stored with [`RelationDb::set_derived`],
    /// if the facts have not changed since.
    pub fn derived(&self) -> Option<&HashMap<Name, Vec<Tuple>>> {
        self.derived.as_ref()
    }

    /// Remember the derived closure of the current facts, to be handed back by
    /// [`RelationDb::derived`] until the next `add` or `clear`.
    pub fn set_derived(&mut self, derived: HashMap<Name, Vec<Tuple>>) {
        self.derived = Some(derived);
    }

    /// Forget the derived closure: the rules it was computed from changed.
    pub fn invalidate(&mut self) {
        self.derived = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_is_a_set_insert_and_clear_empties_one_relation() {
        let mut db = RelationDb::new();
        assert!(db.add("edge", vec![Value::Int(0), Value::Int(1)]));
        assert!(!db.add("edge", vec![Value::Int(0), Value::Int(1)]));
        assert!(db.add("edge", vec![Value::Int(1), Value::Int(2)]));
        assert!(db.add("node", vec![Value::Int(0)]));
        assert_eq!(db.facts("edge").len(), 2);
        assert_eq!(db.facts("edge")[0], vec![Value::Int(0), Value::Int(1)]);
        assert!(db.facts("missing").is_empty());
        db.clear("edge");
        assert!(db.facts("edge").is_empty());
        assert_eq!(db.relations().collect::<Vec<_>>(), ["node"]);
    }

    #[test]
    fn the_derived_closure_is_kept_until_the_facts_change() {
        let mut db = RelationDb::new();
        db.add("edge", vec![Value::Int(0), Value::Int(1)]);
        assert!(db.derived().is_none());
        db.set_derived(HashMap::new());
        assert!(db.derived().is_some());
        assert!(!db.add("edge", vec![Value::Int(0), Value::Int(1)]));
        assert!(db.derived().is_some()); // nothing changed
        db.add("edge", vec![Value::Int(1), Value::Int(2)]);
        assert!(db.derived().is_none());
        db.set_derived(HashMap::new());
        db.clear("edge");
        assert!(db.derived().is_none());
    }

    #[test]
    fn many_adds_stay_cheap() {
        let mut db = RelationDb::new();
        for i in 0..200_000 {
            assert!(db.add("n", vec![Value::Int(i)]));
        }
        assert!(!db.add("n", vec![Value::Int(7)]));
        assert_eq!(db.facts("n").len(), 200_000);
    }
}
