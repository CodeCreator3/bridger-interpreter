//! Milestone M3 — state and control (public subset).  [frozen — do not edit]
//!
//! `if` / `while` / `for`, `ref` / `deref` / `:=` over the store, and `return`
//! through the `Control::Return` channel. Loops and the store are built by hand
//! out of the M1/M2 forms.
//!
//! Gated on the `m3` feature: compiled at every milestone from M3 on.

#![cfg(feature = "m3")]

mod common;

use bridger::ast::BinOp::*;
use bridger::ast::{Expr, UnOp};
use bridger::interp::value::Value;
use bridger::interp::RuntimeError;
use common::*;

fn ref_(e: Expr, s: usize) -> Expr {
    unary(UnOp::Ref, e, s)
}
fn deref(e: Expr, s: usize) -> Expr {
    unary(UnOp::Deref, e, s)
}

// ---- E-If ----

#[test]
fn if_takes_the_branch_the_condition_selects() {
    assert_eq!(
        value_of(&if_else(boolean(true, 1), int(1, 2), int(2, 3), 0)),
        Value::Int(1)
    );
    assert_eq!(
        value_of(&if_else(boolean(false, 1), int(1, 2), int(2, 3), 0)),
        Value::Int(2)
    );
}

#[test]
fn an_else_less_if_is_unit_when_false() {
    assert_eq!(value_of(&if_(boolean(false, 1), int(9, 2), 0)), Value::Unit);
    assert_eq!(value_of(&if_(boolean(true, 1), unit(2), 0)), Value::Unit);
}

#[test]
fn a_non_boolean_condition_is_stuck_at_the_if() {
    let err = stuck(&if_else(int(3, 1), int(1, 2), int(2, 3), 0));
    assert!(matches!(err, RuntimeError::TypeError { span, .. } if span.start == 0));
}

// ---- E-Ref / E-Deref / E-Assign ----

#[test]
fn deref_reads_back_what_ref_stored() {
    // deref (ref 5)  ==>  5
    assert_eq!(value_of(&deref(ref_(int(5, 2), 1), 0)), Value::Int(5));
}

#[test]
fn assignment_updates_the_cell() {
    // { let r = ref 0; r := 7; deref r }  ==>  7
    let b = block(
        vec![
            let_("r", ref_(int(0, 2), 1), 0),
            expr_stmt(assign(var("r", 3), int(7, 4), 5)),
        ],
        Some(deref(var("r", 6), 7)),
        8,
    );
    assert_eq!(value_of(&b), Value::Int(7));
}

#[test]
fn aliases_share_a_cell() {
    // { let r = ref 1; let s = r; s := 9; deref r }  ==>  9
    let b = block(
        vec![
            let_("r", ref_(int(1, 2), 1), 0),
            let_("s", var("r", 3), 4),
            expr_stmt(assign(var("s", 5), int(9, 6), 7)),
        ],
        Some(deref(var("r", 8), 9)),
        10,
    );
    assert_eq!(value_of(&b), Value::Int(9));
}

#[test]
fn dereferencing_a_non_reference_is_stuck() {
    let err = stuck(&deref(int(5, 1), 0));
    assert!(matches!(err, RuntimeError::TypeError { span, .. } if span.start == 0));
}

#[test]
fn assigning_through_a_non_reference_is_stuck() {
    let err = stuck(&assign(int(5, 1), int(6, 2), 0));
    assert!(matches!(err, RuntimeError::TypeError { span, .. } if span.start == 0));
}

// ---- E-While ----

#[test]
fn while_iterates_until_the_condition_is_false() {
    // { let i = ref 0; while deref i < 3 { i := deref i + 1 }; deref i }  ==>  3
    let body = assign(
        var("i", 10),
        binary(Add, deref(var("i", 11), 12), int(1, 13), 14),
        15,
    );
    let loop_ = while_(binary(Lt, deref(var("i", 6), 7), int(3, 8), 9), body, 5);
    let b = block(
        vec![let_("i", ref_(int(0, 2), 1), 0), expr_stmt(loop_)],
        Some(deref(var("i", 16), 17)),
        18,
    );
    assert_eq!(value_of(&b), Value::Int(3));
}

#[test]
fn a_false_condition_runs_the_body_zero_times() {
    assert_eq!(
        value_of(&while_(boolean(false, 1), int(1, 2), 0)),
        Value::Unit
    );
}

#[test]
fn a_non_boolean_while_condition_is_stuck() {
    let err = stuck(&while_(int(1, 1), unit(2), 0));
    assert!(matches!(err, RuntimeError::TypeError { span, .. } if span.start == 0));
}

// ---- E-For ----

#[test]
fn for_walks_a_list_binding_each_element() {
    // { let acc = ref 0; for x in [1,2,3] { acc := deref acc + x }; deref acc }  ==>  6
    let body = assign(
        var("acc", 20),
        binary(Add, deref(var("acc", 21), 22), var("x", 23), 24),
        25,
    );
    let loop_ = for_("x", list(vec![int(1, 6), int(2, 7), int(3, 8)], 5), body, 9);
    let b = block(
        vec![let_("acc", ref_(int(0, 2), 1), 0), expr_stmt(loop_)],
        Some(deref(var("acc", 30), 31)),
        32,
    );
    assert_eq!(value_of(&b), Value::Int(6));
}

#[test]
fn for_over_an_empty_list_runs_the_body_zero_times() {
    assert_eq!(
        value_of(&for_("x", list(vec![], 1), var("x", 2), 0)),
        Value::Unit
    );
}

#[test]
fn iterating_a_non_list_is_stuck() {
    let err = stuck(&for_("x", int(5, 1), unit(2), 0));
    assert!(matches!(err, RuntimeError::TypeError { span, .. } if span.start == 0));
}

// ---- E-Return (no call boundary yet: it surfaces at the top level) ----

#[test]
fn return_leaves_through_the_return_channel() {
    assert_eq!(returned(&return_(int(5, 1), 0)), Value::Int(5));
}

#[test]
fn return_abandons_an_enclosing_loop() {
    // while true { return 7 }
    let loop_ = while_(boolean(true, 1), return_(int(7, 3), 2), 0);
    assert_eq!(returned(&loop_), Value::Int(7));
    // for x in [1,2,3] { return x }  — returns on the first element
    let f = for_(
        "x",
        list(vec![int(1, 2), int(2, 3), int(3, 4)], 1),
        return_(var("x", 6), 5),
        0,
    );
    assert_eq!(returned(&f), Value::Int(1));
}
