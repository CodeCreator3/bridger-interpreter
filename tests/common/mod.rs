//! Helpers shared by the public milestone tests: short constructors for  [frozen — do not edit]
//! building an `Expr` by hand, and assertions over an evaluation's outcome.
//!
//! Every node gets its own `Span` (`sp(n)` = bytes `[n, n+1)`), so a stuck test
//! can check that the error points at the node the rule blames.

#![allow(dead_code)]

use bridger::ast::{BinOp, Expr, LambdaParam, Lit, Span, SrcId, Stmt, UnOp};
use bridger::interp::env::Env;
use bridger::interp::value::Value;
use bridger::interp::Interpreter;
use bridger::interp::{Control, RuntimeError};
use std::rc::Rc;

/// A one-byte span starting at `n`: distinct per node, cheap to write.
pub fn sp(n: usize) -> Span {
    Span {
        src: SrcId::SYNTHETIC,
        start: n,
        end: n + 1,
    }
}

pub fn int(n: i64, s: usize) -> Expr {
    Expr::Lit(Lit::Int(n), sp(s))
}
pub fn boolean(b: bool, s: usize) -> Expr {
    Expr::Lit(Lit::Bool(b), sp(s))
}
pub fn string(x: &str, s: usize) -> Expr {
    Expr::Lit(Lit::Str(x.to_string()), sp(s))
}
pub fn unit(s: usize) -> Expr {
    Expr::Lit(Lit::Unit, sp(s))
}
pub fn unary(op: UnOp, e: Expr, s: usize) -> Expr {
    Expr::Unary(op, Box::new(e), sp(s))
}
pub fn binary(op: BinOp, l: Expr, r: Expr, s: usize) -> Expr {
    Expr::Binary(op, Box::new(l), Box::new(r), sp(s))
}
pub fn tuple(es: Vec<Expr>, s: usize) -> Expr {
    Expr::Tuple(es, sp(s))
}
pub fn list(es: Vec<Expr>, s: usize) -> Expr {
    Expr::List(es, sp(s))
}
pub fn proj(e: Expr, i: u32, s: usize) -> Expr {
    Expr::Proj(Box::new(e), i, sp(s))
}
pub fn var(x: &str, s: usize) -> Expr {
    Expr::Var(x.to_string(), sp(s))
}
pub fn let_(x: &str, e: Expr, s: usize) -> Stmt {
    Stmt::Let(x.to_string(), None, e, sp(s))
}
pub fn expr_stmt(e: Expr) -> Stmt {
    Stmt::Expr(e)
}
pub fn block(stmts: Vec<Stmt>, tail: Option<Expr>, s: usize) -> Expr {
    Expr::Block(stmts, tail.map(Box::new), sp(s))
}
pub fn if_(cond: Expr, then: Expr, s: usize) -> Expr {
    Expr::If(Box::new(cond), Box::new(then), None, sp(s))
}
pub fn if_else(cond: Expr, then: Expr, els: Expr, s: usize) -> Expr {
    Expr::If(Box::new(cond), Box::new(then), Some(Box::new(els)), sp(s))
}
pub fn while_(cond: Expr, body: Expr, s: usize) -> Expr {
    Expr::While(Box::new(cond), Box::new(body), sp(s))
}
pub fn for_(x: &str, iter: Expr, body: Expr, s: usize) -> Expr {
    Expr::For(x.to_string(), Box::new(iter), Box::new(body), sp(s))
}
pub fn assign(lhs: Expr, rhs: Expr, s: usize) -> Expr {
    Expr::Assign(Box::new(lhs), Box::new(rhs), sp(s))
}
pub fn return_(e: Expr, s: usize) -> Expr {
    Expr::Return(Box::new(e), sp(s))
}
pub fn lambda(params: &[&str], body: Expr, s: usize) -> Expr {
    let params = params
        .iter()
        .map(|p| LambdaParam {
            name: p.to_string(),
            ty: None,
            span: sp(s),
        })
        .collect();
    Expr::Lambda(params, Box::new(body), sp(s))
}
pub fn call(callee: Expr, args: Vec<Expr>, s: usize) -> Expr {
    Expr::Call(Box::new(callee), args, sp(s))
}

/// Value constructors matching the `Rc` payloads of the crate's `Value`.
pub fn vstr(x: &str) -> Value {
    Value::Str(Rc::from(x))
}
pub fn vtuple(vs: Vec<Value>) -> Value {
    Value::Tuple(Rc::from(vs))
}
pub fn vlist(vs: Vec<Value>) -> Value {
    Value::List(vs.into())
}
pub fn vctor(name: &str, vs: Vec<Value>) -> Value {
    Value::Ctor(name.to_string(), Rc::from(vs))
}

/// Evaluate `e` in a fresh interpreter and an empty environment.
pub fn eval(e: &Expr) -> Result<Value, Control> {
    Interpreter::new().eval_expr(e, &Env::new())
}

/// An environment binding each `(name, value)` in order.
pub fn env_with(bindings: &[(&str, Value)]) -> Env {
    let mut env = Env::new();
    for (x, v) in bindings {
        env = env.extend((*x).to_string(), v.clone());
    }
    env
}

/// Evaluate `e` in a fresh interpreter under a given environment.
pub fn eval_in(e: &Expr, env: &Env) -> Result<Value, Control> {
    Interpreter::new().eval_expr(e, env)
}

/// Load a whole `.brg` program (on top of the prelude) and run `main`, returning
/// its value and everything it printed. Panics on a parse or runtime failure.
/// Uses `eval_program` directly, so it exercises the evaluator without the M5/M6
/// static pre-passes and keeps passing at every later milestone.
pub fn run_program(src: &str) -> (Value, String) {
    let mut it = Interpreter::new();
    it.load_program(src)
        .unwrap_or_else(|e| panic!("load error: {e}"));
    let v = it
        .eval_program()
        .unwrap_or_else(|e| panic!("runtime error: {e}"));
    (v, it.output().to_string())
}

/// Load a program, run the static checks (installing `?` conversions), then
/// run `main` — the typed pipeline. Use this for tests that need the checker's
/// elaboration, such as a cross-type `?`. Panics on any failure.
pub fn run_typed(src: &str) -> (Value, String) {
    let mut it = Interpreter::new();
    it.load_program(src)
        .unwrap_or_else(|e| panic!("load error: {e}"));
    it.check().unwrap_or_else(|e| panic!("check error: {e}"));
    let v = it
        .eval_program()
        .unwrap_or_else(|e| panic!("runtime error: {e}"));
    (v, it.output().to_string())
}

/// Load a program with `input` available to the `read_*` functions, then run
/// `main`. Panics on any failure.
pub fn run_with_input(src: &str, input: &str) -> (Value, String) {
    let mut it = Interpreter::new();
    it.load_program(src)
        .unwrap_or_else(|e| panic!("load error: {e}"));
    it.set_input(input.to_string());
    let v = it
        .eval_program()
        .unwrap_or_else(|e| panic!("runtime error: {e}"));
    (v, it.output().to_string())
}

/// Load and run a whole program expecting it to get stuck; returns the error.
pub fn run_stuck(src: &str) -> RuntimeError {
    let mut it = Interpreter::new();
    it.load_program(src)
        .unwrap_or_else(|e| panic!("load error: {e}"));
    match it.eval_program() {
        Err(e) => e,
        Ok(v) => panic!("expected a stuck program, got the value {v:?}"),
    }
}

/// Like [`run_stuck`], with `input` available to the `read_*` functions.
pub fn run_stuck_with_input(src: &str, input: &str) -> RuntimeError {
    let mut it = Interpreter::new();
    it.load_program(src)
        .unwrap_or_else(|e| panic!("load error: {e}"));
    it.set_input(input.to_string());
    match it.eval_program() {
        Err(e) => e,
        Ok(v) => panic!("expected a stuck program, got the value {v:?}"),
    }
}

/// Evaluate `e` and unwrap the value; panics if evaluation left early.
pub fn value_of(e: &Expr) -> Value {
    match eval(e) {
        Ok(v) => v,
        Err(c) => panic!("expected a value, got {c:?}"),
    }
}

/// Evaluate `e` and unwrap the stuck error; panics on a value or a `return`.
pub fn stuck(e: &Expr) -> RuntimeError {
    match eval(e) {
        Err(Control::Raise(err)) => err,
        Ok(v) => panic!("expected a stuck evaluation, got the value {v:?}"),
        Err(Control::Return(v)) => panic!("expected a stuck evaluation, got return {v:?}"),
    }
}

/// Evaluate `e` and unwrap a `return`'s value; panics on a value or a stuck
/// error. A bare `return` has no call boundary to catch it, so it surfaces here.
pub fn returned(e: &Expr) -> Value {
    match eval(e) {
        Err(Control::Return(v)) => v,
        Ok(v) => panic!("expected a return, got the value {v:?}"),
        Err(Control::Raise(err)) => panic!("expected a return, got stuck {err:?}"),
    }
}
