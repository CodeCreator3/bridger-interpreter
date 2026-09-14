//! Milestone M5 — the type checker (public subset).  [frozen — do not edit]
//!
//! A parallel static pass: `Checker::check_one` types one hand-built `Expr`,
//! `Checker::check_program` a whole loaded program (the prelude is trusted and
//! skipped). Types compare by shape (`Ty: PartialEq` ignores spans), and — as
//! for the runtime errors — tests match a `TyError`'s variant, not its message.
//!
//! Gated on the `m5` feature: compiled at every milestone from M5 on.

#![cfg(feature = "m5")]

mod common;

use bridger::ast::BinOp::*;
use bridger::ast::{Expr, Ty, UnOp};
use bridger::interp::Interpreter;
use bridger::types::{Checker, Ctx, TyError};
use common::*;

fn infer(e: &Expr) -> Result<Ty, TyError> {
    Checker::check_one(e, &Ctx::new(None, None), None)
}
fn check(e: &Expr, expected: &Ty) -> Result<Ty, TyError> {
    Checker::check_one(e, &Ctx::new(None, None), Some(expected))
}
fn infer_in_fn(e: &Expr, ret: Ty) -> Result<Ty, TyError> {
    Checker::check_one(e, &Ctx::new(Some(ret), None), None)
}
/// Type-check a whole program (on top of the trusted prelude).
fn check_program(src: &str) -> Result<(), TyError> {
    let mut it = Interpreter::new();
    it.load_program(src).expect("program should parse");
    Checker::check_program(it.program()).map(drop)
}

// ---- operators ----

#[test]
fn literals_and_arithmetic_and_comparison() {
    assert_eq!(infer(&int(5, 0)).unwrap(), Ty::int());
    assert_eq!(infer(&boolean(true, 0)).unwrap(), Ty::bool());
    assert_eq!(
        infer(&binary(Add, int(1, 0), int(2, 1), 2)).unwrap(),
        Ty::int()
    );
    assert_eq!(
        infer(&binary(Lt, int(1, 0), int(2, 1), 2)).unwrap(),
        Ty::bool()
    );
    assert_eq!(
        infer(&binary(And, boolean(true, 0), boolean(false, 1), 2)).unwrap(),
        Ty::bool()
    );
}

#[test]
fn arithmetic_on_a_non_integer_is_a_mismatch() {
    let e = binary(Add, int(1, 0), boolean(true, 1), 2);
    assert!(matches!(infer(&e), Err(TyError::Mismatch { .. })));
}

#[test]
fn concat_and_cons() {
    assert_eq!(
        infer(&binary(Concat, string("a", 0), string("b", 1), 2)).unwrap(),
        Ty::str()
    );
    let lists = binary(
        Concat,
        list(vec![int(1, 0)], 1),
        list(vec![int(2, 2)], 3),
        4,
    );
    assert_eq!(infer(&lists).unwrap(), Ty::list(Ty::int()));
    let cons = binary(Cons, int(1, 0), list(vec![int(2, 1)], 2), 3);
    assert_eq!(infer(&cons).unwrap(), Ty::list(Ty::int()));
}

#[test]
fn equality_requires_a_type_that_admits_it() {
    assert_eq!(
        infer(&binary(Eq, int(1, 0), int(2, 1), 2)).unwrap(),
        Ty::bool()
    );
    // comparing two functions has no equality
    let f = lambda(&["x"], var("x", 1), 0);
    let g = lambda(&["y"], var("y", 3), 2);
    assert!(matches!(
        infer(&binary(Eq, f, g, 4)),
        Err(TyError::NoEquality { .. })
    ));
}

// ---- compound data ----

#[test]
fn tuples_lists_and_projection() {
    let t = tuple(vec![int(1, 0), boolean(true, 1)], 2);
    assert_eq!(infer(&t).unwrap(), Ty::tuple(vec![Ty::int(), Ty::bool()]));
    assert_eq!(
        infer(&proj(tuple(vec![int(1, 0), boolean(true, 1)], 2), 1, 3)).unwrap(),
        Ty::bool()
    );
    assert_eq!(
        infer(&list(vec![int(1, 0), int(2, 1)], 2)).unwrap(),
        Ty::list(Ty::int())
    );
}

#[test]
fn a_list_literals_elements_must_agree() {
    let e = list(vec![int(1, 0), boolean(true, 1)], 2);
    assert!(matches!(infer(&e), Err(TyError::Mismatch { .. })));
}

#[test]
fn an_empty_list_takes_its_element_type_from_context() {
    let empty = list(vec![], 0);
    assert_eq!(
        check(&empty, &Ty::list(Ty::int())).unwrap(),
        Ty::list(Ty::int())
    );
}

// ---- binding, control flow ----

#[test]
fn a_block_threads_let_bindings() {
    // { let x = 1; let y = x + 1; y }  :  Int
    let b = block(
        vec![
            let_("x", int(1, 1), 0),
            let_("y", binary(Add, var("x", 2), int(1, 3), 4), 5),
        ],
        Some(var("y", 6)),
        7,
    );
    assert_eq!(infer(&b).unwrap(), Ty::int());
}

#[test]
fn if_branches_must_have_one_type() {
    let ok = if_else(boolean(true, 0), int(1, 1), int(2, 2), 3);
    assert_eq!(infer(&ok).unwrap(), Ty::int());
    let bad = if_else(boolean(true, 0), int(1, 1), boolean(false, 2), 3);
    assert!(matches!(infer(&bad), Err(TyError::Mismatch { .. })));
}

#[test]
fn loops_and_assignment_are_unit() {
    assert_eq!(
        infer(&while_(boolean(false, 0), unit(1), 2)).unwrap(),
        Ty::unit()
    );
    // { let r = ref 0; r := 5 }  :  ()
    let b = block(
        vec![let_("r", unary(UnOp::Ref, int(0, 2), 1), 0)],
        Some(assign(var("r", 3), int(5, 4), 5)),
        6,
    );
    assert_eq!(infer(&b).unwrap(), Ty::unit());
}

#[test]
fn return_matches_the_enclosing_return_type() {
    // inside a fn returning Int, `return 1` is fine and slots in anywhere
    assert!(infer_in_fn(&return_(int(1, 1), 0), Ty::int()).is_ok());
    // its operand must match the return type
    assert!(matches!(
        infer_in_fn(&return_(boolean(true, 1), 0), Ty::int()),
        Err(TyError::Mismatch { .. })
    ));
    // at the top level there is no return type
    assert!(matches!(
        infer(&return_(int(1, 1), 0)),
        Err(TyError::ReturnOutsideFunction { .. })
    ));
}

// ---- functions ----

#[test]
fn a_lambda_infers_a_function_type() {
    // |x| x + 1  :  fn(Int) -> Int
    let f = lambda(&["x"], binary(Add, var("x", 1), int(1, 2), 3), 0);
    assert_eq!(infer(&f).unwrap(), Ty::func(vec![Ty::int()], Ty::int()));
}

#[test]
fn a_lambda_checks_against_an_expected_function_type() {
    // |x| x  against  fn(Int) -> Int
    let f = lambda(&["x"], var("x", 1), 0);
    let want = Ty::func(vec![Ty::int()], Ty::int());
    assert_eq!(check(&f, &want).unwrap(), want);
}

#[test]
fn application_checks_arguments_and_yields_the_result() {
    // (|x| x + 1)(4)  :  Int
    let f = lambda(&["x"], binary(Add, var("x", 1), int(1, 2), 3), 0);
    assert_eq!(infer(&call(f, vec![int(4, 4)], 5)).unwrap(), Ty::int());
    // wrong arity
    let g = lambda(&["x", "y"], int(0, 1), 0);
    assert!(matches!(
        infer(&call(g, vec![int(1, 2)], 3)),
        Err(TyError::ArityMismatch { .. })
    ));
    // calling a non-function
    assert!(matches!(
        infer(&call(int(5, 1), vec![], 0)),
        Err(TyError::NotAFunction { .. })
    ));
}

// ---- whole programs (prelude trusted) ----

#[test]
fn a_well_typed_program_checks() {
    assert!(
        check_program("fn double(x: Int) -> Int = x * 2; fn main() -> Int = double(21);").is_ok()
    );
}

#[test]
fn a_recursive_program_checks() {
    let src = "fn fact(n: Int) -> Int = if n == 0 { 1 } else { n * fact(n - 1) };\
               fn main() -> Int = fact(5);";
    assert!(check_program(src).is_ok());
}

#[test]
fn a_generic_prelude_call_is_instantiated() {
    // map : fn([A], fn(A) -> B) -> [B]; here A = B = Int
    let src = "fn main() -> [Int] = map([1, 2, 3], |x| x + 1);";
    assert!(check_program(src).is_ok());
}

#[test]
fn native_prelude_functions_are_typed() {
    // `print` / `to_string` are parametric
    assert!(check_program("fn main() -> () = print(42);").is_ok());
    assert!(check_program("fn main() -> String = to_string([1, 2, 3]);").is_ok());
    // `min` requires `Ord`, which `Int` satisfies; `len` requires `Len`, which a
    // list satisfies
    assert!(check_program("fn main() -> Int = min(3, 7);").is_ok());
    assert!(check_program("fn main() -> Int = len([1, 2, 3]);").is_ok());
    // an unknown name is still unbound
    assert!(matches!(
        check_program("fn main() -> Int = frobnicate(1);"),
        Err(TyError::UnboundVariable { .. })
    ));
}

#[test]
fn a_bound_native_call_on_a_non_conforming_type_is_rejected() {
    // tuples are not `Ord`, so `min` on them has no satisfying bound
    assert!(matches!(
        check_program("fn main() -> (Int, Int) = min((1, 2), (3, 4));"),
        Err(TyError::UnsatisfiedBound { .. })
    ));
}

#[test]
fn the_checker_rejects_a_program_the_evaluator_would_get_stuck_on() {
    assert!(matches!(
        check_program("fn main() -> Int = 1 + true;").map_err(|_| ()),
        Err(())
    ));
    // branches that disagree
    assert!(matches!(
        check_program("fn main() -> Int = if true { 1 } else { false };"),
        Err(TyError::Mismatch { .. })
    ));
}

#[test]
fn a_cycle_among_globals_is_a_static_error() {
    let err = check_program("let a = b + 1; let b = a + 1; fn main() -> Int = a;");
    assert!(matches!(err, Err(TyError::InitializationCycle { .. })));
}

#[test]
fn an_annotation_must_name_a_declared_type() {
    let unknown = check_program("fn main() -> Int { let x: Foo = 1; x }");
    assert!(matches!(unknown, Err(TyError::UnknownType { .. })));
    let in_sig = check_program("fn f(x: Nope) -> Int = 1; fn main() -> Int = 1;");
    assert!(matches!(in_sig, Err(TyError::UnknownType { .. })));
}

#[test]
fn a_type_parameter_is_a_type_only_in_its_declaration() {
    assert!(check_program("fn id<T>(x: T) -> T = x; fn main() -> Int = id(1);").is_ok());
    let escaped =
        check_program("fn id<T>(x: T) -> T = x; fn f(x: T) -> T = x; fn main() -> Int = 1;");
    assert!(matches!(escaped, Err(TyError::UnknownType { .. })));
}

#[test]
fn an_unannotated_global_is_inferred_once() {
    // Each global reads the previous one twice. Re-inferring an initializer
    // at every reference would take 2^n steps; recording each global's type
    // in initialization order takes n.
    let mut src = String::from("let g0 = 1;");
    for i in 1..=40 {
        src.push_str(&format!(" let g{i} = g{} + g{};", i - 1, i - 1));
    }
    src.push_str(" fn main() -> Int = g40;");
    let mut it = Interpreter::new();
    it.load_program(&src).expect("parse");
    assert!(Checker::check_program(it.program()).is_ok());
}

#[test]
fn a_global_has_one_type() {
    // `xs` has one type, fixed by its initializer and annotation alone: one
    // they leave open is an error, and a use at another type is a mismatch
    assert!(matches!(
        check_program("let xs = []; fn f() -> [Int] = xs; fn main() -> () = ();"),
        Err(TyError::Ambiguous { .. })
    ));
    let src =
        "let xs: [Int] = []; fn f() -> [Int] = xs; fn g() -> [Bool] = xs; fn main() -> () = ();";
    assert!(matches!(check_program(src), Err(TyError::Mismatch { .. })));
}

#[test]
fn the_expected_type_reaches_tuples_and_lists() {
    fn check(src: &str) -> Result<(), TyError> {
        let mut it = Interpreter::new();
        it.load_program(src).expect("parse");
        Checker::check_program(it.program()).map(|_| ())
    }
    // a lambda in each position is typed from the annotation, so its
    // parameter needs no annotation of its own (match arms and constructor
    // arguments are the M7 half of this property, in `m7.rs`)
    for body in ["(|p| p + 1, 1).0", "[|p| p + 1][0]"] {
        let src = format!("fn main() -> Int {{ let f: fn(Int) -> Int = {body}; f(1) }}");
        // `[..][0]` is not Bridger syntax; use `head` for the list case
        let src = src.replace("[|p| p + 1][0]", "unwrap_or(head([|p| p + 1]), |p| p)");
        assert!(check(&src).is_ok(), "{body}");
    }
}

#[test]
fn printing_and_equality_need_data_all_the_way_down() {
    fn check(src: &str) -> Result<(), TyError> {
        let mut it = Interpreter::new();
        it.load_program(src).expect("parse");
        Checker::check_program(it.program()).map(|_| ())
    }
    // a function cannot be printed …
    assert!(matches!(
        check("fn main() -> () = println(abs);"),
        Err(TyError::NotPrintable { .. })
    ));
    assert!(matches!(
        check("fn main() -> String = to_string((1, abs));"),
        Err(TyError::NotPrintable { .. })
    ));
    // … nor searched for with `contains`
    assert!(matches!(
        check("fn main() -> Bool { let f = |x: Int| x; contains([f], f) }"),
        Err(TyError::NoEquality { .. })
    ));
    // `==` at a type the context fixes later is still caught
    assert!(matches!(
        check(
            "fn main() -> Bool { let xs = []; let b = xs == xs; let ys: [fn(Int) -> Int] = xs; b }"
        ),
        Err(TyError::NoEquality { .. })
    ));
    // a bare type parameter admits neither without a bound
    assert!(matches!(
        check("fn same<T>(a: T, b: T) -> Bool = a == b; fn main() -> () = ();"),
        Err(TyError::NoEquality { .. })
    ));
    assert!(matches!(
        check("fn show<T>(x: T) -> () = println(x); fn main() -> () = ();"),
        Err(TyError::NotPrintable { .. })
    ));
    // a reference prints what it holds, so a cell of functions does not print
    assert!(matches!(
        check("fn main() -> () = println(ref abs);"),
        Err(TyError::NotPrintable { .. })
    ));
    // while data does, references included
    assert!(check("fn main() -> () = println((ref 1, [2], \"s\"));").is_ok());
}

// ---- declarations: names and deferred bounds ----

#[test]
fn a_type_parameter_may_not_take_the_name_of_a_type() {
    assert!(matches!(
        check_program("type T = A | B; fn g<T>(x: T) -> Bool = x == x; fn main() -> () = ();"),
        Err(TyError::TypeParamShadows { .. })
    ));
    assert!(matches!(
        check_program("fn h<Int>(x: Int) -> Int = x; fn main() -> () = ();"),
        Err(TyError::TypeParamShadows { .. })
    ));
}

#[test]
fn every_type_is_determined_within_its_declaration() {
    // a type nothing in the declaration pins down is an error at the earliest
    // such expression — a global, a local, a lambda parameter, a bare `[]`
    for src in [
        "let m = |x| min(x, x); fn main() -> Int = m(1);",
        "fn main() -> Int { let x = []; 1 }",
        "fn main() -> Int { let f = |y| y; 1 }",
        "fn main() -> () = println([]);",
    ] {
        assert!(
            matches!(check_program(src), Err(TyError::Ambiguous { .. })),
            "{src}"
        );
    }
    // pinned by an annotation, or by a use within the same declaration
    for src in [
        "let m = |x: Int| min(x, x); fn main() -> Int = m(1);",
        "fn main() -> Int { let x = []; len(x ++ [1]) }",
        "fn main() -> () { let xs: [Int] = []; println(xs) }",
    ] {
        assert!(check_program(src).is_ok(), "{src}");
    }
    // and a bound is judged on the determined type
    assert!(matches!(
        check_program("let m = |x: [Int]| min(x, x); fn main() -> [Int] = m([1]);"),
        Err(TyError::UnsatisfiedBound { .. })
    ));
}

#[test]
fn no_type_crosses_a_declaration_except_through_an_annotation() {
    // an unannotated global whose type its initializer leaves open would let
    // one declaration's type parameter leak into another's
    let leak = "let g = ref [];\
        fn f<T>(x: T) -> () = g := [x];\
        fn h<T>(d: T) -> T = unwrap_or(head(deref g), d);\
        fn main() -> String { f(1); h(\"a\") ++ \"b\" }";
    assert!(matches!(
        check_program(leak),
        Err(TyError::Ambiguous { .. })
    ));
    let cell = "let g = ref [];\
        fn f() -> () = println(deref g);\
        fn h(x: fn(Int) -> Int) -> () = g := [x];\
        fn main() -> () { h(|x| x); f() }";
    assert!(matches!(
        check_program(cell),
        Err(TyError::Ambiguous { .. })
    ));
    // with the annotation, the program is judged on the one type it has
    let typed = "let g: ref<[fn(Int) -> Int]> = ref [];\
        fn f() -> () = println(deref g);\
        fn main() -> () = f();";
    assert!(matches!(
        check_program(typed),
        Err(TyError::NotPrintable { .. })
    ));
}

#[test]
fn a_form_whose_every_path_returns_is_diverging() {
    for src in [
        "fn f(c: Bool) -> Int { if c { return 1 } else { return 2 }; 5 }\
         fn main() -> Int = f(true);",
        "fn f(c: Bool) -> Int { let x = if c { return 1 } else { return 2 }; 5 }\
         fn main() -> Int = f(true);",
    ] {
        assert!(check_program(src).is_ok(), "{src}");
    }
}

#[test]
fn errors_are_reported_in_source_order() {
    // `zz` is wrong first in the file, `aa` second: the report is `zz`'s
    let src = "fn zz() -> Int = \"a\"; fn aa() -> Int = true; fn main() -> () = ();";
    let mut it = Interpreter::new();
    it.load_program(src).expect("parse");
    match Checker::check_program(it.program()) {
        Err(TyError::Mismatch { found, .. }) => assert_eq!(found.to_string(), "String"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_deeply_nested_literal_checks_in_linear_space() {
    // types share structure: the 3 000 recorded list types are one spine
    let depth = 3000;
    let src = format!(
        "fn main() -> Int = len({}1{});",
        "[".repeat(depth),
        "]".repeat(depth)
    );
    assert!(check_program(&src).is_ok());
}

// ---- diagnostics: the mistake, not its symptom ----

#[test]
fn an_omitted_result_type_and_a_missing_else_are_named() {
    // Plumbing: the help text is the reference's, not a student's; the payload
    // it reads is the graded part.
    assert!(matches!(
        check_program("fn sq(x: Int) = x * x; fn main() -> Int = sq(3);"),
        Err(TyError::MissingResultType { .. })
    ));
    // a written `-> ()` is a plain mismatch
    assert!(matches!(
        check_program("fn sq(x: Int) -> () = x * x; fn main() -> () = sq(3);"),
        Err(TyError::Mismatch { .. })
    ));
    assert!(matches!(
        check_program("fn f(n: Int) -> Int { if n > 0 { 1 } } fn main() -> Int = f(1);"),
        Err(TyError::IfWithoutElse { .. })
    ));
    // every error knows where it is, and some know what to suggest
    let err = check_program("fn sq(x: Int) = x * x; fn main() -> Int = sq(3);").unwrap_err();
    assert!(err.span().start > 0);
    assert!(err.help().is_some_and(|h| h.contains("-> Int")));
}

#[test]
fn a_block_that_ends_by_returning_is_diverging() {
    let src = "fn f(x: Int) -> Int = { if x > 0 { return 1; }; return 2; };\
        fn main() -> Int = f(0);";
    assert!(check_program(src).is_ok());
}

#[test]
fn errors_name_the_callee_and_blame_the_argument() {
    // arity: the callee is named
    match check_program("fn add(a: Int, b: Int) -> Int = a + b; fn main() -> Int = add(1);") {
        Err(TyError::ArityMismatch { callee, .. }) => assert_eq!(callee, "add"),
        other => panic!("{other:?}"),
    }
    // a bound failed by an argument is reported at the argument, not the callee
    let src = "fn main() -> () = println(|x: Int| x);";
    match check_program(src) {
        Err(TyError::NotPrintable { span, .. }) => {
            assert_eq!(&src[span.start..span.end], "|x: Int| x");
        }
        other => panic!("{other:?}"),
    }
    // a non-function callee is blamed itself
    let src = "fn main() -> Int { let n = 5; n(1) }";
    match check_program(src) {
        Err(TyError::NotAFunction { span, .. }) => assert_eq!(&src[span.start..span.end], "n"),
        other => panic!("{other:?}"),
    }
    // a type applied to the wrong number of arguments is a type-arity error
    assert!(matches!(
        check_program("fn f(x: Option<Int, Int>) -> Int = 1; fn main() -> () = ();"),
        Err(TyError::TypeArity { .. })
    ));
}

// ---- seventh review round ----

#[test]
fn a_bound_is_blamed_on_the_argument_of_the_bounded_parameter() {
    // `T: Ord` is on the second parameter: the second argument is underlined,
    // even when the first has the same type
    let src = "fn inc(n: Int) -> Int = n + 1; fn pick<T: Ord>(n: Int, x: T) -> T = x;\
        fn main() -> () { let g = pick(1, inc); }";
    let err = check_program(src).unwrap_err();
    assert!(matches!(err, TyError::UnsatisfiedBound { .. }), "{err:?}");
    assert_eq!(err.span().start, src.rfind("inc").unwrap());
    // a nested call's obligation stays on the nested call's argument
    let src = "fn inc(n: Int) -> Int = n + 1; fn same<T: Ord>(x: T) -> T = x;\
        fn id(x: fn(Int) -> Int) -> fn(Int) -> Int = x;\
        fn main() -> () { let g = id(same(inc)); }";
    let err = check_program(src).unwrap_err();
    assert_eq!(err.span().start, src.rfind("inc").unwrap());
}

#[test]
fn an_omitted_result_type_is_blamed_only_for_the_body_value() {
    // a loop body or an assignment that is not `()` is its own mismatch,
    // which no `-> T` would mend
    for src in [
        "fn main() { while true { 1 } }",
        "fn main() { let r = ref (); r := 1 }",
    ] {
        assert!(
            matches!(check_program(src), Err(TyError::Mismatch { .. })),
            "{src}"
        );
    }
    // the branches of an `if` standing as the body produce the body's value
    assert!(matches!(
        check_program("fn f(c: Bool) { if c { 1 } else { 2 } } fn main() -> () = f(true);"),
        Err(TyError::MissingResultType { .. })
    ));
}

#[test]
fn an_else_less_if_whose_branch_leaves_is_unit() {
    // the branch has no value, so the `if` is `()`; that is the mismatch,
    // not a branch "of type Int"
    let src = "fn f(c: Bool) -> Int { if c { return 1; } } fn main() -> Int = f(true);";
    assert!(matches!(
        check_program(src),
        Err(TyError::Mismatch { expected, found, .. }) if expected == Ty::int() && found == Ty::unit()
    ));
}

#[test]
fn dead_code_after_a_leaving_block_needs_no_annotation() {
    for src in [
        "fn f() -> Int { let x = { return 1; }; println(x); 2 } fn main() -> Int = f();",
        "fn f() -> Int { let q = { return 5; }; q.0 } fn main() -> Int = f();",
        "fn f() -> Int { let q = { return 5; }; q == q; 1 } fn main() -> Int = f();",
        "fn f() -> Int { return 1; let y = []; println(y); 2 } fn main() -> Int = f();",
        "fn f() -> Int { for x in { return 1; } { println(x) }; 2 } fn main() -> Int = f();",
        "fn f() -> Int { if { return 1; } { println([]) } else { () }; 2 } fn main() -> Int = f();",
    ] {
        assert!(check_program(src).is_ok(), "{src}");
    }
    // but a live value a leaving form's type reached is judged as usual: the
    // lambda is a value, and its parameter is undetermined
    for src in [
        "fn main() -> () { let f = |x| { return x; }; }",
        "fn main() -> () { let f = |x, y| { return x; }; }",
        "let g = |x| { return x; }; fn main() -> () { println(g(1)); println(g(\"a\")) }",
        "fn main() -> () { let f = |x| { return x; }; println(min(f, f)) }",
    ] {
        assert!(
            matches!(check_program(src), Err(TyError::Ambiguous { .. })),
            "{src}"
        );
    }
    // `return e;` as the whole body of a function with no `-> T`
    assert!(matches!(
        check_program("fn f() { return 1; } fn main() -> () = f();"),
        Err(TyError::MissingResultType { .. })
    ));
}

#[test]
fn a_projection_out_of_range_or_off_a_non_tuple_is_named() {
    assert!(matches!(
        check_program("fn main() -> Int = (1, 2).2;"),
        Err(TyError::NoSuchComponent {
            index: 2,
            arity: 2,
            ..
        })
    ));
    assert!(matches!(
        check_program("fn main() -> Int = 1.0;"),
        Err(TyError::NotATuple { .. })
    ));
}

#[test]
fn a_bound_in_scope_meets_a_prelude_bound_at_every_milestone() {
    // `T: Ord` on the declaration satisfies `min`'s bound, whether or not
    // user impls count yet
    assert!(check_program(
        "fn smallest<T: Ord>(a: T, b: T) -> T = min(a, b); fn main() -> Int = smallest(2, 1);"
    )
    .is_ok());
}

#[test]
fn an_undetermined_argument_is_blamed_not_the_callee() {
    for (src, arg) in [
        ("fn main() -> () = println([]);", "[]"),
        ("fn f<T>(x: T) -> () = (); fn main() -> () = f([]);", "[]"),
    ] {
        let err = check_program(src).unwrap_err();
        assert!(matches!(err, TyError::Ambiguous { .. }), "{src}");
        assert_eq!(&src[err.span().start..err.span().end], arg, "{src}");
    }
}

#[test]
fn nesting_past_the_checkers_bound_is_a_clean_error() {
    // `not`s past the bound around a literal: a clean error, never an
    // exhausted stack
    let mut e = Expr::Lit(bridger::ast::Lit::Bool(true), sp(0));
    for i in 1..=bridger::types::MAX_NESTING {
        e = Expr::Unary(UnOp::Not, Box::new(e), sp(i));
    }
    assert!(matches!(
        Checker::check_one(&e, &Ctx::new(None, None), None),
        Err(TyError::TooDeep { .. })
    ));
}

#[test]
fn main_takes_no_parameters_statically() {
    for src in ["fn main(x: Int) -> Int = x;", "fn main<T>() -> () = ();"] {
        assert!(
            matches!(check_program(src), Err(TyError::MainSignature { .. })),
            "{src}"
        );
    }
    // a fragment without `main` still checks
    assert!(check_program("fn f() -> Int = 1;").is_ok());
}

#[test]
fn a_bound_no_argument_mentions_is_blamed_on_the_call() {
    let src = "fn f<T: Ord>(x: Int) -> Int = x; fn main() -> () = println(f(1));";
    let err = check_program(src).unwrap_err();
    assert!(matches!(err, TyError::Ambiguous { .. }), "{err:?}");
    assert_eq!(&src[err.span().start..err.span().end], "f(1)");
}

#[test]
fn a_bound_missing_from_a_type_parameter_is_hinted() {
    // Plumbing: the help text is the reference's, not a student's; the payload
    // it reads is the graded part.
    let src = "fn big<T>(a: T, b: T) -> T = max(a, b); fn main() -> Int = big(1, 2);";
    let err = check_program(src).unwrap_err();
    assert!(
        matches!(err, TyError::UnsatisfiedBound { is_param: true, .. }),
        "{err:?}"
    );
    assert!(err.help().is_some_and(|h| h.contains("`T: Ord`")));
}

// ---- eleventh review round: typing sentences with no direct test ----

#[test]
fn string_unit_negation_and_or_are_typed_directly() {
    assert!(check_program("fn main() -> String = \"a\";").is_ok());
    assert!(check_program("fn main() -> () = ();").is_ok());
    assert!(matches!(
        check_program("fn main() -> Int = -true;"),
        Err(TyError::Mismatch { .. })
    ));
    assert!(matches!(
        check_program("fn main() -> Bool = 1 or true;"),
        Err(TyError::Mismatch { .. })
    ));
}

#[test]
fn a_concatenation_of_two_leaving_operands_is_undetermined() {
    let src =
        "fn f() -> String = if (return \"a\") ++ (return \"b\") != \"c\" { \"d\" } else { \"e\" };\
        fn main() -> () { let s = f(); }";
    assert!(matches!(check_program(src), Err(TyError::Ambiguous { .. })));
}

#[test]
fn a_for_loop_needs_a_list_and_a_unit_body() {
    assert!(matches!(
        check_program("fn main() -> () { for x in 5 { () } }"),
        Err(TyError::Mismatch { .. })
    ));
    assert!(matches!(
        check_program("fn main() -> () { for x in [1] { x } }"),
        Err(TyError::Mismatch { .. })
    ));
}

#[test]
fn a_let_bound_lambda_is_not_generalized() {
    let src = "fn main() -> Int { let id = |x| x; let a = id(1); let b = id(\"s\"); a }";
    assert!(matches!(check_program(src), Err(TyError::Mismatch { .. })));
}

#[test]
fn a_bad_left_operand_of_concatenation_is_blamed_at_that_operand() {
    let src = "fn main() -> Int = true ++ 2;";
    let err = check_program(src).unwrap_err();
    let TyError::Mismatch {
        expected,
        found,
        span,
    } = err
    else {
        panic!("{err:?}");
    };
    assert_eq!(expected.to_string(), "String");
    assert_eq!(found.to_string(), "Bool");
    assert_eq!(&src[span.start..span.end], "true");
}

// ---- fifteenth review round: unify and operator contracts the suite left open ----

#[test]
fn self_application_is_an_infinite_type() {
    // `|x| x(x)` unifies x with a function taking x; the occurs check must
    // reject it rather than build a cyclic type (which later loops forever).
    assert!(matches!(
        check_program("fn main() -> () { let f = |x| x(x); }"),
        Err(TyError::InfiniteType { .. })
    ));
}

#[test]
fn a_tuple_of_the_wrong_width_does_not_unify() {
    // unify must compare tuple arity, not zip-and-truncate.
    let src = "fn pair() -> (Int, Int, Int) = (1, 2); fn main() -> () { let t = pair(); }";
    assert!(matches!(check_program(src), Err(TyError::Mismatch { .. })));
}

#[test]
fn a_lambda_of_the_wrong_arity_does_not_unify_with_a_function_type() {
    // unify must compare function arity too.
    let src = "fn apply1(f: fn(Int) -> Int) -> Int = f(1);\
        fn main() -> () { let z = apply1(|x, y| x + y); }";
    assert!(matches!(check_program(src), Err(TyError::Mismatch { .. })));
}

#[test]
fn ordering_compares_only_integers() {
    // two operands of the same non-`Int` type must not slip past `<`.
    assert!(matches!(
        check_program("fn main() -> () { let b = \"a\" < \"b\"; }"),
        Err(TyError::Mismatch { .. })
    ));
}

#[test]
fn concatenation_admits_only_strings_and_lists() {
    // both operands `Int` is neither the string nor the list rule.
    assert!(matches!(
        check_program("fn main() -> () { let n = 1 ++ 2; }"),
        Err(TyError::Mismatch { .. })
    ));
}
