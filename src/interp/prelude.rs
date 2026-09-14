//! The prelude  [frozen — do not edit]
//!
//! Built-in values every program starts with, in two kinds:
//!
//!  - **Bridger-written** declarations ship as source in `prelude.brg` — the
//!    ADTs and the prelude traits — and, from M8, the bounded functions in
//!    `prelude-bounds.brg`; both are loaded by
//!    [`Interpreter::new`](crate::interp::Interpreter::new).
//!  - **Native primitives** are Rust: I/O, the list primitives, the
//!    higher-order `map`/`filter`/`fold` (which iterate, so a long list costs
//!    no stack), and until M8 the ad-hoc–overloaded functions whose signatures
//!    the language cannot yet write (`len` over a `String` or a `[T]`,
//!    `min`/`max` over any `Ord`; Appendix C). They are stored as
//!    [`Closure::Native`] values and run
//!    by the provided `Interpreter::eval_prelude`, so they flow through the
//!    same `Call` path as any other function.
//!
//! `print` / `println` write through `Interpreter::emit`: into a buffer whole-
//! program tests read back, or, once the `bridger` binary has turned on
//! `stream_output`, straight to stdout as they happen.
//!
//! Everything below `native_bindings` is reached only through
//! `Interpreter::eval_prelude`, which the `Call` arm of the evaluator calls
//! once it dispatches a [`Closure::Native`] — that arrives at M4, so until then
//! the dispatch half of this file has no caller in a `cargo build`.
#![allow(dead_code)] // native primitives come alive when the M4 `Call` arm dispatches to them

use super::env::Env;
use super::store::Loc;
use super::value::{type_of, uncomparable, List};
use super::{Closure, Control, Interpreter, RuntimeError, Value};
use crate::ast::{Expr, Span, Ty};
use std::cmp::Ordering;
use std::rc::Rc;

/// Identifies one native prelude primitive. Opaque to student code; the provided
/// `Interpreter::eval_prelude` maps it to the Rust implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BuiltinId(pub(crate) u32);

/// Each native primitive, with its stable id (the discriminant) and the name it
/// binds to in the starting environment. The `Call` arm of the evaluator (M4)
/// dispatches a [`Closure::Native`] here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
enum Native {
    Print,
    Println,
    ToString,
    Abs,
    Even,
    Odd,
    Len,
    Min,
    Max,
    Minimum,
    Maximum,
    Contains,
    Head,
    Tail,
    Range,
    UnwrapOr,
    IsSome,
    /// `map`, `filter`, `fold`: iterate the list natively, applying the
    /// function they are handed once per element through the evaluator's own
    /// `Call` arm — so a long list costs no stack, whatever the frame size
    Map,
    Filter,
    Fold,
    ReadInt,
    ReadBool,
    ReadLine,
    ReadIntOpt,
    ReadBoolOpt,
    ReadLineOpt,
    /// `Ord::cmp` for the built-in `Ord` types — a method, never bound by name
    Cmp,
    /// `Len::length` for the built-in `Len` types — a method, never bound by name
    Length,
}

impl Native {
    /// Name → primitive, the table `eval_program` seeds the environment from.
    const ALL: &'static [(&'static str, Native)] = &[
        ("print", Native::Print),
        ("println", Native::Println),
        ("to_string", Native::ToString),
        ("abs", Native::Abs),
        ("even", Native::Even),
        ("odd", Native::Odd),
        ("len", Native::Len),
        ("min", Native::Min),
        ("max", Native::Max),
        ("minimum", Native::Minimum),
        ("maximum", Native::Maximum),
        ("contains", Native::Contains),
        ("head", Native::Head),
        ("tail", Native::Tail),
        ("range", Native::Range),
        ("unwrap_or", Native::UnwrapOr),
        ("is_some", Native::IsSome),
        ("map", Native::Map),
        ("filter", Native::Filter),
        ("fold", Native::Fold),
        ("read_int", Native::ReadInt),
        ("read_bool", Native::ReadBool),
        ("read_line", Native::ReadLine),
        ("read_int_opt", Native::ReadIntOpt),
        ("read_bool_opt", Native::ReadBoolOpt),
        ("read_line_opt", Native::ReadLineOpt),
        ("cmp", Native::Cmp),
        ("length", Native::Length),
    ];

    /// The natives that stand in for the bounded prelude functions until M8:
    /// `min`/`max`/`minimum`/`maximum` (bound `Ord`) and `len` (bound `Len`).
    /// From M8 the same names are ordinary Bridger in `prelude-bounds.brg`,
    /// written over the trait methods, so a user `impl Ord`/`impl Len` flows
    /// through them; until then they are native over the built-in types only.
    const BOUNDED: &'static [Native] = &[
        Native::Len,
        Native::Min,
        Native::Max,
        Native::Minimum,
        Native::Maximum,
    ];

    /// The trait methods the built-in types conform with, resolved by
    /// [`builtin_method`] rather than bound in the environment.
    const METHODS: &'static [Native] = &[Native::Cmp, Native::Length];

    fn id(self) -> BuiltinId {
        BuiltinId(self as u32)
    }

    fn from_id(id: BuiltinId) -> Native {
        // The reverse of `id()`; the runtime only ever hands back an id it issued
        // from `ALL`, so an unknown one is an internal invariant break.
        Native::ALL
            .iter()
            .find(|(_, n)| n.id() == id)
            .map(|(_, n)| *n)
            .expect("unknown BuiltinId")
    }
}

/// The names the native primitives are bound to at this milestone — reserved:
/// a program may not declare one at top level, any more than it may redeclare
/// a Bridger-written prelude name.
pub(crate) fn native_names() -> impl Iterator<Item = &'static str> {
    Native::ALL
        .iter()
        .filter(|(_, n)| !Native::METHODS.contains(n))
        .filter(|(_, n)| cfg!(not(feature = "m8")) || !Native::BOUNDED.contains(n))
        .map(|(name, _)| *name)
}

/// The `(name, value)` bindings for every native primitive, for `eval_program`
/// to install in the root scope before it adds the program's own `fn`s.
pub(crate) fn native_bindings() -> Vec<(String, Value)> {
    Native::ALL
        .iter()
        .filter(|(_, n)| !Native::METHODS.contains(n))
        .filter(|(_, n)| cfg!(not(feature = "m8")) || !Native::BOUNDED.contains(n))
        .map(|(name, n)| {
            (
                (*name).to_string(),
                Value::Closure(Rc::new(Closure::Native(n.id()))),
            )
        })
        .collect()
}

/// The native behind a method call on a built-in value — the run-time twin of
/// the checker's built-in conformances: `Int`/`String`/`Bool` are `Ord` (so
/// have `cmp`), `String` and lists are `Len` (so have `length`). `None` when
/// the built-in types give `receiver` no method `m`; a user `impl` is looked up
/// first by the evaluator, so this is the fallback. Call the result through
/// [`Interpreter::eval_prelude`] with the receiver as the first argument.
pub(crate) fn builtin_method(receiver: &Value, m: &str) -> Option<BuiltinId> {
    let native = match (m, receiver) {
        ("cmp", Value::Int(_) | Value::Str(_) | Value::Bool(_)) => Native::Cmp,
        ("length", Value::Str(_) | Value::List(_)) => Native::Length,
        _ => return None,
    };
    Some(native.id())
}

impl Interpreter {
    /// Run one native primitive on already-evaluated arguments. Called by the
    /// `Call` arm of `eval_expr` when the callee is a [`Closure::Native`]; `span`
    /// is the call site's, so a stuck primitive blames the right node. Errors are
    /// plain [`RuntimeError`]s; the `Call` arm lifts them into `Control` with `?`.
    pub(crate) fn eval_prelude(
        &mut self,
        id: BuiltinId,
        args: Vec<Value>,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        use Native::*;
        match Native::from_id(id) {
            Print => {
                let s = self.format_value(&arg1(args, span)?, false, span)?;
                self.emit(&s);
                Ok(Value::Unit)
            }
            Println => {
                let mut s = self.format_value(&arg1(args, span)?, false, span)?;
                s.push('\n');
                self.emit(&s);
                Ok(Value::Unit)
            }
            ToString => {
                let s = self.format_value(&arg1(args, span)?, false, span)?;
                Ok(Value::Str(Rc::from(s.as_str())))
            }
            Abs => match arg1(args, span)? {
                Value::Int(n) => Ok(Value::Int(n.wrapping_abs())),
                v => Err(type_err(Ty::int(), &v, span)),
            },
            Even => match arg1(args, span)? {
                Value::Int(n) => Ok(Value::Bool(n % 2 == 0)),
                v => Err(type_err(Ty::int(), &v, span)),
            },
            Odd => match arg1(args, span)? {
                Value::Int(n) => Ok(Value::Bool(n % 2 != 0)),
                v => Err(type_err(Ty::int(), &v, span)),
            },
            // `len` (until M8) and the `Len::length` conformance behind it, over
            // `String` and `[T]` (Appendix C); the
            // `expected` type on the error is a convention (grading matches the
            // variant and span, not the message).
            Len | Length => match arg1(args, span)? {
                Value::Str(s) => Ok(Value::Int(s.chars().count() as i64)),
                Value::List(xs) => Ok(Value::Int(xs.len() as i64)),
                v => Err(type_err(Ty::str(), &v, span)),
            },
            // `Ord::cmp` on the built-in `Ord` types, as an `Ordering` value.
            Cmp => {
                let (a, b) = arg2(args, span)?;
                let ord = value_cmp(&a, &b).ok_or_else(|| not_ordered(&a, &b, span))?;
                let name = match ord {
                    Ordering::Less => "Less",
                    Ordering::Equal => "Equal",
                    Ordering::Greater => "Greater",
                };
                Ok(Value::Ctor(name.into(), Rc::from([])))
            }
            Min | Max => {
                let (a, b) = arg2(args, span)?;
                let ord = value_cmp(&a, &b).ok_or_else(|| not_ordered(&a, &b, span))?;
                let take_a = match Native::from_id(id) {
                    Min => ord != Ordering::Greater,
                    _ => ord != Ordering::Less,
                };
                Ok(if take_a { a } else { b })
            }
            Minimum | Maximum => match arg1(args, span)? {
                Value::List(xs) => {
                    let want_min = Native::from_id(id) == Minimum;
                    let mut best: Option<Value> = None;
                    for x in xs.iter() {
                        best = Some(match best {
                            None => x.clone(),
                            Some(b) => {
                                let ord =
                                    value_cmp(&b, x).ok_or_else(|| not_ordered(&b, x, span))?;
                                let keep_b = if want_min {
                                    ord != Ordering::Greater
                                } else {
                                    ord != Ordering::Less
                                };
                                if keep_b {
                                    b
                                } else {
                                    x.clone()
                                }
                            }
                        });
                    }
                    Ok(option(best))
                }
                v => Err(type_err(Ty::list(Ty::int()), &v, span)),
            },
            Contains => {
                let (xs, x) = arg2(args, span)?;
                let xs = match xs {
                    Value::List(xs) => xs,
                    v => return Err(type_err(Ty::list(type_of(&x)), &v, span)),
                };
                // Structural `==`, which a function admits no more here than
                // as an operand of `==`.
                if let Some(found) = uncomparable(&x).or_else(|| xs.iter().find_map(uncomparable)) {
                    return Err(RuntimeError::NotComparable { found, span });
                }
                Ok(Value::Bool(xs.iter().any(|v| *v == x)))
            }
            Head => match arg1(args, span)? {
                Value::List(xs) => Ok(option(xs.head().cloned())),
                v => Err(type_err(Ty::list(Ty::int()), &v, span)),
            },
            Tail => match arg1(args, span)? {
                Value::List(xs) => Ok(option(xs.tail().map(|t| Value::List(t.clone())))),
                v => Err(type_err(Ty::list(Ty::int()), &v, span)),
            },
            Range => {
                let (lo, hi) = arg2(args, span)?;
                match (lo, hi) {
                    (Value::Int(lo), Value::Int(hi)) => {
                        // Bounded: a typo'd bound must not try to allocate
                        // billions of elements.
                        const LIMIT: usize = 1 << 26;
                        let len = (hi as i128 - lo as i128).max(0);
                        if len > LIMIT as i128 {
                            return Err(RuntimeError::RangeTooLarge {
                                len,
                                limit: LIMIT,
                                span,
                            });
                        }
                        Ok(Value::List((lo..hi).map(Value::Int).collect()))
                    }
                    (a, b) => {
                        let offender = if matches!(a, Value::Int(_)) { b } else { a };
                        Err(type_err(Ty::int(), &offender, span))
                    }
                }
            }
            UnwrapOr => {
                let (opt, default) = arg2(args, span)?;
                match opt {
                    Value::Ctor(c, vs) if c == "Some" && vs.len() == 1 => Ok(vs[0].clone()),
                    Value::Ctor(c, _) if c == "None" => Ok(default),
                    v => Err(type_err(Ty::named("Option".into(), vec![]), &v, span)),
                }
            }
            IsSome => match arg1(args, span)? {
                Value::Ctor(c, _) if c == "Some" || c == "None" => Ok(Value::Bool(c == "Some")),
                v => Err(type_err(Ty::named("Option".into(), vec![]), &v, span)),
            },

            // The higher-order list functions. Each walks its list in a loop and
            // applies the function it was handed once per element, so the
            // evaluator's recursion depth does not grow with the list. The
            // reference definitions, in Bridger:
            //   fn map<A, B>(xs: [A], f: fn(A) -> B) -> [B] =
            //       match xs { [] => [], h :: t => f(h) :: map(t, f) };
            //   fn filter<A>(xs: [A], keep: fn(A) -> Bool) -> [A] =
            //       match xs {
            //           [] => [],
            //           h :: t => if keep(h) { h :: filter(t, keep) }
            //                     else { filter(t, keep) },
            //       };
            //   fn fold<A, B>(xs: [A], init: B, f: fn(B, A) -> B) -> B =
            //       match xs { [] => init, h :: t => fold(t, f(init, h), f) };
            Map => {
                let (xs, f) = arg2(args, span)?;
                let xs = list_arg(xs, span)?;
                let mut out = Vec::with_capacity(xs.len());
                for x in xs.iter() {
                    out.push(self.apply(f.clone(), vec![x.clone()], span)?);
                }
                Ok(Value::List(out.into()))
            }
            Filter => {
                let (xs, keep) = arg2(args, span)?;
                let xs = list_arg(xs, span)?;
                let mut out = Vec::new();
                for x in xs.iter() {
                    match self.apply(keep.clone(), vec![x.clone()], span)? {
                        Value::Bool(true) => out.push(x.clone()),
                        Value::Bool(false) => {}
                        v => return Err(type_err(Ty::bool(), &v, span)),
                    }
                }
                Ok(Value::List(out.into()))
            }
            Fold => {
                let (xs, init, f) = arg3(args, span)?;
                let xs = list_arg(xs, span)?;
                let mut acc = init;
                for x in xs.iter() {
                    acc = self.apply(f.clone(), vec![acc, x.clone()], span)?;
                }
                Ok(acc)
            }

            // Input: read one whitespace-delimited token or a whole line from
            // the program's input. The `_opt` readers return `None` on malformed
            // or absent input; the plain readers get stuck with an `InputError`
            // that says what was expected.
            ReadInt => {
                arg0(args, span)?;
                self.read_int_value()
                    .ok_or_else(|| input_error("an integer", self.peek_token(), span))
            }
            ReadBool => {
                arg0(args, span)?;
                self.read_bool_value()
                    .ok_or_else(|| input_error("a boolean", self.peek_token(), span))
            }
            ReadLine => {
                arg0(args, span)?;
                self.read_line_value()
                    .ok_or_else(|| input_error("a line", None, span))
            }
            ReadIntOpt => {
                arg0(args, span)?;
                Ok(option(self.read_int_value()))
            }
            ReadBoolOpt => {
                arg0(args, span)?;
                Ok(option(self.read_bool_value()))
            }
            ReadLineOpt => {
                arg0(args, span)?;
                Ok(option(self.read_line_value()))
            }
        }
    }

    /// Apply a function value to already-evaluated arguments from native code.
    /// The call is made by evaluating a synthetic `f(a0, …, an)` in a fresh
    /// environment that binds those names, so it goes through the evaluator's
    /// own `Call` arm — arity, `return`, the Bridger-versus-native dispatch —
    /// rather than a second copy of that logic. A `Return` cannot escape a call,
    /// so the only control left is a stuck evaluation.
    pub(crate) fn apply(
        &mut self,
        f: Value,
        args: Vec<Value>,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        // A relation value would resolve its name afresh in the synthetic
        // environment below; it is not a function to begin with.
        if matches!(f, Value::Relation(_)) {
            return Err(RuntimeError::NotAFunction {
                found: type_of(&f),
                span,
            });
        }
        let mut env = Env::new().extend("f".to_string(), f);
        let mut arg_exprs = Vec::with_capacity(args.len());
        for (i, v) in args.into_iter().enumerate() {
            let name = format!("a{i}");
            arg_exprs.push(Expr::Var(name.clone(), span));
            env = env.extend(name, v);
        }
        let call = Expr::Call(Box::new(Expr::Var("f".to_string(), span)), arg_exprs, span);
        match self.eval_expr(&call, &env) {
            Ok(v) | Err(Control::Return(v)) => Ok(v),
            Err(Control::Raise(e)) => Err(e),
        }
    }

    /// The next token parsed as an integer, or `None` (end of input, or the
    /// token is not an integer).
    fn read_int_value(&mut self) -> Option<Value> {
        // The token must be an integer literal as the lexer reads one. A
        // token that is not one is left unread, for the next reader.
        let before = self.input_pos;
        let v = self
            .read_token()
            .and_then(|t| crate::parser::lexer::parse_int_literal(&t))
            .map(Value::Int);
        if v.is_none() {
            self.input_pos = before;
        }
        v
    }

    /// The next token as `true` / `false`, or `None` — the token then left
    /// unread.
    fn read_bool_value(&mut self) -> Option<Value> {
        let before = self.input_pos;
        let v = match self.read_token().as_deref() {
            Some("true") => Some(Value::Bool(true)),
            Some("false") => Some(Value::Bool(false)),
            _ => None,
        };
        if v.is_none() {
            self.input_pos = before;
        }
        v
    }

    /// The rest of the current line (without its newline), or `None` at end of
    /// input — which distinguishes a genuine EOF from an empty line. When a
    /// token reader has left the cursor mid-line and only whitespace remains
    /// on that line, the remainder is skipped first, so `read_int()` followed
    /// by `read_line()` reads the next line rather than an empty tail. A line
    /// the cursor starts at is returned as it is, empty or not.
    fn read_line_value(&mut self) -> Option<Value> {
        let bytes = self.input.as_bytes();
        let mid_line = self.input_pos > 0 && bytes[self.input_pos - 1] != b'\n';
        if mid_line {
            let mut p = self.input_pos;
            while p < bytes.len() && bytes[p] != b'\n' && bytes[p].is_ascii_whitespace() {
                p += 1;
            }
            if p >= bytes.len() || bytes[p] == b'\n' {
                self.input_pos = (p + 1).min(bytes.len());
            }
        }
        if self.input_pos >= bytes.len() {
            return None;
        }
        let start = self.input_pos;
        while self.input_pos < bytes.len() && bytes[self.input_pos] != b'\n' {
            self.input_pos += 1;
        }
        // A line written on Windows ends in `\r\n`; the `\r` is not part of it.
        let line = self.input[start..self.input_pos]
            .strip_suffix('\r')
            .unwrap_or(&self.input[start..self.input_pos])
            .to_string();
        if self.input_pos < bytes.len() {
            self.input_pos += 1; // consume the newline
        }
        Some(Value::Str(Rc::from(line.as_str())))
    }

    /// The next token without consuming it, for a diagnostic.
    fn peek_token(&mut self) -> Option<String> {
        let before = self.input_pos;
        let t = self.read_token();
        self.input_pos = before;
        t
    }

    /// The next whitespace-delimited token of the input, advancing past it;
    /// `None` at end of input.
    fn read_token(&mut self) -> Option<String> {
        let bytes = self.input.as_bytes();
        while self.input_pos < bytes.len() && bytes[self.input_pos].is_ascii_whitespace() {
            self.input_pos += 1;
        }
        if self.input_pos >= bytes.len() {
            return None;
        }
        let start = self.input_pos;
        while self.input_pos < bytes.len() && !bytes[self.input_pos].is_ascii_whitespace() {
            self.input_pos += 1;
        }
        Some(self.input[start..self.input_pos].to_string())
    }

    /// Render `v` in the **canonical text format** (Appendix C, "Printing") — the format `print`,
    /// `println`, and `to_string` share, and the one autograders diff, so it is
    /// normative. `nested` is `false` at the top level and `true` inside a
    /// compound: a `String` prints raw at the top level and double-quoted when
    /// nested, mirroring Rust's `Display`/`Debug` split. A function or relation
    /// has no printable form and gets stuck.
    fn format_value(&self, v: &Value, nested: bool, span: Span) -> Result<String, RuntimeError> {
        let mut out = String::new();
        self.fmt_value(v, nested, span, &mut Vec::new(), &mut out)?;
        Ok(out)
    }

    /// `format_value` with the references being printed, so a cyclic
    /// structure prints `ref(…)` where it meets itself instead of recursing.
    fn fmt_value(
        &self,
        v: &Value,
        nested: bool,
        span: Span,
        visiting: &mut Vec<Loc>,
        out: &mut String,
    ) -> Result<(), RuntimeError> {
        use std::fmt::Write;
        match v {
            Value::Int(n) => write!(out, "{n}").unwrap(),
            Value::Bool(b) => write!(out, "{b}").unwrap(),
            Value::Unit => out.push_str("()"),
            Value::Str(s) => {
                if nested {
                    out.push_str(&quote_str(s));
                } else {
                    out.push_str(s);
                }
            }
            Value::Ref(loc) => {
                if visiting.contains(loc) {
                    out.push_str("ref(…)");
                    return Ok(());
                }
                visiting.push(*loc);
                let inner = self.store.read(*loc);
                out.push_str("ref(");
                self.fmt_value(&inner, true, span, visiting, out)?;
                out.push(')');
                visiting.pop();
            }
            Value::List(xs) => {
                out.push('[');
                self.format_each(xs.iter(), span, visiting, out)?;
                out.push(']');
            }
            Value::Tuple(xs) => {
                out.push('(');
                self.format_each(xs.iter(), span, visiting, out)?;
                out.push(')');
            }
            Value::Ctor(name, xs) if xs.is_empty() => out.push_str(name),
            Value::Ctor(name, xs) => {
                out.push_str(name);
                out.push('(');
                self.format_each(xs.iter(), span, visiting, out)?;
                out.push(')');
            }
            Value::Struct(name, fields) if fields.is_empty() => {
                out.push_str(name);
                out.push_str(" {}");
            }
            Value::Struct(name, fields) => {
                out.push_str(name);
                out.push_str(" { ");
                for (i, (f, v)) in fields.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    out.push_str(f);
                    out.push_str(": ");
                    self.fmt_value(v, true, span, visiting, out)?;
                }
                out.push_str(" }");
            }
            Value::Closure(_) | Value::Relation(_) | Value::Type(_) => {
                return Err(RuntimeError::NotPrintable {
                    found: type_of(v),
                    span,
                })
            }
        }
        Ok(())
    }

    /// The elements of a compound, comma-separated, each nested.
    fn format_each<'a>(
        &self,
        xs: impl Iterator<Item = &'a Value>,
        span: Span,
        visiting: &mut Vec<Loc>,
        out: &mut String,
    ) -> Result<(), RuntimeError> {
        for (i, x) in xs.enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            self.fmt_value(x, true, span, visiting, out)?;
        }
        Ok(())
    }
}

/// `print("a\tb")` nested → `"a\tb"`: double-quoted, with the escapes the lexer
/// accepts (Appendix B), so the round trip is unambiguous.
fn quote_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\0' => out.push_str("\\0"),
            c if c.is_control() => out.push_str(&format!("\\u{{{:x}}}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The total order behind `min`/`max`/`minimum`/`maximum`, defined on the
/// built-in `Ord` types (`Int`, `String`, `Bool`; Appendix C). Mismatched or unordered
/// operands return `None`, which the caller reports as `NotOrdered`.
fn value_cmp(a: &Value, b: &Value) -> Option<Ordering> {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => Some(x.cmp(y)),
        (Value::Str(x), Value::Str(y)) => Some(x.cmp(y)),
        (Value::Bool(x), Value::Bool(y)) => Some(x.cmp(y)),
        _ => None,
    }
}

/// `Some(v)` for a present value, `None` for an absent one — the built-in
/// `Option`, as the constructor values the evaluator uses.
fn option(v: Option<Value>) -> Value {
    match v {
        Some(v) => some(v),
        None => none(),
    }
}

fn some(v: Value) -> Value {
    Value::Ctor("Some".into(), Rc::from([v]))
}

fn none() -> Value {
    Value::Ctor("None".into(), Rc::from([]))
}

/// No arguments, or an [`RuntimeError::ArityMismatch`].
fn arg0(args: Vec<Value>, span: Span) -> Result<(), RuntimeError> {
    if args.is_empty() {
        Ok(())
    } else {
        Err(arity(0, args.len(), span))
    }
}

/// Exactly one argument, or an [`RuntimeError::ArityMismatch`].
fn arg1(args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
    let [a] = <[Value; 1]>::try_from(args).map_err(|v| arity(1, v.len(), span))?;
    Ok(a)
}

/// Exactly two arguments, or an [`RuntimeError::ArityMismatch`].
fn arg2(args: Vec<Value>, span: Span) -> Result<(Value, Value), RuntimeError> {
    let [a, b] = <[Value; 2]>::try_from(args).map_err(|v| arity(2, v.len(), span))?;
    Ok((a, b))
}

fn arg3(args: Vec<Value>, span: Span) -> Result<(Value, Value, Value), RuntimeError> {
    let [a, b, c] = <[Value; 3]>::try_from(args).map_err(|v| arity(3, v.len(), span))?;
    Ok((a, b, c))
}

/// The list a list-consuming primitive was handed, or a type error naming a
/// list (the element type is a convention; grading matches variant and span).
fn list_arg(v: Value, span: Span) -> Result<List, RuntimeError> {
    match v {
        Value::List(xs) => Ok(xs),
        v => Err(type_err(Ty::list(Ty::int()), &v, span)),
    }
}

fn arity(expected: usize, found: usize, span: Span) -> RuntimeError {
    RuntimeError::ArityMismatch {
        expected,
        found,
        span,
    }
}

fn input_error(expected: &str, found: Option<String>, span: Span) -> RuntimeError {
    RuntimeError::InputError {
        expected: expected.to_string(),
        found,
        span,
    }
}

fn type_err(expected: Ty, found: &Value, span: Span) -> RuntimeError {
    RuntimeError::TypeError {
        expected,
        found: type_of(found),
        span,
    }
}

fn not_ordered(a: &Value, b: &Value, span: Span) -> RuntimeError {
    // Blame the first operand that breaks the shared `Ord` type, left to right.
    let found = if matches!(
        (a, b),
        (Value::Int(_), _) | (Value::Str(_), _) | (Value::Bool(_), _)
    ) {
        type_of(b)
    } else {
        type_of(a)
    };
    RuntimeError::NotOrdered { found, span }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interp::Interpreter;

    const SP: Span = Span {
        src: crate::ast::SrcId::SYNTHETIC,
        start: 0,
        end: 0,
    };

    fn call(it: &mut Interpreter, n: Native, args: Vec<Value>) -> Result<Value, RuntimeError> {
        it.eval_prelude(n.id(), args, SP)
    }

    fn int(n: i64) -> Value {
        Value::Int(n)
    }
    fn list(vs: Vec<Value>) -> Value {
        Value::List(vs.into())
    }
    fn s(x: &str) -> Value {
        Value::Str(Rc::from(x))
    }

    // ---- the canonical text format (Appendix C, "Printing") — normative, diffed by autograders ----

    #[test]
    fn scalars_format_canonically() {
        let it = Interpreter::new();
        assert_eq!(it.format_value(&int(-42), false, SP).unwrap(), "-42");
        assert_eq!(
            it.format_value(&Value::Bool(true), false, SP).unwrap(),
            "true"
        );
        assert_eq!(it.format_value(&Value::Unit, false, SP).unwrap(), "()");
    }

    #[test]
    fn strings_are_raw_at_top_level_and_quoted_when_nested() {
        let it = Interpreter::new();
        assert_eq!(it.format_value(&s("hi"), false, SP).unwrap(), "hi");
        assert_eq!(it.format_value(&s("hi"), true, SP).unwrap(), "\"hi\"");
        // escapes appear only in the nested (quoted) form
        let esc = s("a\"b\n\t");
        assert_eq!(it.format_value(&esc, true, SP).unwrap(), "\"a\\\"b\\n\\t\"");
    }

    #[test]
    fn compounds_quote_their_string_elements() {
        let it = Interpreter::new();
        assert_eq!(
            it.format_value(&list(vec![int(1), int(2), int(3)]), false, SP)
                .unwrap(),
            "[1, 2, 3]"
        );
        assert_eq!(
            it.format_value(&list(vec![s("a"), s("b")]), false, SP)
                .unwrap(),
            "[\"a\", \"b\"]"
        );
        let tup = Value::Tuple(vec![int(1), Value::Bool(true)].into());
        assert_eq!(it.format_value(&tup, false, SP).unwrap(), "(1, true)");
    }

    #[test]
    fn constructors_and_structs_format_canonically() {
        let it = Interpreter::new();
        let none = Value::Ctor("None".into(), Rc::from([]));
        let some = Value::Ctor("Some".into(), Rc::from([int(3)]));
        assert_eq!(it.format_value(&none, false, SP).unwrap(), "None");
        assert_eq!(it.format_value(&some, false, SP).unwrap(), "Some(3)");
        let point = Value::Struct(
            "Point".into(),
            Rc::from([("x".to_string(), int(1)), ("y".to_string(), int(2))]),
        );
        assert_eq!(
            it.format_value(&point, false, SP).unwrap(),
            "Point { x: 1, y: 2 }"
        );
    }

    #[test]
    fn a_ref_shows_its_contents() {
        let mut it = Interpreter::new();
        let loc = it.store.alloc(int(0));
        assert_eq!(
            it.format_value(&Value::Ref(loc), false, SP).unwrap(),
            "ref(0)"
        );
    }

    #[test]
    fn a_function_is_not_printable() {
        let mut it = Interpreter::new();
        let f = Value::Closure(Rc::new(Closure::Native(Native::Abs.id())));
        assert!(matches!(
            call(&mut it, Native::ToString, vec![f]),
            Err(RuntimeError::NotPrintable { .. })
        ));
    }

    // ---- print / println write to the interpreter's output buffer ----

    #[test]
    fn print_and_println_accumulate_output() {
        let mut it = Interpreter::new();
        call(&mut it, Native::Print, vec![s("hi ")]).unwrap();
        call(&mut it, Native::Println, vec![int(7)]).unwrap();
        call(&mut it, Native::Print, vec![Value::Bool(false)]).unwrap();
        assert_eq!(it.output(), "hi 7\nfalse");
    }

    // ---- pure builtins ----

    #[test]
    fn arithmetic_builtins() {
        let mut it = Interpreter::new();
        assert_eq!(call(&mut it, Native::Abs, vec![int(-5)]).unwrap(), int(5));
        assert_eq!(
            call(&mut it, Native::Even, vec![int(4)]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            call(&mut it, Native::Odd, vec![int(4)]).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn len_over_strings_and_lists() {
        let mut it = Interpreter::new();
        assert_eq!(
            call(&mut it, Native::Len, vec![s("héllo")]).unwrap(),
            int(5)
        );
        assert_eq!(
            call(&mut it, Native::Len, vec![list(vec![int(1), int(2)])]).unwrap(),
            int(2)
        );
    }

    #[test]
    fn min_max_and_their_list_forms() {
        let mut it = Interpreter::new();
        assert_eq!(
            call(&mut it, Native::Min, vec![int(3), int(7)]).unwrap(),
            int(3)
        );
        assert_eq!(
            call(&mut it, Native::Max, vec![s("a"), s("b")]).unwrap(),
            s("b")
        );
        assert_eq!(
            call(
                &mut it,
                Native::Minimum,
                vec![list(vec![int(5), int(2), int(9)])]
            )
            .unwrap(),
            Value::Ctor("Some".into(), Rc::from([int(2)]))
        );
        // an empty list has no minimum
        assert_eq!(
            call(&mut it, Native::Maximum, vec![list(vec![])]).unwrap(),
            Value::Ctor("None".into(), Rc::from([]))
        );
    }

    #[test]
    fn head_tail_contains_range() {
        let mut it = Interpreter::new();
        let xs = list(vec![int(1), int(2), int(3)]);
        assert_eq!(
            call(&mut it, Native::Head, vec![xs.clone()]).unwrap(),
            Value::Ctor("Some".into(), Rc::from([int(1)]))
        );
        assert_eq!(
            call(&mut it, Native::Tail, vec![xs.clone()]).unwrap(),
            Value::Ctor("Some".into(), Rc::from([list(vec![int(2), int(3)])]))
        );
        assert_eq!(
            call(&mut it, Native::Head, vec![list(vec![])]).unwrap(),
            Value::Ctor("None".into(), Rc::from([]))
        );
        assert_eq!(
            call(&mut it, Native::Contains, vec![xs, int(2)]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            call(&mut it, Native::Range, vec![int(1), int(4)]).unwrap(),
            list(vec![int(1), int(2), int(3)])
        );
    }

    #[test]
    fn option_helpers() {
        let mut it = Interpreter::new();
        let some = Value::Ctor("Some".into(), Rc::from([int(9)]));
        let none = Value::Ctor("None".into(), Rc::from([]));
        assert_eq!(
            call(&mut it, Native::UnwrapOr, vec![some.clone(), int(0)]).unwrap(),
            int(9)
        );
        assert_eq!(
            call(&mut it, Native::UnwrapOr, vec![none.clone(), int(0)]).unwrap(),
            int(0)
        );
        assert_eq!(
            call(&mut it, Native::IsSome, vec![some]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            call(&mut it, Native::IsSome, vec![none]).unwrap(),
            Value::Bool(false)
        );
    }

    // ---- stuck cases carry the call span and the right variant ----

    #[test]
    fn wrong_arity_and_type_get_stuck() {
        let mut it = Interpreter::new();
        assert!(matches!(
            call(&mut it, Native::Abs, vec![]),
            Err(RuntimeError::ArityMismatch {
                expected: 1,
                found: 0,
                ..
            })
        ));
        assert!(matches!(
            call(&mut it, Native::Abs, vec![Value::Bool(true)]),
            Err(RuntimeError::TypeError { .. })
        ));
        // `min` on values with no shared `Ord` type
        assert!(matches!(
            call(&mut it, Native::Min, vec![int(1), s("x")]),
            Err(RuntimeError::NotOrdered { .. })
        ));
    }

    #[test]
    fn new_loads_the_bridger_prelude() {
        let it = Interpreter::new();
        for name in ["Option", "Result", "Ordering", "Ord", "Len", "From"] {
            assert!(
                it.program().decls.contains_key(name),
                "prelude missing {name}"
            );
        }
        // the higher-order functions are native, bound by name rather than declared
        for name in ["map", "filter", "fold"] {
            assert!(native_names().any(|n| n == name), "native missing {name}");
            assert!(!it.program().decls.contains_key(name));
        }
    }
}
