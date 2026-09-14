//! The Bridger interpreter — student starter crate.  [frozen — do not edit]
//!
//! You build one interpreter that grows all semester. The plumbing that fights
//! the borrow checker (the environment, the store, the relation database, the
//! parser) is provided and frozen; you write the *meaning* of the language, one
//! evaluation rule at a time, in `interp/eval.rs`.
//!
//! ## Where your work lives
//!
//! Every hole you must fill is a milestone-tagged macro — `todo_m1!`, …,
//! `todo_m8!` (defined in `src/macros.rs`). To list a milestone's holes:
//!
//! ```text
//! grep -rn 'todo_m3!' src
//! ```
//!
//! ## Layout
//!
//! The crate is one directory per pass. `parser/` turns text into the `ast`;
//! `interp/` evaluates it (values, environment, store, the evaluator);
//! `types/` checks it (Part VI); `relations/` holds the Datalog fragment's
//! store and matching (Part VII). Frozen modules carry the types and plumbing;
//! the files you edit are `interp/eval.rs` (the evaluator, M1–M8),
//! `types/check.rs` (the type checker, M5+), and at M6 `relations/unify.rs`,
//! `relations/engine.rs`, and `relations/check.rs` (matching, the fixpoint
//! engine, and the static checks on rules). The public tests in
//! `tests/` are gated by milestone: `cargo m3` runs every test in force at
//! Milestone 3 (see `Cargo.toml`'s `[features]`).

#[macro_use]
mod macros;

pub mod ast;
pub mod interp;
pub mod parser;
pub mod relations;
pub mod types;
