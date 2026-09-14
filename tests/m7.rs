//! Milestone M7 — algebraic data types (public subset).  [frozen — do not edit]
//!
//! Constructors, `match` with patterns and guards, and `?`. This is also when
//! the prelude's constructors (`Some`, `Ok`, `Less`, …) come to life.
//! Whole-program tests run through `eval_program`; type checks go through
//! `Checker::check_program`.
//!
//! Gated on the `m7` feature: compiled at every milestone from M7 on.

#![cfg(feature = "m7")]

mod common;

use bridger::interp::{Interpreter, RuntimeError, Value};
use bridger::relations::{check_rules, RuleError};
use bridger::types::{Checker, TyError};
use common::*;
use std::rc::Rc;

fn ctor(name: &str, args: Vec<Value>) -> Value {
    Value::Ctor(name.to_string(), Rc::from(args))
}
fn check_program(src: &str) -> Result<(), TyError> {
    let mut it = Interpreter::new();
    it.load_program(src).expect("program should parse");
    Checker::check_program(it.program()).map(drop)
}
/// The rule checks and, from M7, the guard checks — the static rule passes
/// `Interpreter::check` runs.
fn check_rules_src(src: &str) -> Result<(), RuleError> {
    use bridger::relations::check_guards;
    let mut it = Interpreter::new();
    it.load_program(src).expect("program should parse");
    check_rules(it.program())?;
    check_guards(it.program())
}

// ---- constructors and match ----

#[test]
fn a_constructor_builds_a_tagged_value() {
    assert_eq!(
        run_program("fn main() -> Option<Int> = Some(3);").0,
        ctor("Some", vec![Value::Int(3)])
    );
    assert_eq!(
        run_program("fn main() -> Option<Int> = None;").0,
        ctor("None", vec![])
    );
}

#[test]
fn match_takes_the_first_matching_arm() {
    let src = "fn describe(o: Option<Int>) -> Int = match o { None => 0, Some(x) => x };\
               fn main() -> Int = describe(Some(7));";
    assert_eq!(run_program(src).0, Value::Int(7));
    let none = "fn describe(o: Option<Int>) -> Int = match o { None => 0, Some(x) => x };\
                fn main() -> Int = describe(None);";
    assert_eq!(run_program(none).0, Value::Int(0));
}

#[test]
fn match_over_a_cons_list_recurses() {
    let src = "fn sum(xs: [Int]) -> Int = match xs { [] => 0, h :: t => h + sum(t) };\
               fn main() -> Int = sum([1, 2, 3, 4]);";
    assert_eq!(run_program(src).0, Value::Int(10));
}

#[test]
fn literal_guard_and_wildcard_patterns() {
    let prog = |n: i64| {
        format!(
            "fn classify(n: Int) -> String =\
                 match n {{ x if x > 0 => \"pos\", 0 => \"zero\", _ => \"neg\" }};\
             fn main() -> String = classify({n});"
        )
    };
    assert_eq!(run_program(&prog(5)).0, vstr("pos"));
    assert_eq!(run_program(&prog(0)).0, vstr("zero"));
    assert_eq!(run_program(&prog(-2)).0, vstr("neg"));
}

#[test]
fn an_or_pattern_matches_either_side() {
    let src = "fn small(n: Int) -> Bool = match n { 1 | 2 | 3 => true, _ => false };\
               fn main() -> [Bool] = [small(2), small(9)];";
    assert_eq!(
        run_program(src).0,
        vlist(vec![Value::Bool(true), Value::Bool(false)])
    );
}

#[test]
fn a_list_pattern_binds_the_rest() {
    let src = "fn tail_len(xs: [Int]) -> Int = match xs { [h, ...t] => len(t), [] => 0 };\
               fn main() -> Int = tail_len([1, 2, 3]);";
    assert_eq!(run_program(src).0, Value::Int(2));
}

#[test]
fn a_non_exhaustive_match_is_stuck() {
    let src = "fn main() -> Int = match Some(1) { None => 0 };";
    assert!(matches!(
        run_stuck(src),
        RuntimeError::NonExhaustiveMatch { .. }
    ));
}

// ---- `?` ----

const RESULT: &str = "fn f(x: Int) -> Result<Int, String> =\
         if x > 0 { Ok(x) } else { Err(\"neg\") };\
     fn g(x: Int) -> Result<Int, String> { let y = f(x)?; Ok(y + 1) }";

#[test]
fn try_unwraps_ok_and_propagates_err() {
    let ok = format!("{RESULT} fn main() -> Result = g(5);");
    assert_eq!(run_program(&ok).0, ctor("Ok", vec![Value::Int(6)]));
    let err = format!("{RESULT} fn main() -> Result = g(-1);");
    assert_eq!(run_program(&err).0, ctor("Err", vec![vstr("neg")]));
}

// ---- type checking ----

#[test]
fn a_generic_constructor_is_typed() {
    assert!(check_program("fn main() -> Option<Int> = Some(1);").is_ok());
    // wrong constructor arity
    assert!(matches!(
        check_program("fn main() -> Option<Int> = Some(1, 2);"),
        Err(TyError::ArityMismatch { .. })
    ));
}

#[test]
fn match_arms_must_agree() {
    let ok = "fn m(o: Option<Int>) -> Int = match o { None => 0, Some(x) => x };\
              fn main() -> Int = m(None);";
    assert!(check_program(ok).is_ok());
    let bad = "fn m(o: Option<Int>) -> Int = match o { None => 0, Some(x) => true };\
               fn main() -> Int = m(None);";
    assert!(matches!(check_program(bad), Err(TyError::Mismatch { .. })));
}

#[test]
fn try_checks_against_the_return_type() {
    let src = "fn f(x: Int) -> Result<Int, String> = if x > 0 { Ok(x) } else { Err(\"neg\") };\
               fn g(x: Int) -> Result<Int, String> { let y = f(x)?; Ok(y + 1) }\
               fn main() -> Result<Int, String> = g(5);";
    assert!(check_program(src).is_ok());
}

/// At M7, `?` requires the *exact* error type — `From`-conversion is M8. So a
/// cross-type `?` is a type error here even with an `impl From` in scope. M8
/// supersedes this (see `try_converts_the_error_through_from` in `m8.rs`), so
/// the property is retired from the M8 snapshot.
#[cfg(not(feature = "m8"))]
#[test]
fn a_cross_type_try_is_a_type_error_before_m8() {
    let src = "type Small = SmallCode(Int); type Big = BigCode(Int);\
        impl From<Small> for Big {\
            fn from(s: Small) -> Big = match s { SmallCode(c) => BigCode(c) };\
        }\
        fn f() -> Result<Int, Small> = Err(SmallCode(1));\
        fn g() -> Result<Int, Big> { let y = f()?; Ok(y) }\
        fn main() -> Result<Int, Big> = g();";
    assert!(matches!(check_program(src), Err(TyError::Mismatch { .. })));
}

#[test]
fn a_non_exhaustive_match_is_a_type_error() {
    // the `Some` case is missing, with no catch-all
    let missing = "fn m(o: Option<Int>) -> Int = match o { None => 0 };\
                   fn main() -> Int = m(None);";
    assert!(matches!(
        check_program(missing),
        Err(TyError::NonExhaustiveMatch { .. })
    ));
    // a catch-all makes it exhaustive
    let ok = "fn m(o: Option<Int>) -> Int = match o { None => 0, _ => 1 };\
              fn main() -> Int = m(None);";
    assert!(check_program(ok).is_ok());
    // a Bool match missing a case
    let bool_gap = "fn m(b: Bool) -> Int = match b { true => 1 };\
                    fn main() -> Int = m(true);";
    assert!(matches!(
        check_program(bool_gap),
        Err(TyError::NonExhaustiveMatch { .. })
    ));
}

#[test]
fn exhaustiveness_looks_inside_constructors() {
    // `Some(1)` covers one value of `Some`, so `Some(_)` is still missing
    let nested = "fn m(o: Option<Int>) -> Int = match o { Some(1) => 1, None => 0 };\
                  fn main() -> Int = m(None);";
    assert!(matches!(
        check_program(nested),
        Err(TyError::NonExhaustiveMatch { .. })
    ));
    // alternatives inside a constructor add up
    let ok = "fn m(o: Option<Bool>) -> Int =\
                  match o { Some(true) => 1, Some(false) => 2, None => 0 };\
              fn main() -> Int = m(None);";
    assert!(check_program(ok).is_ok());
}

#[test]
fn exhaustiveness_covers_tuples_lists_and_literals() {
    let pair_ok = "fn m(p: (Bool, Bool)) -> Int = match p { (true, _) => 1, (false, _) => 0 };\
                   fn main() -> Int = m((true, false));";
    assert!(check_program(pair_ok).is_ok());
    let pair_gap = "fn m(p: (Bool, Bool)) -> Int = match p { (true, true) => 1, (false, _) => 0 };\
                    fn main() -> Int = m((true, false));";
    assert!(matches!(
        check_program(pair_gap),
        Err(TyError::NonExhaustiveMatch { .. })
    ));
    let list_ok = "fn m(xs: [Int]) -> Int = match xs { [] => 0, h :: t => h };\
                   fn main() -> Int = m([1]);";
    assert!(check_program(list_ok).is_ok());
    let list_gap = "fn m(xs: [Int]) -> Int = match xs { [] => 0, [x] => x };\
                    fn main() -> Int = m([1]);";
    assert!(matches!(
        check_program(list_gap),
        Err(TyError::NonExhaustiveMatch { .. })
    ));
    // integers have no finite signature: only a catch-all completes them
    let int_gap = "fn m(n: Int) -> Int = match n { 1 => 1, 2 => 2 };\
                   fn main() -> Int = m(1);";
    assert!(matches!(
        check_program(int_gap),
        Err(TyError::NonExhaustiveMatch { .. })
    ));
    let int_ok = "fn m(n: Int) -> Int = match n { 1 => 1, _ => 0 };\
                  fn main() -> Int = m(1);";
    assert!(check_program(int_ok).is_ok());
}

#[test]
fn a_guarded_arm_does_not_count_toward_exhaustiveness() {
    let src = "fn m(o: Option<Int>) -> Int = match o { Some(x) if x > 0 => x, None => 0 };\
               fn main() -> Int = m(None);";
    assert!(matches!(
        check_program(src),
        Err(TyError::NonExhaustiveMatch { .. })
    ));
}

#[test]
fn a_match_guard_must_be_pure() {
    use bridger::relations::{check_guards, RuleError};
    let mut it = Interpreter::new();
    it.load_program(
        "fn m(n: Int) -> Int = match n { x if { print(x); true } => 1, _ => 0 };\
         fn main() -> Int = m(1);",
    )
    .expect("parse");
    assert!(matches!(
        check_guards(it.program()),
        Err(RuleError::ImpureGuard { .. })
    ));
    let mut ok = Interpreter::new();
    ok.load_program(
        "fn m(n: Int) -> Int = match n { x if even(x) => 1, _ => 0 }; fn main() -> Int = m(1);",
    )
    .expect("parse");
    assert!(check_guards(ok.program()).is_ok());
}

#[test]
fn a_pattern_binds_each_variable_once() {
    let twice = "fn m(p: (Int, Int)) -> Int = match p { (x, x) => x };\
                 fn main() -> Int = m((1, 2));";
    assert!(matches!(
        check_program(twice),
        Err(TyError::NonLinearPattern { .. })
    ));
    let nested = "fn m(o: Option<(Int, Int)>) -> Int = match o { Some((a, a)) => a, None => 0 };\
                  fn main() -> Int = m(None);";
    assert!(matches!(
        check_program(nested),
        Err(TyError::NonLinearPattern { .. })
    ));
    // the `...rest` binder counts too
    let rest = "fn m(xs: [Int]) -> [Int] = match xs { [x, ...x] => x, _ => [] };\
                fn main() -> [Int] = m([1, 2]);";
    assert!(matches!(
        check_program(rest),
        Err(TyError::NonLinearPattern { .. })
    ));
    // and an or-pattern must bind the rest name on both sides
    let sides = "fn m(xs: [Int]) -> [Int] = match xs { [_, ...r] | [] => [], _ => [] };\
                 fn main() -> [Int] = m([1, 2]);";
    assert!(matches!(
        check_program(sides),
        Err(TyError::OrPatternBindings { .. })
    ));
}

#[test]
fn or_pattern_alternatives_must_bind_alike() {
    // one side binds `y`, the other `z`
    let one_sided = "type T = A(Int) | B(Int);\
                     fn m(t: T) -> Int = match t { A(y) | B(z) => y };\
                     fn main() -> Int = m(A(1));";
    assert!(matches!(
        check_program(one_sided),
        Err(TyError::OrPatternBindings { .. })
    ));
    // both bind `x`, at different types
    let differing = "type T = A(Int) | B(Bool);\
                     fn m(t: T) -> Int = match t { A(x) | B(x) => 0 };\
                     fn main() -> Int = m(A(1));";
    assert!(matches!(
        check_program(differing),
        Err(TyError::OrPatternBindings { .. })
    ));
    // both bind `x` at `Int`: the body may use it
    let ok = "type T = A(Int) | B(Int);\
              fn m(t: T) -> Int = match t { A(x) | B(x) => x };\
              fn main() -> Int = m(B(2));";
    assert!(check_program(ok).is_ok());
}

#[test]
fn a_constructor_is_a_top_level_name() {
    use bridger::parser::ParseError;
    fn load(src: &str) -> Result<(), ParseError> {
        Interpreter::new().load_program(src)
    }
    fn rejected(src: &str, mentions: &str) {
        match load(src) {
            Err(ParseError::Invalid { message, .. }) => {
                assert!(message.contains(mentions), "{message}")
            }
            other => panic!("expected a rejection, got {other:?}"),
        }
    }
    // twice within one type, and across two types
    rejected("type A = X | X(Int);", "`X`");
    rejected(
        "type A = X(Int) | Y; type B = Z | X(Bool);",
        "constructor of `A`",
    );
    // against a declaration of another kind, in either order (the grammar
    // keeps functions, globals, and relations lowercase, so a constructor can
    // only meet a type, struct, trait, or constructor)
    rejected(
        "struct X { v: Int } type A = X | Y;",
        "`X` is already defined",
    );
    rejected("type A = X | Y; struct X { v: Int }", "constructor of `A`");
    rejected(
        "type A = X | Y; trait X { fn f(self) -> Int; }",
        "constructor of `A`",
    );
    rejected("type A = X | Y; type X = Q;", "constructor of `A`");
    // a constructor named after its own type is the same collision
    rejected(
        "type Wrapper = Wrapper(Int);",
        "`Wrapper` is already defined",
    );
    // and against the prelude's constructors
    rejected("type Maybe = Some(Int) | Nothing;", "`Option`");
    // distinct names are fine
    assert!(load("type A = X | Y; type B = Z(Int); fn w() -> A = X;").is_ok());
}

#[test]
fn the_least_integer_is_a_pattern() {
    let src = "fn f(n: Int) -> Int = match n { -9223372036854775808 => 1, _ => 0 };\
        fn main() -> (Int, Int) = (f(-9223372036854775808), f(0));";
    assert_eq!(
        run_program(src).0,
        vtuple(vec![Value::Int(1), Value::Int(0)])
    );
}

#[test]
fn a_try_inside_a_lambda_fixes_its_error_type() {
    let src = "type E1 = Oops;\
        fn a() -> Result<Int, E1> = Err(Oops);\
        fn d() -> Result<Int, E1> { let f = || { let v = a()?; Ok(v) }; f() }\
        fn main() -> Result<Int, E1> = d();";
    assert_eq!(
        run_typed(src).0,
        Value::Ctor(
            "Err".into(),
            Rc::from(vec![Value::Ctor("Oops".into(), Rc::from(vec![]))])
        )
    );
}

#[test]
fn a_guard_must_be_a_boolean_even_unchecked() {
    let err = run_stuck("fn main() -> Int = match 1 { x if 5 => x, _ => 0 };");
    assert!(matches!(err, RuntimeError::TypeError { .. }));
}

#[test]
fn the_first_impure_guard_in_source_order_is_reported() {
    use bridger::relations::{check_guards, RuleError};
    let src = "fn b(x: Int) -> Int = match x { y if { print(y); true } => y, _ => 0 };\
        fn a(x: Int) -> Int = match x { y if { print(y); true } => y, _ => 0 };\
        fn main() -> () = ();";
    let mut it = Interpreter::new();
    it.load_program(src).expect("parse");
    for _ in 0..8 {
        let err = check_guards(it.program()).unwrap_err();
        let RuleError::ImpureGuard { span, .. } = err else {
            panic!("{err}")
        };
        assert!(span.start < src.find("fn a").unwrap(), "reported {span:?}");
    }
}

// ---- the expected type reaches match arms and constructor arguments ----

#[test]
fn the_expected_type_reaches_match_arms_and_constructor_arguments() {
    // a lambda in each position is typed from the annotation, so its
    // parameter needs no annotation of its own (tuples and lists: `m5.rs`)
    let arms = "fn main() -> Int {\
        let f: fn(Int) -> Int = match true { true => |p| p + 1, false => |p| p - 1 };\
        f(1) }";
    assert!(check_program(arms).is_ok());
    let ctor = "fn main() -> Int { let o: Option<fn(Int) -> Int> = Some(|p| p + 1); 1 }";
    assert!(check_program(ctor).is_ok());
}

// ---- guards: what the purity analysis can and cannot follow ----

#[test]
fn a_guard_calling_a_local_closure_is_impure() {
    let src = "fn main() -> Int {\
        let g = |y: Int| { println(\"guard ran\"); true };\
        match 1 { k if g(k) => 1, _ => 2 } }";
    assert!(matches!(
        check_rules_src(src),
        Err(RuleError::ImpureGuard { ref witness, .. }) if witness == &["g".to_string()]
    ));
    // a guard reaching a function with a function-typed parameter is rejected
    // for the same reason a filter is
    let hof = "fn app(g: fn(Int) -> Bool, x: Int) -> Bool = g(x);\
        fn main() -> Int = match 1 { k if app(|y| y > 0, k) => 1, _ => 2 };";
    assert!(matches!(
        check_rules_src(hof),
        Err(RuleError::HigherOrder { .. })
    ));
}

#[test]
fn a_try_in_a_guard_is_control_not_an_effect() {
    // `e?` is a `match` and a `return`; with no `impl From` in play it is pure
    let src = "fn f(x: Int) -> Option<Int> = if x > 0 { Some(x) } else { None };\
        fn g(x: Int) -> Option<String> =\
            Some(match x { n if f(n)? > 0 => \"pos\", _ => \"other\" });\
        fn main() -> (Option<String>, Option<String>) = (g(1), g(-1));";
    assert!(check_rules_src(src).is_ok());
    assert_eq!(
        run_typed(src).0,
        vtuple(vec![ctor("Some", vec![vstr("pos")]), ctor("None", vec![])])
    );
}

#[test]
fn unchecked_constructors_are_checked_against_their_declaration() {
    assert!(matches!(
        run_stuck("fn main() -> Option<Int> = Some(1, 2);"),
        RuntimeError::ArityMismatch {
            expected: 1,
            found: 2,
            ..
        }
    ));
    assert!(matches!(
        run_stuck("fn main() -> Int { let x = Foo(1); 1 }"),
        RuntimeError::UnboundVariable { .. }
    ));
}

/// Until M8 the bounded prelude functions are native over the built-in types,
/// so a user `impl` is not evidence the runtime can honor: the bound fails
/// statically rather than the program getting stuck. Its M8 twin runs.
#[cfg(not(feature = "m8"))]
#[test]
fn a_user_impl_does_not_meet_a_bound_before_m8() {
    let src = "type Bag = MkBag([Int]);\
        impl Len for Bag { fn length(self) -> Int = 7; }\
        fn main() -> Int = len(MkBag([1]));";
    assert!(matches!(
        check_program(src),
        Err(TyError::UnsatisfiedBound { .. })
    ));
}

#[cfg(not(feature = "m8"))]
#[test]
fn a_bound_a_user_impl_would_meet_is_explained_before_m8() {
    // Plumbing: the help text is the reference's, not a student's; the payload
    // it reads is the graded part.
    let err = check_program(
        "type Bag = MkBag([Int]); impl Len for Bag { fn length(self) -> Int = 7; }\
         fn main() -> Int = len(MkBag([1]));",
    )
    .unwrap_err();
    assert!(err.help().is_some_and(|h| h.contains("M8")), "{err:?}");
}

#[test]
fn a_bare_constructor_is_not_a_value_and_says_how_to_pass_one() {
    // Plumbing: the help text is the reference's, not a student's; the payload
    // it reads is the graded part.
    let err = check_program("fn main() -> () { let f = Some; }").unwrap_err();
    assert!(
        matches!(err, TyError::ArityMismatch { found: 0, .. }),
        "{err:?}"
    );
    assert!(
        err.help().is_some_and(|h| h.contains("|x| Some(x)")),
        "{err:?}"
    );
}

#[test]
fn a_misplaced_try_is_explained() {
    assert!(matches!(
        check_program(
            "fn half(n: Int) -> Option<Int> = if even(n) { Some(n / 2) } else { None };\
             fn f(n: Int) -> Int = half(n)? + 1; fn main() -> Int = f(4);"
        ),
        Err(TyError::TryInNonCarrierFunction { .. })
    ));
    assert!(matches!(
        check_program("fn f() -> Option<Int> = Some(1?); fn main() -> Option<Int> = f();"),
        Err(TyError::TryOnNonCarrier { .. })
    ));
}

#[test]
fn a_pattern_of_the_wrong_type_blames_the_pattern() {
    // the scrutinee's type is what is expected; the pattern's is what was found
    match check_program(
        "fn f(o: Option<Int>) -> Int = match o { Ok(y) => y, _ => 0 }; fn main() -> Int = f(None);",
    ) {
        Err(TyError::Mismatch {
            expected, found, ..
        }) => {
            assert!(expected.to_string().starts_with("Option"), "{expected}");
            assert!(found.to_string().starts_with("Result"), "{found}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_try_in_a_lambda_and_a_wrong_pattern_shape_are_explained() {
    assert!(matches!(
        check_program("fn main() -> Int { let g = |x: Option<Int>| x?; 1 }"),
        Err(TyError::TryInLambda { .. })
    ));
    assert!(matches!(
        check_program("type T = A(Int); fn main() -> Int = match A(1) { T { x } => x };"),
        Err(TyError::NotAStruct { .. })
    ));
}

// ---- moved here from earlier milestones: they need `match` or `Ctor` ----

#[test]
fn list_recursion_is_linear_because_tails_are_shared() {
    // taking a list apart with `h :: t` and building one with `::` each
    // touch one cell, so these run in time proportional to the list
    let src = "fn total(xs: [Int]) -> Int = match xs { [] => 0, h :: t => h + total(t) };\
        fn build(n: Int) -> [Int] = if n == 0 { [] } else { n :: build(n - 1) };\
        fn main() -> (Int, Int) = (total(range(0, 50000)), len(build(50000)));";
    assert_eq!(
        run_program(src).0,
        vtuple(vec![Value::Int(49999 * 50000 / 2), Value::Int(50000)])
    );
    // a `...rest` binding shares the tail too
    let walk = "fn main() -> Int {\
        ref n = 0; ref cur = range(0, 100000);\
        while len(deref cur) > 0 {\
            match deref cur { [x, ...rest] => { n := deref n + x; cur := rest }, [] => () } }\
        deref n }";
    assert_eq!(run_program(walk).0, Value::Int(99999 * 100000 / 2));
}

#[test]
fn a_filter_may_bind_names_in_a_match_and_name_natives() {
    // a `match` arm's pattern variables are its own, not logic variables
    let src = "relation s : (Option<Int>); relation r : (Option<Int>);\
        rule r(x) :- s(x) and match x { Some(y) => y > 0, None => false };\
        fn main() -> [Option<Int>] { add s(Some(1)); add s(Some(-1)); add s(None); solutions r(?x) }";
    assert!(check_rules_src(src).is_ok());
    assert_eq!(
        run_typed(src).0,
        vlist(vec![vctor("Some", vec![Value::Int(1)])])
    );
}

#[test]
fn a_bare_none_is_undetermined_and_a_returning_match_is_diverging() {
    assert!(matches!(
        check_program("fn main() -> () = println(None);"),
        Err(TyError::Ambiguous { .. })
    ));
    assert!(check_program(
        "fn f(o: Option<Int>) -> Int { match o { None => return 0, Some(_) => return 1 }; 5 }\
         fn main() -> Int = f(None);"
    )
    .is_ok());
}

// ---- seventh review round ----

#[test]
fn a_try_in_a_lambda_relabels_only_the_body_value() {
    let half = "fn half(n: Int) -> Option<Int> = if even(n) { Some(n / 2) } else { None };";
    // a mismatch elsewhere in the body is what it is
    for body in [
        "{ let z = half(x)?; z + \"s\" }",
        "{ let z = half(x)?; let w: Int = \"s\"; Some(z) }",
        "{ let z = half(x)?; let g = |y| y + \"s\"; Some(z) }",
    ] {
        let src = format!("{half} fn main() -> () {{ let f = |x| {body}; }}");
        assert!(
            matches!(check_program(&src), Err(TyError::Mismatch { .. })),
            "{body}"
        );
    }
    // the body's own value is what the `?` pinned
    let src = format!(
        "{half} fn main() -> () {{ let f = |x| {{ let z = half(x)?; \
         if z > 1 {{ Some(z) }} else {{ 0 }} }}; }}"
    );
    assert!(matches!(
        check_program(&src),
        Err(TyError::TryInLambda { .. })
    ));
    // `?` on an operand that leaves never applies
    let src = format!("{half} fn f() -> Option<Int> {{ let z = {{ return None; }}?; Some(z) }}");
    assert!(check_program(&src).is_ok());
    // a lambda whose written result type is not a carrier is a lambda too
    let src = format!(
        "{half} fn main() -> Int {{ let f: fn(Option<Int>) -> Int = |o| o? + 1; f(Some(1)) }}"
    );
    assert!(matches!(
        check_program(&src),
        Err(TyError::TryInLambda { .. })
    ));
}

// ---- eleventh review round ----

#[test]
fn exhaustiveness_over_unit_and_strings() {
    assert!(check_program("fn main() -> Int = match () { () => 1 };").is_ok());
    assert!(matches!(
        check_program(
            "fn f(s: String) -> Int = match s { \"a\" => 1 }; fn main() -> Int = f(\"a\");"
        ),
        Err(TyError::NonExhaustiveMatch { .. })
    ));
    assert!(check_program(
        "fn f(s: String) -> Int = match s { \"a\" => 1, _ => 2 }; fn main() -> Int = f(\"a\");"
    )
    .is_ok());
}

#[test]
fn a_guard_need_not_be_total() {
    // recursion is allowed in a guard, which runs once per arm
    let mut it = Interpreter::new();
    it.load_program(
        "fn spin(n: Int) -> Bool = spin(n);\
         fn main() -> Int = match 1 { x if spin(x) => 1, _ => 0 };",
    )
    .expect("parse");
    assert!(it.check().is_ok());
}

#[test]
fn constructors_order_by_name_then_arguments_in_solutions() {
    let src = "relation o : (Option<Int>);\
        fn main() -> [Option<Int>] { add o(Some(1)); add o(None); add o(Some(0)); solutions o(?x) }";
    assert_eq!(
        run_program(src).0,
        vlist(vec![
            vctor("None", vec![]),
            vctor("Some", vec![Value::Int(0)]),
            vctor("Some", vec![Value::Int(1)]),
        ])
    );
}

#[test]
fn a_constructor_argument_that_is_a_tuple_keeps_its_parentheses_when_printed() {
    assert_eq!(
        run_program("fn main() -> () = print(Ok((1, \"a\")));").1,
        "Ok((1, \"a\"))"
    );
}

#[test]
fn a_guard_may_not_stringify_either() {
    let mut it = Interpreter::new();
    it.load_program(
        "fn f(r: ref<Int>) -> Int = match r { x if to_string(x) == \"ref(5)\" => 1, _ => 0 };\
         fn main() -> Int = f(ref 5);",
    )
    .expect("parse");
    assert!(matches!(
        it.check(),
        Err(bridger::interp::RunError::Rule(
            RuleError::ImpureGuard { .. }
        ))
    ));
}
