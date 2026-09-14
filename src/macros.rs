//! Milestone hole markers  [frozen — do not edit]
//!
//! Every place you must write code is a `todo_mN!` for the milestone that owns
//! it. Each expands to a diverging `unimplemented!`, so it type-checks in any
//! position and the crate always compiles; reaching one at run time panics with
//! a message naming the milestone and the rule (e.g. "M3: E-If").
//!
//! To see exactly what a milestone asks you to write:
//!
//! ```text
//! grep -rn 'todo_m3!' src
//! ```
//!
//! lists every hole due at M3, and only those.

/// Hole owned by Milestone 1 (expressions: literals, operators, tuples, lists).
#[macro_export]
macro_rules! todo_m1 {
    ($what:literal) => {
        ::std::unimplemented!(concat!("M1: ", $what))
    };
    () => {
        ::std::unimplemented!("M1: not yet implemented")
    };
}

/// Hole owned by Milestone 2 (binding: variables, `let`, block scope).
#[macro_export]
macro_rules! todo_m2 {
    ($what:literal) => {
        ::std::unimplemented!(concat!("M2: ", $what))
    };
    () => {
        ::std::unimplemented!("M2: not yet implemented")
    };
}

/// Hole owned by Milestone 3 (state & control: `if`/`while`/`for`, `ref`, `return`).
#[macro_export]
macro_rules! todo_m3 {
    ($what:literal) => {
        ::std::unimplemented!(concat!("M3: ", $what))
    };
    () => {
        ::std::unimplemented!("M3: not yet implemented")
    };
}

/// Hole owned by Milestone 4 (functions: lambdas, calls, closures).
#[macro_export]
macro_rules! todo_m4 {
    ($what:literal) => {
        ::std::unimplemented!(concat!("M4: ", $what))
    };
    () => {
        ::std::unimplemented!("M4: not yet implemented")
    };
}

/// Hole owned by Milestone 5 (types: the `check_expr` pass).
#[macro_export]
macro_rules! todo_m5 {
    ($what:literal) => {
        ::std::unimplemented!(concat!("M5: ", $what))
    };
    () => {
        ::std::unimplemented!("M5: not yet implemented")
    };
}

/// Hole owned by Milestone 6 (relations: Datalog `add`/`clear`/query, unification).
#[macro_export]
macro_rules! todo_m6 {
    ($what:literal) => {
        ::std::unimplemented!(concat!("M6: ", $what))
    };
    () => {
        ::std::unimplemented!("M6: not yet implemented")
    };
}

/// Hole owned by Milestone 7 (algebraic data types: constructors, `match`).
#[macro_export]
macro_rules! todo_m7 {
    ($what:literal) => {
        ::std::unimplemented!(concat!("M7: ", $what))
    };
    () => {
        ::std::unimplemented!("M7: not yet implemented")
    };
}

/// Hole owned by Milestone 8 (objects: structs, fields, method dispatch).
#[macro_export]
macro_rules! todo_m8 {
    ($what:literal) => {
        ::std::unimplemented!(concat!("M8: ", $what))
    };
    () => {
        ::std::unimplemented!("M8: not yet implemented")
    };
}
