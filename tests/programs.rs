//! Whole programs, one per part of the book, run through the checked pipeline  [frozen — do not edit]
//! (`Interpreter::run`: parse, type check, rule checks, evaluate) and compared
//! against their recorded output. Each `tests/programs/NAME.brg` has a
//! `NAME.out` (what it prints) and, where it reads input, a `NAME.in`.
//!
//! They came from a review that wrote realistic, hand-verified programs; a
//! change to the language that alters any of these outputs is worth a look.
//!
//! Gated on the `m8` feature: the programs use every part's constructs.

#![cfg(feature = "m8")]

use bridger::interp::Interpreter;
use std::fs;
use std::path::Path;

fn run_recorded(name: &str) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/programs");
    let src = fs::read_to_string(dir.join(format!("{name}.brg"))).expect("program");
    let want = fs::read_to_string(dir.join(format!("{name}.out"))).expect("recorded output");
    let input = fs::read_to_string(dir.join(format!("{name}.in"))).unwrap_or_default();
    let mut it = Interpreter::new();
    it.set_input(input);
    it.run(&src).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!(it.output(), want, "{name}");
}

#[test]
fn a_expressions() {
    run_recorded("a_expr");
}
#[test]
fn b_bindings_and_control() {
    run_recorded("b_control");
}
#[test]
fn c_functions_and_closures() {
    run_recorded("c_closures");
}
#[test]
fn d_relations() {
    run_recorded("d_relations");
}
#[test]
fn e_algebraic_data_types() {
    run_recorded("e_adt");
}
#[test]
fn f_objects() {
    run_recorded("f_objects");
}
#[test]
fn g_stack_machine_with_input() {
    run_recorded("g_stackvm");
}
