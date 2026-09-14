//! Runtime values  [frozen — do not edit]
//!
//! A `Value` is what evaluating an expression produces. Its shape follows the
//! book's *Values* chapter: compound payloads use `Rc<[Value]>` / `Rc<str>`
//! rather than `Vec` / `String`, so `Value: Clone` is a cheap pointer bump,
//! which is what lets [`Store::read`](crate::interp::store::Store::read) hand
//! a `Value` back by value; and a list is a [`List`] of cons cells with shared
//! tails, the shape the evaluation rules take apart.
//! The two non-data variants, `Relation` and `Type`, are what a relation name
//! and a bare type name evaluate to (Parts VII and IX); the chapter lists them
//! with the rest.

use super::env::Env;
use super::prelude::BuiltinId;
use super::store::Loc;
use crate::ast::{Expr, Name, Ty};
use std::rc::Rc;

/// The result of evaluating an expression.
#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Bool(bool),
    Str(Rc<str>),
    Unit,
    /// `(v, v, …)`, two or more
    Tuple(Rc<[Value]>),
    /// `[v, …]`: the empty list or a cons cell, see [`List`]
    List(List),
    /// a constructor value: `C(v, …)`
    Ctor(Name, Rc<[Value]>),
    /// a struct value: `S { f = v, … }`
    Struct(Name, Rc<[(Name, Value)]>),
    /// a location in the store (Part IV)
    Ref(Loc),
    /// a function together with the environment it captured (Part V)
    Closure(Rc<Closure>),
    /// a declared relation, bound at the root under its name (Part VII). It is
    /// resolved like any other name, so a local may shadow it; applying it is
    /// a query, and nothing else consumes it.
    Relation(Name),
    /// a declared struct or type, named as the receiver of an associated
    /// call (`Point.origin()`, Part IX); like a relation, it is a name
    /// resolved through the program, and nothing else consumes it.
    Type(Name),
}

/// A list value: the empty list, or a **cons cell** — a head joined to a
/// shared tail — which is exactly the shape Appendix D's rules take a list
/// apart in. Because tails are shared, `x :: xs`, the pattern `h :: t`,
/// `head`, and `tail` each cost one step and allocate one cell at most; a
/// list literal or `++` costs its left length; the length is stored in each
/// cell, so `len` is one step too. There is no indexing: reading the *n*th
/// element is a walk, as the rules say.
#[derive(Debug, Clone, Default)]
pub struct List(Option<Rc<Cons>>);

/// One cell of a [`List`].
#[derive(Debug)]
pub struct Cons {
    head: Value,
    tail: List,
    len: usize,
}

impl List {
    /// The empty list `[]`.
    pub fn nil() -> List {
        List(None)
    }
    /// `head :: tail`.
    pub fn cons(head: Value, tail: List) -> List {
        let len = tail.len() + 1;
        List(Some(Rc::new(Cons { head, tail, len })))
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_none()
    }
    pub fn len(&self) -> usize {
        self.0.as_ref().map_or(0, |c| c.len)
    }
    /// The first element, if any.
    pub fn head(&self) -> Option<&Value> {
        self.0.as_ref().map(|c| &c.head)
    }
    /// The list after its first element, if any.
    pub fn tail(&self) -> Option<&List> {
        self.0.as_ref().map(|c| &c.tail)
    }
    /// The head and the tail together — the `h :: t` pattern.
    pub fn uncons(&self) -> Option<(&Value, &List)> {
        self.0.as_ref().map(|c| (&c.head, &c.tail))
    }
    /// The elements, first to last.
    pub fn iter(&self) -> Iter<'_> {
        Iter(self)
    }
    /// `self ++ other`: the elements of `self` consed onto `other`, so the
    /// right operand is shared and the cost is the left one's length.
    pub fn concat(&self, other: &List) -> List {
        let left: Vec<Value> = self.iter().cloned().collect();
        left.into_iter()
            .rev()
            .fold(other.clone(), |tail, v| List::cons(v, tail))
    }
}

/// The elements of a [`List`], first to last.
pub struct Iter<'a>(&'a List);

impl<'a> Iterator for Iter<'a> {
    type Item = &'a Value;
    fn next(&mut self) -> Option<&'a Value> {
        let (h, t) = self.0.uncons()?;
        self.0 = t;
        Some(h)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.0.len(), Some(self.0.len()))
    }
}

impl<'a> IntoIterator for &'a List {
    type Item = &'a Value;
    type IntoIter = Iter<'a>;
    fn into_iter(self) -> Iter<'a> {
        self.iter()
    }
}

/// Build a list from its elements in order: `[v, …]`.
impl FromIterator<Value> for List {
    fn from_iter<I: IntoIterator<Item = Value>>(items: I) -> List {
        let items: Vec<Value> = items.into_iter().collect();
        items
            .into_iter()
            .rev()
            .fold(List::nil(), |tail, v| List::cons(v, tail))
    }
}

impl From<Vec<Value>> for List {
    fn from(items: Vec<Value>) -> List {
        items.into_iter().collect()
    }
}

impl From<Rc<[Value]>> for List {
    fn from(items: Rc<[Value]>) -> List {
        items.iter().cloned().collect()
    }
}

impl PartialEq for List {
    fn eq(&self, other: &List) -> bool {
        self.len() == other.len() && self.iter().zip(other.iter()).all(|(a, b)| a == b)
    }
}

/// Dropping a long list one cell after another would recurse once per cell;
/// unlink the tails iteratively instead, as far as this list owns them.
impl Drop for Cons {
    fn drop(&mut self) {
        let mut next = std::mem::take(&mut self.tail);
        while let Some(rc) = next.0.take() {
            match Rc::try_unwrap(rc) {
                Ok(mut cell) => next = std::mem::take(&mut cell.tail),
                Err(_) => break,
            }
        }
    }
}

/// A callable value. To apply a [`Closure::Bridger`] you read its three fields:
/// extend `env` with the arguments bound to `params`, then evaluate `body`. A
/// [`Closure::Native`] is a prelude primitive the provided `eval_prelude` runs.
#[derive(Debug)]
pub enum Closure {
    Bridger {
        params: Vec<Name>,
        body: Expr,
        env: Env,
    },
    Native(BuiltinId),
}

/// A value's **shallow** runtime type, used to fill `RuntimeError::TypeError`'s
/// `found` field for diagnostics. It is exact for scalars and tuples; a closure
/// reports a bare function type, and an empty list or a reference has no known
/// element type, so a placeholder stands in. Grading matches the error variant
/// and its span, not this type, so the placeholders are harmless.
pub fn type_of(v: &Value) -> Ty {
    match v {
        Value::Int(_) => Ty::int(),
        Value::Bool(_) => Ty::bool(),
        Value::Str(_) => Ty::str(),
        Value::Unit => Ty::unit(),
        Value::Tuple(vs) => Ty::tuple(vs.iter().map(type_of).collect()),
        Value::List(vs) => Ty::list(vs.head().map(type_of).unwrap_or_else(Ty::unit)),
        Value::Ctor(name, _) | Value::Struct(name, _) => Ty::named(name.clone(), Vec::new()),
        Value::Ref(_) => Ty::reference(Ty::unit()),
        // A closure's parameter types are not known at run time; the
        // written shape stands for "a function" in a diagnostic.
        Value::Closure(_) => Ty::named("fn(…)".to_string(), Vec::new()),
        // No written type names a relation; this is for diagnostics only.
        Value::Relation(name) => Ty::named(format!("relation {name}"), Vec::new()),
        Value::Type(name) => Ty::named(format!("type {name}"), Vec::new()),
    }
}

/// The **canonical order** on values: the total order behind a query's
/// solutions (Appendix D, E-Solutions), so the same set of facts yields the
/// same list however the facts were derived. Integers numerically; `false`
/// before `true`; strings lexicographically; tuples and lists
/// lexicographically, a shorter prefix first; constructor values by name,
/// then arguments; struct values by name, then fields in declaration order;
/// references by location. Across kinds: Int < Bool < String < () < tuple <
/// list < constructor < struct < reference; the non-data values (functions,
/// relations, types) sort last and equal to one another. Provided here so a
/// relation engine can sort solutions with `sols.sort_by(..)`.
pub fn compare(a: &Value, b: &Value) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    use Value::*;
    fn rank(v: &Value) -> u8 {
        match v {
            Int(_) => 0,
            Bool(_) => 1,
            Str(_) => 2,
            Unit => 3,
            Tuple(_) => 4,
            List(_) => 5,
            Ctor(..) => 6,
            Struct(..) => 7,
            Ref(_) => 8,
            Closure(_) | Relation(_) | Type(_) => 9,
        }
    }
    fn seq<'a>(
        xs: impl Iterator<Item = &'a Value>,
        ys: impl Iterator<Item = &'a Value>,
    ) -> Ordering {
        let (mut xs, mut ys) = (xs, ys);
        loop {
            match (xs.next(), ys.next()) {
                (None, None) => return Ordering::Equal,
                (None, Some(_)) => return Ordering::Less,
                (Some(_), None) => return Ordering::Greater,
                (Some(x), Some(y)) => match compare(x, y) {
                    Ordering::Equal => continue,
                    o => return o,
                },
            }
        }
    }
    match (a, b) {
        (Int(x), Int(y)) => x.cmp(y),
        (Bool(x), Bool(y)) => x.cmp(y),
        (Str(x), Str(y)) => x.cmp(y),
        (Unit, Unit) => Ordering::Equal,
        (Tuple(xs), Tuple(ys)) => seq(xs.iter(), ys.iter()),
        (List(xs), List(ys)) => seq(xs.iter(), ys.iter()),
        (Ctor(n, xs), Ctor(m, ys)) => n.cmp(m).then_with(|| seq(xs.iter(), ys.iter())),
        (Struct(n, xs), Struct(m, ys)) => n
            .cmp(m)
            .then_with(|| seq(xs.iter().map(|(_, v)| v), ys.iter().map(|(_, v)| v))),
        (Ref(x), Ref(y)) => x.cmp(y),
        _ => rank(a).cmp(&rank(b)),
    }
}

/// Structural equality, for the harness and test assertions. Data compares by
/// value; two closures compare by identity (`Rc` pointer). This is *not* the
/// language's `==` — that is `E-Eq`, which you implement in `eval_expr` and
/// which raises `NotComparable` on a function rather than comparing pointers.
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        use Value::*;
        match (self, other) {
            (Int(a), Int(b)) => a == b,
            (Bool(a), Bool(b)) => a == b,
            (Str(a), Str(b)) => a == b,
            (Unit, Unit) => true,
            (Tuple(a), Tuple(b)) => a == b,
            (List(a), List(b)) => a == b,
            (Ctor(n, a), Ctor(m, b)) => n == m && a == b,
            (Struct(n, a), Struct(m, b)) => n == m && a == b,
            (Ref(a), Ref(b)) => a == b,
            (Closure(a), Closure(b)) => Rc::ptr_eq(a, b),
            (Relation(a), Relation(b)) | (Type(a), Type(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for Value {}

/// Hashing agrees with `==`: structural over data, by location for a
/// reference, by identity (the pointer) for a closure, by name for a relation
/// or type name. What lets a fact set and a solution set be hashed.
impl std::hash::Hash for Value {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        use Value::*;
        std::mem::discriminant(self).hash(state);
        match self {
            Int(n) => n.hash(state),
            Bool(b) => b.hash(state),
            Str(s) => s.hash(state),
            Unit => {}
            Tuple(vs) => vs.hash(state),
            List(vs) => {
                vs.len().hash(state);
                for v in vs.iter() {
                    v.hash(state);
                }
            }
            Ctor(n, vs) => {
                n.hash(state);
                vs.hash(state);
            }
            Struct(n, fs) => {
                n.hash(state);
                for (f, v) in fs.iter() {
                    f.hash(state);
                    v.hash(state);
                }
            }
            Ref(loc) => loc.hash(state),
            Closure(c) => Rc::as_ptr(c).hash(state),
            Relation(n) | Type(n) => n.hash(state),
        }
    }
}

/// The type of the first component of `v` that admits no equality — a
/// function, relation, or type name at any depth — or `None` when `v` is
/// data all the way down. What `==` and `contains` refuse.
pub fn uncomparable(v: &Value) -> Option<Ty> {
    match v {
        Value::Closure(_) | Value::Relation(_) | Value::Type(_) => Some(type_of(v)),
        Value::Tuple(vs) | Value::Ctor(_, vs) => vs.iter().find_map(uncomparable),
        Value::List(vs) => vs.iter().find_map(uncomparable),
        Value::Struct(_, fs) => fs.iter().find_map(|(_, v)| uncomparable(v)),
        _ => None,
    }
}
