//! Milestone M4 — functions (public subset).  [frozen — do not edit]
//!
//! Lambdas, closures, and application, built by hand; and the first
//! **whole-program** tests, which load a `.brg` source on top of the prelude and
//! run `main` end to end through `eval_program` — the payoff of the driver.
//!
//! Gated on the `m4` feature: compiled at every milestone from M4 on.

#![cfg(feature = "m4")]

mod common;

use bridger::ast::BinOp::*;
use bridger::interp::value::Value;
use bridger::interp::RuntimeError;
use common::*;

// ---- E-Lam / E-App, by hand ----

#[test]
fn a_lambda_applies_to_its_argument() {
    // (|x| x + 1)(4)  ==>  5
    let f = lambda(&["x"], binary(Add, var("x", 1), int(1, 2), 3), 0);
    assert_eq!(value_of(&call(f, vec![int(4, 4)], 5)), Value::Int(5));
}

#[test]
fn a_closure_captures_its_definition_environment() {
    // { let y = 10; let f = |x| x + y; f(5) }  ==>  15
    let b = block(
        vec![
            let_("y", int(10, 1), 0),
            let_(
                "f",
                lambda(&["x"], binary(Add, var("x", 3), var("y", 4), 5), 2),
                6,
            ),
        ],
        Some(call(var("f", 7), vec![int(5, 8)], 9)),
        10,
    );
    assert_eq!(value_of(&b), Value::Int(15));
}

#[test]
fn functions_are_first_class_arguments() {
    // (|f| f(3))(|x| x * 2)  ==>  6
    let apply = lambda(&["f"], call(var("f", 1), vec![int(3, 2)], 3), 0);
    let double = lambda(&["x"], binary(Mul, var("x", 5), int(2, 6), 7), 4);
    assert_eq!(value_of(&call(apply, vec![double], 8)), Value::Int(6));
}

#[test]
fn return_is_caught_at_the_call_boundary() {
    // (|| { return 7; 9 })()  ==>  7   (the tail 9 never runs)
    let body = block(vec![expr_stmt(return_(int(7, 2), 1))], Some(int(9, 3)), 0);
    let f = lambda(&[], body, 4);
    assert_eq!(value_of(&call(f, vec![], 5)), Value::Int(7));
}

#[test]
fn the_wrong_number_of_arguments_is_stuck() {
    // (|x, y| 0)(1)
    let f = lambda(&["x", "y"], int(0, 1), 0);
    let err = stuck(&call(f, vec![int(1, 2)], 3));
    assert!(
        matches!(err, RuntimeError::ArityMismatch { expected: 2, found: 1, span }
        if span.start == 3)
    );
}

#[test]
fn calling_a_non_function_is_stuck() {
    let err = stuck(&call(int(5, 1), vec![], 0));
    assert!(matches!(err, RuntimeError::NotAFunction { span, .. } if span.start == 0));
}

// ---- whole programs, end to end ----

#[test]
fn a_program_returns_its_main_value() {
    let (v, _) = run_program("fn main() -> Int = 1 + 2;");
    assert_eq!(v, Value::Int(3));
}

#[test]
fn print_reaches_the_output() {
    let (v, out) = run_program("fn main() -> () { print(42); }");
    assert_eq!(v, Value::Unit);
    assert_eq!(out, "42");
}

#[test]
fn a_program_may_call_another_top_level_function() {
    let (v, _) = run_program("fn double(x: Int) -> Int = x * 2; fn main() -> Int = double(21);");
    assert_eq!(v, Value::Int(42));
}

#[test]
fn top_level_functions_are_recursive() {
    let src = "fn fact(n: Int) -> Int = if n == 0 { 1 } else { n * fact(n - 1) };\
               fn main() -> Int = fact(5);";
    assert_eq!(run_program(src).0, Value::Int(120));
}

#[test]
fn a_native_prelude_function_runs_in_a_program() {
    let (_, out) = run_program("fn main() -> () { println(abs(-5)); }");
    assert_eq!(out, "5\n");
}

#[test]
fn a_global_initializer_is_visible_to_main() {
    let (v, _) = run_program("let g = 10; fn main() -> Int = g + 5;");
    assert_eq!(v, Value::Int(15));
}

#[test]
fn return_from_a_function_body_yields_the_calls_value() {
    let src = "fn f(x: Int) -> Int { if x > 0 { return x; } 0 } fn main() -> Int = f(9);";
    assert_eq!(run_program(src).0, Value::Int(9));
}

#[test]
fn a_program_can_get_stuck_at_runtime() {
    // untyped at M4: `1 + true` is a runtime type error
    let err = run_stuck("fn main() -> Int = 1 + true;");
    assert!(matches!(err, RuntimeError::TypeError { .. }));
}

// ---- input: the read_* family (native, so available from M4) ----

#[test]
fn readers_consume_tokens_and_lines() {
    // whitespace-delimited tokens, left to right
    let (v, _) = run_with_input("fn main() -> Int = read_int() + read_int();", "3 4\n");
    assert_eq!(v, Value::Int(7));
    let (v, _) = run_with_input(
        "fn main() -> [Bool] = [read_bool(), read_bool()];",
        "true false",
    );
    assert_eq!(v, vlist(vec![Value::Bool(true), Value::Bool(false)]));
    // read_line reads through the newline
    let (v, _) = run_with_input("fn main() -> String = read_line();", "hello world\nnext");
    assert_eq!(v, vstr("hello world"));
}

#[test]
fn opt_readers_recover_and_detect_eof() {
    // a bad token is `None`, not stuck
    let (v, _) = run_with_input("fn main() -> Option<Int> = read_int_opt();", "oops");
    assert_eq!(v, vctor("None", vec![]));
    // read_line_opt tells an empty line from end of input
    let (v, _) = run_with_input("fn main() -> Option<String> = read_line_opt();", "");
    assert_eq!(v, vctor("None", vec![]));
    let (v, _) = run_with_input("fn main() -> Option<String> = read_line_opt();", "\n");
    assert_eq!(v, vctor("Some", vec![vstr("")]));
}

#[test]
fn a_plain_reader_is_stuck_with_an_input_error() {
    // malformed token
    let err = run_stuck_with_input("fn main() -> Int = read_int();", "not_a_number");
    assert!(matches!(err, RuntimeError::InputError { .. }));
    // end of input
    let err = run_stuck_with_input("fn main() -> Int = read_int();", "");
    assert!(matches!(err, RuntimeError::InputError { .. }));
}

#[test]
fn globals_initialize_in_dependency_order() {
    // `a` names `b`, declared later: `b` is initialized first
    let (v, _) = run_program("let a = b + 1; let b = 10; fn main() -> Int = a;");
    assert_eq!(v, Value::Int(11));
}

#[test]
fn a_global_dependency_through_a_function_is_honoured() {
    let src = "let a = f(); fn f() -> Int = b * 2; let b = 21; fn main() -> Int = a;";
    assert_eq!(run_program(src).0, Value::Int(42));
}

#[test]
fn a_cycle_among_globals_is_an_initialization_error() {
    let err = run_stuck("let a = b + 1; let b = a + 1; fn main() -> Int = a;");
    assert!(matches!(err, RuntimeError::InitializationCycle { .. }));
}

#[test]
fn a_prelude_name_cannot_be_redeclared() {
    use bridger::interp::Interpreter;
    use bridger::parser::ParseError;
    // a native primitive
    let mut it = Interpreter::new();
    assert!(matches!(
        it.load_program("fn print(x: Int) -> () = (); fn main() -> () = ();"),
        Err(ParseError::Invalid { .. })
    ));
    // a Bridger-written prelude definition
    let mut it2 = Interpreter::new();
    assert!(matches!(
        it2.load_program("fn map(x: Int) -> Int = x; fn main() -> () = ();"),
        Err(ParseError::Invalid { .. })
    ));
}

#[test]
fn a_built_in_type_name_cannot_be_declared() {
    use bridger::interp::Interpreter;
    use bridger::parser::ParseError;
    let mut it = Interpreter::new();
    assert!(matches!(
        it.load_program("struct Int { v: Int } fn main() -> () = ();"),
        Err(ParseError::Invalid { .. })
    ));
}

#[test]
fn read_line_after_a_token_reader_skips_a_blank_remainder() {
    // the classic pitfall: a number on one line, a name on the next
    let src = "fn main() -> (Int, String) { let n = read_int(); (n, read_line()) }";
    assert_eq!(
        run_with_input(src, "5\nhello\n").0,
        vtuple(vec![Value::Int(5), Value::Str("hello".into())])
    );
    // trailing spaces after the token count as blank too
    assert_eq!(
        run_with_input(src, "5   \nhello").0,
        vtuple(vec![Value::Int(5), Value::Str("hello".into())])
    );
    // content on the same line is that line's remainder, verbatim
    assert_eq!(
        run_with_input(src, "5 rest\nnext").0,
        vtuple(vec![Value::Int(5), Value::Str(" rest".into())])
    );
    // an empty line the cursor starts at is still an empty line
    let two = "fn main() -> (String, String) { let n = read_int(); (read_line(), read_line()) }";
    assert_eq!(
        run_with_input(two, "5\n\nx").0,
        vtuple(vec![Value::Str("".into()), Value::Str("x".into())])
    );
    // and nothing after the token is end of input
    let err = run_stuck_with_input("fn main() -> String { read_int(); read_line() }", "5\n");
    assert!(matches!(err, RuntimeError::InputError { .. }));
}

#[test]
fn printing_a_cyclic_reference_terminates() {
    // a cell that (through a list) contains itself prints `ref(…)` at the knot
    let src = "fn main() -> () { let r = ref []; r := [r]; println(r) }";
    let (_, out) = run_program(src);
    assert_eq!(out, "ref([ref(…)])\n");
}

// ---- the prelude's higher-order functions (native: they iterate) ----

#[test]
fn map_filter_fold_run() {
    assert_eq!(
        run_program("fn main() -> [Int] = map([1, 2, 3], |x| x * 2);").0,
        vlist(vec![Value::Int(2), Value::Int(4), Value::Int(6)])
    );
    assert_eq!(
        run_program("fn main() -> [Int] = filter([1, 2, 3, 4], |x| even(x));").0,
        vlist(vec![Value::Int(2), Value::Int(4)])
    );
    assert_eq!(
        run_program("fn main() -> Int = fold([1, 2, 3, 4], 0, |a, x| a + x);").0,
        Value::Int(10)
    );
}

#[test]
fn the_higher_order_functions_iterate_rather_than_recurse() {
    // a 100 000-element list costs no evaluator stack: `fold` and `map` walk
    // the list natively and call the function once per element
    let (v, _) = run_program("fn main() -> Int = fold(range(0, 100000), 0, |a, x| a + x);");
    assert_eq!(v, Value::Int(4_999_950_000));
    let (v, _) = run_program("fn main() -> Int = len(map(range(0, 100000), |x| x + 1));");
    assert_eq!(v, Value::Int(100_000));
}

#[test]
fn the_function_handed_to_a_higher_order_function_goes_through_the_call_rule() {
    // arity and the result are the call rule's business: a wrong arity is an
    // `ArityMismatch`, a non-Bool from `filter`'s predicate a `TypeError`, and a
    // `return` inside the function ends that one call only
    assert!(matches!(
        run_stuck("fn main() -> Int = fold([1], 0, |x| x);"),
        RuntimeError::ArityMismatch {
            expected: 1,
            found: 2,
            ..
        }
    ));
    assert!(matches!(
        run_stuck("fn main() -> [Int] = filter([1], |x| x);"),
        RuntimeError::TypeError { .. }
    ));
    assert_eq!(
        run_program("fn main() -> Int = fold([1, 2], 0, |a, x| { return a + x; });").0,
        Value::Int(3)
    );
}

#[test]
fn deep_recursion_has_room() {
    // `cargo test` runs on a 2 GiB stack (`.cargo/config.toml`), so a program
    // recursing tens of thousands of calls deep runs rather than overflowing
    let src = "fn count(n: Int) -> Int = if n == 0 { 0 } else { 1 + count(n - 1) };\
               fn main() -> Int = count(20000);";
    assert_eq!(run_program(src).0, Value::Int(20000));
}

// ---- whole-program properties that reach the evaluator only through `main` ----
//
// Each of these needs `eval_program`, whose call to `main` goes through the
// `Call` arm, so they are in force from M4 even where the property itself
// (a lexer rule, the canonical print format) belongs to an earlier part.

#[test]
fn the_least_integer_is_a_literal_only_with_its_sign() {
    use bridger::interp::Interpreter;
    // `-9223372036854775808` is i64::MIN, as a literal and as an operand
    let (v, _) = run_program("fn main() -> Int = -9223372036854775808;");
    assert_eq!(v, Value::Int(i64::MIN));
    let (v, _) = run_program("fn main() -> Int = 0 + -9223372036854775808;");
    assert_eq!(v, Value::Int(i64::MIN));
    // and the greatest is still fine
    let (v, _) = run_program("fn main() -> Int = 9223372036854775807;");
    assert_eq!(v, Value::Int(i64::MAX));
    // the magnitude alone is out of range — after a binary minus, or after a
    // prefix minus separated by a space, which is negation applied to it
    for src in [
        "fn main() -> Int = 9223372036854775808;",
        "fn main() -> Int = 1 - 9223372036854775808;",
        "fn main() -> Int = - 9223372036854775808;",
        "fn main() -> Int = 9223372036854775809;",
    ] {
        assert!(Interpreter::new().load_program(src).is_err(), "{src}");
    }
}

#[test]
fn a_minus_before_digits_is_a_literal_unless_it_follows_an_operand() {
    // `x -1` is subtraction: the lexer glues `-` to digits only where a `-`
    // cannot be binary, and Bridger has no juxtaposition
    let (v, _) = run_program("fn main() -> Int { let x = 3; x -1 }");
    assert_eq!(v, Value::Int(2));
    let (v, _) = run_program("fn main() -> Int = f(2) -1; fn f(x: Int) -> Int = x;");
    assert_eq!(v, Value::Int(1));
    // negation still applies to a literal with a space, and to a literal
    let (v, _) = run_program("fn main() -> Int = - 1;");
    assert_eq!(v, Value::Int(-1));
    let (v, _) = run_program("fn main() -> Int = --1;");
    assert_eq!(v, Value::Int(1));
    // after a braced statement the `-` is read as a prefix by the grammar
    let (v, _) = run_program("fn main() -> Int { ref c = false; while deref c { } -1 }");
    assert_eq!(v, Value::Int(-1));
}

#[test]
fn a_negative_literal_may_group_its_digits() {
    let (v, _) = run_program("fn main() -> Int = -123_456;");
    assert_eq!(v, Value::Int(-123_456));
    let (v, _) = run_program("fn main() -> Int = -1_000_000 + 1_000;");
    assert_eq!(v, Value::Int(-999_000));
}

#[test]
fn main_is_checked_before_anything_runs() {
    use bridger::interp::Interpreter;
    // no `main`: reported before the global initializer runs
    let mut it = Interpreter::new();
    it.load_program("let g = { println(\"init\"); 1 };")
        .expect("parse");
    assert!(matches!(it.eval_program(), Err(RuntimeError::Main { .. })));
    assert_eq!(it.output(), "");
    // `main` takes no parameters
    let err = run_stuck("fn main(x: Int) -> Int = x;");
    assert!(matches!(err, RuntimeError::Main { .. }));
    // its result is the program's value, whatever the type
    assert_eq!(run_program("fn main() -> Int = 5;").0, Value::Int(5));
}

#[test]
fn nested_strings_escape_every_control_character() {
    let (_, out) = run_program("fn main() -> () = println([\"\\0\", \"\\u{1}\", \"a\\tb\"]);");
    assert_eq!(out, "[\"\\0\", \"\\u{1}\", \"a\\tb\"]\n");
}

#[test]
fn contains_refuses_a_function_like_equality_does() {
    let err = run_stuck("fn main() -> Bool { let f = |x: Int| x; contains([f], f) }");
    assert!(matches!(err, RuntimeError::NotComparable { .. }));
}

#[test]
fn read_int_reads_an_integer_literal_and_leaves_anything_else_unread() {
    // the token must be a literal as the lexer reads one: no `+`, thousands
    // grouping only, no leading zeros — and a token that is not one stays
    // in the input for the next reader (`-1` and `"-"` stand for "failed")
    let src = "fn main() -> ([Int], [String]) {\
        let a = read_int_opt(); let b = read_line_opt();\
        let c = read_int_opt();\
        let d = read_int_opt(); let e = read_line_opt();\
        let f = read_int_opt();\
        let g = read_int_opt(); let h = read_line_opt();\
        ([unwrap_or(a, -1), unwrap_or(c, -1), unwrap_or(d, -1), unwrap_or(f, -1),\
          unwrap_or(g, -1)],\
         [unwrap_or(b, \"-\"), unwrap_or(e, \"-\"), unwrap_or(h, \"-\")]) }";
    let (v, _) = run_with_input(src, "+5\n1_000\n007\n-0\n1_0000\n");
    let ints = [-1, 1000, -1, 0, -1].into_iter().map(Value::Int).collect();
    assert_eq!(
        v,
        vtuple(vec![
            vlist(ints),
            vlist(vec![vstr("+5"), vstr("007"), vstr("1_0000")]),
        ])
    );
}

#[test]
fn runaway_recursion_is_a_stuck_state_and_keeps_the_output() {
    use bridger::interp::Interpreter;
    let mut it = Interpreter::new();
    it.load_program(
        "fn f(n: Int) -> Int = if n == 0 { 0 } else { 1 + f(n - 1) };\
         fn main() -> Int { println(\"before\"); f(1000000) }",
    )
    .expect("parse");
    assert!(matches!(
        it.eval_program(),
        Err(RuntimeError::StackOverflow { .. })
    ));
    assert_eq!(it.output(), "before\n");
}

#[test]
fn a_range_too_large_to_build_is_a_stuck_state() {
    assert!(matches!(
        run_stuck("fn main() -> Int = len(range(0, 9223372036854775807));"),
        RuntimeError::RangeTooLarge { .. }
    ));
    assert!(matches!(
        run_stuck("fn main() -> Int = len(range(-9223372036854775808, 9223372036854775807));"),
        RuntimeError::RangeTooLarge { .. }
    ));
}

#[test]
fn read_line_drops_a_windows_line_ending() {
    let (v, _) = run_with_input(
        "fn main() -> (String, String) = (read_line(), read_line());",
        "hello world\r\nnext\r\n",
    );
    assert_eq!(v, vtuple(vec![vstr("hello world"), vstr("next")]));
}

#[test]
fn an_input_error_says_whether_the_input_ended_or_what_it_found() {
    let ended = run_stuck_with_input("fn main() -> Int = read_int();", "");
    assert!(
        matches!(&ended, RuntimeError::InputError { found: None, .. }),
        "{ended:?}"
    );
    let bad = run_stuck_with_input("fn main() -> Int = read_int();", "abc\n");
    assert!(
        matches!(&bad, RuntimeError::InputError { found: Some(t), .. } if t == "abc"),
        "{bad:?}"
    );
}

/// Until M8 `min` is native; from M8 it is Bridger-written and the same
/// program is stuck on `cmp` being no method of a tuple.
#[cfg(not(feature = "m8"))]
#[test]
fn ordering_on_values_with_no_order_is_its_own_stuck_state() {
    // tuples admit equality but no order: `min` says so, rather than
    // "admits no equality"
    assert!(matches!(
        run_stuck("fn main() -> (Int, Int) = min((1, 2), (1, 3));"),
        RuntimeError::NotOrdered { .. }
    ));
    assert_eq!(
        run_program("fn main() -> Bool = (1, 2) == (1, 3);").0,
        Value::Bool(false)
    );
}

#[test]
fn the_interpreter_knows_whether_its_last_output_line_is_unfinished() {
    // the binary ends such a line before printing a diagnostic
    use bridger::interp::Interpreter;
    for (src, open) in [
        ("fn main() -> () = print(\"abc\");", true),
        ("fn main() -> () = println(\"abc\");", false),
        ("fn main() -> () { print(\"a\"); print(\"b\\n\") }", false),
        ("fn main() -> () = ();", false),
    ] {
        let mut it = Interpreter::new();
        it.load_program(src).expect("parse");
        it.eval_program().expect("run");
        assert_eq!(it.output_line_open(), open, "{src}");
    }
}

// ---- eleventh review round: sentences of Appendix D and C with no test ----

/// Load `src` unchecked, run it, and return the stuck state with what was
/// printed before it.
fn stuck_with_output(src: &str) -> (RuntimeError, String) {
    use bridger::interp::Interpreter;
    let mut it = Interpreter::new();
    it.load_program(src).expect("parse");
    let err = it.eval_program().expect_err("expected a stuck state");
    (err, it.output().to_string())
}

#[test]
fn operands_are_evaluated_left_to_right_threading_effects() {
    let src = "fn p(x: Int) -> Int { println(x); x }\
        fn f(a: Int, b: Int) -> Int = a + b;\
        fn main() -> () { let s = p(1) + p(2); let l = [p(3)] ++ [p(4)]; let t = (p(5), p(6));\
            let c = f(p(7), p(8)); let q = p(9) :: [p(10)]; }";
    assert_eq!(run_program(src).1, "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n");
}

#[test]
fn a_non_location_target_is_stuck_before_its_right_hand_side_runs() {
    let (err, out) = stuck_with_output("fn main() -> () { 1 := { println(\"r\"); 2 } }");
    assert!(matches!(err, RuntimeError::TypeError { .. }), "{err:?}");
    assert_eq!(out, "");
}

#[test]
fn a_non_function_is_stuck_before_its_arguments_and_a_wrong_arity_after_them() {
    let (err, out) = stuck_with_output("fn main() -> () { 1({ println(\"a\"); 2 }) }");
    assert!(matches!(err, RuntimeError::NotAFunction { .. }), "{err:?}");
    assert_eq!(out, "");
    let (err, out) = stuck_with_output(
        "fn f(x: Int) -> Int = x; fn main() -> () { f({ println(\"a\"); 1 }, 2) }",
    );
    assert!(matches!(err, RuntimeError::ArityMismatch { .. }), "{err:?}");
    assert_eq!(out, "a\n");
}

#[test]
fn independent_globals_initialize_in_alphabetical_order() {
    let src = "let b = { println(\"b\"); 1 }; let a = { println(\"a\"); 2 };\
        fn main() -> Int = a + b;";
    assert_eq!(run_program(src).1, "a\nb\n");
}

#[test]
fn references_compare_by_identity() {
    let src = "fn main() -> (Bool, Bool) { let a = ref 1; let b = ref 1; (a == b, a == a) }";
    assert_eq!(
        run_program(src).0,
        vtuple(vec![Value::Bool(false), Value::Bool(true)])
    );
}

#[test]
fn equality_on_a_function_is_stuck_even_between_a_value_and_itself() {
    assert!(matches!(
        run_stuck("fn f() -> Int = 1; fn main() -> Bool = f == f;"),
        RuntimeError::NotComparable { .. }
    ));
}

#[test]
fn a_block_ending_in_a_let_is_unit() {
    assert_eq!(
        run_program("fn main() -> () = { let x = 1; };").0,
        Value::Unit
    );
}

#[test]
fn printing_a_function_is_stuck_unchecked() {
    assert!(matches!(
        run_stuck("fn main() -> () = println(abs);"),
        RuntimeError::NotPrintable { .. }
    ));
}

/// Until M8 `minimum`/`maximum` are native; from M8 the same program is
/// stuck on `cmp` being no method of a tuple.
#[cfg(not(feature = "m8"))]
#[test]
fn minimum_and_maximum_on_unordered_elements_have_no_order() {
    assert!(matches!(
        run_stuck("fn main() -> Option<(Int, Int)> = minimum([(1, 2), (3, 4)]);"),
        RuntimeError::NotOrdered { .. }
    ));
}

#[test]
fn prelude_edge_cases_from_appendix_c() {
    // `abs` wraps at the least integer; `even`/`odd` by `% 2`, negatives
    // included; `head`/`tail` of the empty list; `range` empty when
    // `lo >= hi`; `contains` on references by identity
    let src =
        "fn main() -> ([Int], [Bool], Option<Int>, Option<[Int]>, [Int], [Int], Bool, Bool) {\
        let r = ref 1;\
        ([abs(-9223372036854775808), abs(-3)], [odd(-3), even(-4), odd(0)],\
         head([]), tail([]), range(5, 1), range(3, 3), contains([ref 1], r), contains([r], r)) }";
    let none = vctor("None", vec![]);
    assert_eq!(
        run_program(src).0,
        vtuple(vec![
            vlist(vec![Value::Int(-9223372036854775808), Value::Int(3)]),
            vlist(vec![
                Value::Bool(true),
                Value::Bool(true),
                Value::Bool(false)
            ]),
            none.clone(),
            none,
            vlist(vec![]),
            vlist(vec![]),
            Value::Bool(false),
            Value::Bool(true),
        ])
    );
}

#[test]
fn read_bool_opt_fails_without_consuming() {
    let (v, _) = run_with_input(
        "fn main() -> (Option<Bool>, String) = (read_bool_opt(), read_line());",
        "maybe\n",
    );
    assert_eq!(v, vtuple(vec![vctor("None", vec![]), vstr("maybe")]));
}

#[test]
fn nested_strings_and_references_print_in_canonical_text() {
    // a reference prints as `ref(v)`; a nested string quotes `"` and `\`
    // and writes a carriage return as `\r`
    let src = "fn main() -> () { print(ref 1); print(\" \"); print([\"a\\\\b\\\"c\\r\"]) }";
    assert_eq!(run_program(src).1, "ref(1) [\"a\\\\b\\\"c\\r\"]");
}

#[test]
fn main_with_type_parameters_is_stuck_unchecked() {
    assert!(matches!(
        run_stuck("fn main<T>() -> Int = 1;"),
        RuntimeError::Main { .. }
    ));
}

#[test]
fn an_expression_nested_past_the_evaluators_bound_is_a_clean_error() {
    // Unchecked, a tree too deep for the recursive evaluator would abort the
    // process; the driver rejects it first, at the same bound the type checker
    // uses from M5. A chain of unary minus, deeper than 150_000.
    let src = format!("fn main() -> Int = {}0;", "-".repeat(200_000));
    assert!(matches!(run_stuck(&src), RuntimeError::TooDeep { .. }));
    // The same guard covers a global initializer, walked before `main` runs.
    let g = format!("let g = {}0; fn main() -> Int = g;", "-".repeat(200_000));
    assert!(matches!(run_stuck(&g), RuntimeError::TooDeep { .. }));
}
