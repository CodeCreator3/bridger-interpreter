//! Milestone M1 — the expression evaluator (public subset).  [frozen — do not edit]
//!
//! Each test builds an `Expr` by hand and calls `eval_expr` on it: one rule at a
//! time, no parser. Stuck tests check the error's **variant and span**, never
//! its message.
//!
//! Gated on the `m1` feature: this file is compiled at every milestone from M1
//! on (the features are cumulative), so a later milestone still runs it. A
//! test whose expectation a later milestone revises carries a
//! `#[cfg(not(feature = "mN"))]` and its replacement lives in `mN.rs`.

#![cfg(feature = "m1")]

mod common;

use bridger::ast::{BinOp::*, Ty, TyKind, UnOp};
use bridger::interp::value::Value;
use bridger::interp::RuntimeError;
use common::*;

// ---- E-Lit ----

#[test]
fn literals_evaluate_to_themselves() {
    assert_eq!(value_of(&int(5, 0)), Value::Int(5));
    assert_eq!(value_of(&boolean(true, 0)), Value::Bool(true));
    assert_eq!(value_of(&string("hi", 0)), vstr("hi"));
    assert_eq!(value_of(&unit(0)), Value::Unit);
}

// ---- E-Arith / E-Div ----

#[test]
fn arithmetic_on_integers() {
    assert_eq!(
        value_of(&binary(Add, int(1, 0), int(2, 1), 2)),
        Value::Int(3)
    );
    assert_eq!(
        value_of(&binary(Sub, int(1, 0), int(2, 1), 2)),
        Value::Int(-1)
    );
    assert_eq!(
        value_of(&binary(Mul, int(3, 0), int(4, 1), 2)),
        Value::Int(12)
    );
}

#[test]
fn division_and_modulo_truncate_toward_zero() {
    assert_eq!(
        value_of(&binary(Div, int(7, 0), int(2, 1), 2)),
        Value::Int(3)
    );
    assert_eq!(
        value_of(&binary(Div, int(-7, 0), int(2, 1), 2)),
        Value::Int(-3)
    );
    assert_eq!(
        value_of(&binary(Mod, int(7, 0), int(2, 1), 2)),
        Value::Int(1)
    );
    assert_eq!(
        value_of(&binary(Mod, int(-7, 0), int(2, 1), 2)),
        Value::Int(-1)
    );
}

#[test]
fn nested_arithmetic() {
    // (1 + 2) * 4
    let e = binary(Mul, binary(Add, int(1, 0), int(2, 1), 2), int(4, 3), 4);
    assert_eq!(value_of(&e), Value::Int(12));
}

#[test]
fn division_by_zero_is_stuck_at_the_operator() {
    let err = stuck(&binary(Div, int(1, 0), int(0, 1), 2));
    assert!(matches!(err, RuntimeError::DivByZero { span } if span == sp(2)));
    let err = stuck(&binary(Mod, int(1, 0), int(0, 1), 2));
    assert!(matches!(err, RuntimeError::DivByZero { span } if span == sp(2)));
}

#[test]
fn arithmetic_on_a_non_integer_is_a_type_error() {
    // 1 + true: expected Int, found Bool, blamed on the `+`
    let err = stuck(&binary(Add, int(1, 0), boolean(true, 1), 2));
    assert!(matches!(
        err,
        RuntimeError::TypeError { expected, found, span } if expected == Ty::int() && found == Ty::bool() && span == sp(2)
    ));
    // "a" * 2: the offending operand is the first one, left to right
    let err = stuck(&binary(Mul, string("a", 0), int(2, 1), 2));
    assert!(matches!(
        err,
        RuntimeError::TypeError { expected, found, span } if expected == Ty::int() && found == Ty::str() && span == sp(2)
    ));
}

#[test]
fn a_stuck_operand_carries_its_own_span_outward() {
    // 1 + (2 / 0): the inner division is what got stuck
    let e = binary(Add, int(1, 0), binary(Div, int(2, 1), int(0, 2), 3), 4);
    let err = stuck(&e);
    assert!(matches!(err, RuntimeError::DivByZero { span } if span == sp(3)));
}

// ---- E-Neg / E-Not ----

#[test]
fn negation_and_not() {
    assert_eq!(value_of(&unary(UnOp::Neg, int(5, 0), 1)), Value::Int(-5));
    let twice = unary(UnOp::Neg, unary(UnOp::Neg, int(5, 0), 1), 2);
    assert_eq!(value_of(&twice), Value::Int(5));
    assert_eq!(
        value_of(&unary(UnOp::Not, boolean(true, 0), 1)),
        Value::Bool(false)
    );
}

#[test]
fn negation_and_not_on_the_wrong_shape_are_type_errors() {
    let err = stuck(&unary(UnOp::Neg, boolean(true, 0), 1));
    assert!(matches!(
        err,
        RuntimeError::TypeError { expected, found, span } if expected == Ty::int() && found == Ty::bool() && span == sp(1)
    ));
    let err = stuck(&unary(UnOp::Not, int(1, 0), 1));
    assert!(matches!(
        err,
        RuntimeError::TypeError { expected, found, span } if expected == Ty::bool() && found == Ty::int() && span == sp(1)
    ));
}

// ---- E-Ord ----

#[test]
fn ordering_on_integers() {
    assert_eq!(
        value_of(&binary(Lt, int(1, 0), int(2, 1), 2)),
        Value::Bool(true)
    );
    assert_eq!(
        value_of(&binary(Le, int(2, 0), int(2, 1), 2)),
        Value::Bool(true)
    );
    assert_eq!(
        value_of(&binary(Gt, int(2, 0), int(3, 1), 2)),
        Value::Bool(false)
    );
    assert_eq!(
        value_of(&binary(Ge, int(2, 0), int(3, 1), 2)),
        Value::Bool(false)
    );
}

#[test]
fn ordering_is_integer_only() {
    let err = stuck(&binary(Lt, string("a", 0), string("b", 1), 2));
    assert!(matches!(
        err,
        RuntimeError::TypeError { expected, found, span } if expected == Ty::int() && found == Ty::str() && span == sp(2)
    ));
}

// ---- E-Eq ----

#[test]
fn equality_is_structural() {
    assert_eq!(
        value_of(&binary(Eq, int(1, 0), int(1, 1), 2)),
        Value::Bool(true)
    );
    assert_eq!(
        value_of(&binary(Ne, int(1, 0), int(1, 1), 2)),
        Value::Bool(false)
    );
    assert_eq!(
        value_of(&binary(Eq, string("a", 0), string("a", 1), 2)),
        Value::Bool(true)
    );
    assert_eq!(
        value_of(&binary(Eq, unit(0), unit(1), 2)),
        Value::Bool(true)
    );
    // (1, [2]) == (1, [2])
    let lhs = tuple(vec![int(1, 0), list(vec![int(2, 1)], 2)], 3);
    let rhs = tuple(vec![int(1, 4), list(vec![int(2, 5)], 6)], 7);
    assert_eq!(value_of(&binary(Eq, lhs, rhs, 8)), Value::Bool(true));
}

#[test]
fn values_of_different_shapes_are_unequal_not_stuck() {
    assert_eq!(
        value_of(&binary(Eq, int(1, 0), boolean(true, 1), 2)),
        Value::Bool(false)
    );
    assert_eq!(
        value_of(&binary(Ne, int(1, 0), string("1", 1), 2)),
        Value::Bool(true)
    );
}

// ---- E-And / E-Or / E-Not ----

#[test]
fn boolean_connectives() {
    for (a, b, and, or) in [
        (false, false, false, false),
        (false, true, false, true),
        (true, false, false, true),
        (true, true, true, true),
    ] {
        assert_eq!(
            value_of(&binary(And, boolean(a, 0), boolean(b, 1), 2)),
            Value::Bool(and)
        );
        assert_eq!(
            value_of(&binary(Or, boolean(a, 0), boolean(b, 1), 2)),
            Value::Bool(or)
        );
    }
}

#[test]
fn and_and_or_short_circuit() {
    // false and (1 / 0): the right operand is never evaluated
    let e = binary(
        And,
        boolean(false, 0),
        binary(Div, int(1, 1), int(0, 2), 3),
        4,
    );
    assert_eq!(value_of(&e), Value::Bool(false));
    // true or (1 / 0)
    let e = binary(
        Or,
        boolean(true, 0),
        binary(Div, int(1, 1), int(0, 2), 3),
        4,
    );
    assert_eq!(value_of(&e), Value::Bool(true));
}

#[test]
fn connectives_on_a_non_boolean_are_type_errors() {
    // 1 and true: the left operand is checked first
    let err = stuck(&binary(And, int(1, 0), boolean(true, 1), 2));
    assert!(matches!(
        err,
        RuntimeError::TypeError { expected, found, span } if expected == Ty::bool() && found == Ty::int() && span == sp(2)
    ));
    // true and 1: the right operand is checked once reached
    let err = stuck(&binary(And, boolean(true, 0), int(1, 1), 2));
    assert!(matches!(
        err,
        RuntimeError::TypeError { expected, found, span } if expected == Ty::bool() && found == Ty::int() && span == sp(2)
    ));
    // false or "x"
    let err = stuck(&binary(Or, boolean(false, 0), string("x", 1), 2));
    assert!(matches!(
        err,
        RuntimeError::TypeError { expected, found, span } if expected == Ty::bool() && found == Ty::str() && span == sp(2)
    ));
}

// ---- E-Concat ----

#[test]
fn concat_joins_strings_and_lists() {
    assert_eq!(
        value_of(&binary(Concat, string("ab", 0), string("cd", 1), 2)),
        vstr("abcd")
    );
    let e = binary(
        Concat,
        list(vec![int(1, 0)], 1),
        list(vec![int(2, 2), int(3, 3)], 4),
        5,
    );
    assert_eq!(
        value_of(&e),
        vlist(vec![Value::Int(1), Value::Int(2), Value::Int(3)])
    );
    let e = binary(Concat, list(vec![], 0), list(vec![], 1), 2);
    assert_eq!(value_of(&e), vlist(vec![]));
}

#[test]
fn concat_needs_two_operands_of_one_shape() {
    let err = stuck(&binary(Concat, string("a", 0), list(vec![int(1, 1)], 2), 3));
    assert!(matches!(err, RuntimeError::TypeError { span, .. } if span == sp(3)));
    let err = stuck(&binary(Concat, list(vec![int(1, 0)], 1), string("a", 2), 3));
    assert!(matches!(err, RuntimeError::TypeError { span, .. } if span == sp(3)));
    let err = stuck(&binary(Concat, int(1, 0), int(2, 1), 2));
    assert!(matches!(err, RuntimeError::TypeError { span, .. } if span == sp(2)));
}

// ---- E-Cons ----

#[test]
fn cons_prepends_to_a_list() {
    let e = binary(Cons, int(1, 0), list(vec![int(2, 1), int(3, 2)], 3), 4);
    assert_eq!(
        value_of(&e),
        vlist(vec![Value::Int(1), Value::Int(2), Value::Int(3)])
    );
    let e = binary(Cons, string("a", 0), list(vec![], 1), 2);
    assert_eq!(value_of(&e), vlist(vec![vstr("a")]));
    // 1 :: 2 :: [] — right-associative, so the tree nests on the right
    let e = binary(
        Cons,
        int(1, 0),
        binary(Cons, int(2, 1), list(vec![], 2), 3),
        4,
    );
    assert_eq!(value_of(&e), vlist(vec![Value::Int(1), Value::Int(2)]));
}

#[test]
fn cons_onto_a_non_list_is_a_type_error() {
    let err = stuck(&binary(Cons, int(1, 0), int(2, 1), 2));
    assert!(matches!(
        err,
        RuntimeError::TypeError { expected, found, span } if matches!(expected.kind, TyKind::List(_)) && found == Ty::int() && span == sp(2)
    ));
}

// ---- E-Tuple / E-List / E-Proj ----

#[test]
fn tuples_and_lists_collect_their_parts() {
    let e = tuple(vec![int(1, 0), boolean(true, 1)], 2);
    assert_eq!(value_of(&e), vtuple(vec![Value::Int(1), Value::Bool(true)]));
    let e = list(vec![int(1, 0), binary(Add, int(1, 1), int(1, 2), 3)], 4);
    assert_eq!(value_of(&e), vlist(vec![Value::Int(1), Value::Int(2)]));
    assert_eq!(value_of(&list(vec![], 0)), vlist(vec![]));
}

#[test]
fn projection_reads_a_tuple_component() {
    let pair = || tuple(vec![int(1, 0), string("x", 1)], 2);
    assert_eq!(value_of(&proj(pair(), 0, 3)), Value::Int(1));
    assert_eq!(value_of(&proj(pair(), 1, 3)), vstr("x"));
}

#[test]
fn projection_out_of_range_or_off_a_non_tuple_is_stuck() {
    let pair = tuple(vec![int(1, 0), int(2, 1)], 2);
    let err = stuck(&proj(pair, 2, 3));
    assert!(
        matches!(err, RuntimeError::NoSuchField { ref field, span } if field == "2" && span == sp(3))
    );
    let err = stuck(&proj(int(5, 0), 0, 1));
    assert!(
        matches!(err, RuntimeError::NoSuchField { ref field, span } if field == "0" && span == sp(1))
    );
    // lists have no projection
    let err = stuck(&proj(list(vec![int(1, 0)], 1), 0, 2));
    assert!(matches!(err, RuntimeError::NoSuchField { span, .. } if span == sp(2)));
}

#[test]
fn a_stuck_part_stops_the_whole_tuple() {
    let e = tuple(vec![binary(Div, int(1, 0), int(0, 1), 2), int(3, 3)], 4);
    let err = stuck(&e);
    assert!(matches!(err, RuntimeError::DivByZero { span } if span == sp(2)));
}
