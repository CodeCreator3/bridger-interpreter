//! Milestone M2 — binding: variables and blocks (public subset).  [frozen — do not edit]
//!
//! Each test builds an `Expr` by hand and calls `eval_expr`. Blocks and `let`
//! are `Expr::Block` / `Stmt::Let`; the environment is threaded by the block
//! arm, so these exercise scope without any of the later machinery.
//!
//! Gated on the `m2` feature: compiled at every milestone from M2 on.

#![cfg(feature = "m2")]

mod common;

use bridger::ast::BinOp::*;
use bridger::interp::value::Value;
use bridger::interp::RuntimeError;
use common::*;

// ---- E-Var ----

#[test]
fn a_variable_is_the_value_it_is_bound_to() {
    let env = env_with(&[("x", Value::Int(5))]);
    assert_eq!(eval_in(&var("x", 0), &env).unwrap(), Value::Int(5));
}

#[test]
fn the_nearest_binding_wins() {
    // an inner scope shadows an outer one of the same name
    let env = env_with(&[("x", Value::Int(1)), ("x", Value::Int(2))]);
    assert_eq!(eval_in(&var("x", 0), &env).unwrap(), Value::Int(2));
}

#[test]
fn an_unbound_variable_is_stuck_at_its_span() {
    // `y` at byte 3 is not in scope
    let err = stuck(&var("y", 3));
    assert!(
        matches!(err, RuntimeError::UnboundVariable { ref name, span }
        if name == "y" && span.start == 3)
    );
}

// ---- E-Let / E-Seq / E-Block / E-Empty ----

#[test]
fn a_let_binds_over_the_rest_of_the_block() {
    // { let x = 2; x + 3 }  ==>  5
    let b = block(
        vec![let_("x", int(2, 1), 0)],
        Some(binary(Add, var("x", 2), int(3, 3), 4)),
        5,
    );
    assert_eq!(value_of(&b), Value::Int(5));
}

#[test]
fn a_later_binding_sees_the_earlier_ones() {
    // { let x = 1; let y = x + 1; y }  ==>  2
    let b = block(
        vec![
            let_("x", int(1, 1), 0),
            let_("y", binary(Add, var("x", 2), int(1, 3), 4), 5),
        ],
        Some(var("y", 6)),
        7,
    );
    assert_eq!(value_of(&b), Value::Int(2));
}

#[test]
fn a_binding_rebinds_by_shadowing() {
    // { let x = 1; let x = x + 1; x }  ==>  2
    let b = block(
        vec![
            let_("x", int(1, 1), 0),
            let_("x", binary(Add, var("x", 2), int(1, 3), 4), 5),
        ],
        Some(var("x", 6)),
        7,
    );
    assert_eq!(value_of(&b), Value::Int(2));
}

#[test]
fn a_block_binding_does_not_escape_the_block() {
    // { { let x = 1; x }; x }  — the outer `x` is unbound
    let inner = block(vec![let_("x", int(1, 2), 1)], Some(var("x", 3)), 0);
    let outer = block(vec![expr_stmt(inner)], Some(var("x", 9)), 8);
    let err = stuck(&outer);
    assert!(
        matches!(err, RuntimeError::UnboundVariable { ref name, span }
        if name == "x" && span.start == 9)
    );
}

#[test]
fn a_statement_is_evaluated_for_effect_and_its_value_discarded() {
    // { 1; 2; 3 }  ==>  3   (the first two are discarded)
    let b = block(
        vec![expr_stmt(int(1, 1)), expr_stmt(int(2, 2))],
        Some(int(3, 3)),
        0,
    );
    assert_eq!(value_of(&b), Value::Int(3));
}

#[test]
fn an_empty_or_semicolon_terminated_block_is_unit() {
    assert_eq!(value_of(&block(vec![], None, 0)), Value::Unit);
    // { 5; }  — a trailing `;` leaves no tail
    assert_eq!(
        value_of(&block(vec![expr_stmt(int(5, 1))], None, 0)),
        Value::Unit
    );
}

#[test]
fn a_stuck_statement_aborts_the_block_at_its_span() {
    // { z; 1 }  — the unbound `z` gets stuck before the tail
    let b = block(vec![expr_stmt(var("z", 2))], Some(int(1, 4)), 0);
    let err = stuck(&b);
    assert!(
        matches!(err, RuntimeError::UnboundVariable { ref name, span }
        if name == "z" && span.start == 2)
    );
}
