//! Milestone M6 — relations (public subset).  [frozen — do not edit]
//!
//! `Subst::unify` matches a generator against a fact; the engine derives the
//! least fixpoint and answers queries; the static checks (`check_rules`) sort a
//! rule body into generators and filters and reject unsafe rules. Whole-program
//! tests run through `eval_program`, so they exercise `add` / `clear` / queries
//! / `for q` end to end.
//!
//! Gated on the `m6` feature: compiled at every milestone from M6 on.

#![cfg(feature = "m6")]

mod common;

use bridger::interp::{Interpreter, Value};
use bridger::relations::{check_rules, RuleError, Subst, Term};
use bridger::types::Checker;
use common::*;

// ---- Subst::unify ----

fn v(n: i64) -> Value {
    Value::Int(n)
}

#[test]
fn unify_binds_variables_and_checks_constants() {
    let s = Subst::new();
    // path(?x, ?y) against (0, 1)  binds x=0, y=1
    let got = s
        .unify(
            &[Term::Var("x".into()), Term::Var("y".into())],
            &[v(0), v(1)],
        )
        .unwrap();
    assert_eq!(got.get("x"), Some(&v(0)));
    assert_eq!(got.get("y"), Some(&v(1)));
    // a constant must match
    assert!(s.unify(&[Term::Val(v(0))], &[v(1)]).is_none());
    // lengths must match
    assert!(s.unify(&[Term::Var("x".into())], &[v(0), v(1)]).is_none());
}

#[test]
fn unify_requires_a_repeated_variable_to_agree() {
    let s = Subst::new();
    // R(?x, ?x) against (2, 2) binds x=2; against (2, 3) fails
    assert!(s
        .unify(
            &[Term::Var("x".into()), Term::Var("x".into())],
            &[v(2), v(2)]
        )
        .is_some());
    assert!(s
        .unify(
            &[Term::Var("x".into()), Term::Var("x".into())],
            &[v(2), v(3)]
        )
        .is_none());
}

// ---- the engine, end to end ----

const GRAPH: &str = "relation edge : (Int, Int);\
     relation path : (Int, Int);\
     rule path(x, y) :- edge(x, y);\
     rule path(x, z) :- edge(x, y) and path(y, z);\
     fn setup() -> () { add edge(0, 1); add edge(1, 2); add edge(2, 3); }";

fn ints(v: &Value) -> Vec<i64> {
    match v {
        Value::List(xs) => xs
            .iter()
            .map(|e| match e {
                Value::Int(n) => *n,
                other => panic!("not an int: {other:?}"),
            })
            .collect(),
        other => panic!("not a list: {other:?}"),
    }
}

#[test]
fn a_boolean_query_reads_the_least_fixpoint() {
    let src = format!("{GRAPH} fn main() -> Bool {{ setup(); path(0, 3) }}");
    assert_eq!(run_program(&src).0, Value::Bool(true));
    let no = format!("{GRAPH} fn main() -> Bool {{ setup(); path(3, 0) }}");
    assert_eq!(run_program(&no).0, Value::Bool(false));
}

#[test]
fn solutions_collects_the_hole_values() {
    let src = format!("{GRAPH} fn main() -> [Int] {{ setup(); solutions path(0, ?x) }}");
    let mut got = ints(&run_program(&src).0);
    got.sort();
    assert_eq!(got, vec![1, 2, 3]);
}

#[test]
fn for_query_walks_every_solution() {
    // sum the nodes reachable from 0: 1 + 2 + 3 = 6
    let src = format!(
        "{GRAPH} fn main() -> Int {{ setup(); let s = ref 0; for path(0, ?x) {{ s := deref s + x }}; deref s }}"
    );
    assert_eq!(run_program(&src).0, Value::Int(6));
}

#[test]
fn add_then_clear_changes_what_holds() {
    let src = "relation r : (Int);\
        fn main() -> [Bool] {\
            add r(1);\
            let before = r(1);\
            clear r;\
            let after = r(1);\
            [before, after]\
        }";
    assert_eq!(
        run_program(src).0,
        vlist(vec![Value::Bool(true), Value::Bool(false)])
    );
}

#[test]
fn a_filter_narrows_the_derivation() {
    // only even destinations
    let src = "relation edge : (Int, Int);\
        relation reach : (Int);\
        rule reach(y) :- edge(x, y) and even(y);\
        fn main() -> [Int] {\
            add edge(0, 1); add edge(0, 2); add edge(0, 3); add edge(0, 4);\
            solutions reach(?y)\
        }";
    let mut got = ints(&run_program(src).0);
    got.sort();
    assert_eq!(got, vec![2, 4]);
}

// ---- the static checks (check_rules) ----

fn check_rules_src(src: &str) -> Result<(), RuleError> {
    let mut it = Interpreter::new();
    it.load_program(src).expect("program should parse");
    check_rules(it.program())
}

#[test]
fn well_formed_rules_pass() {
    assert!(check_rules_src(GRAPH).is_ok());
}

#[test]
fn the_head_must_name_a_declared_relation() {
    let src = "relation edge : (Int, Int); rule notrel(x, y) :- edge(x, y);";
    assert!(matches!(
        check_rules_src(src),
        Err(RuleError::NotARelation { .. })
    ));
}

#[test]
fn a_generators_arity_must_match() {
    let src = "relation edge : (Int, Int); relation p : (Int); rule p(x) :- edge(x);";
    assert!(matches!(check_rules_src(src), Err(RuleError::Arity { .. })));
}

#[test]
fn every_head_variable_must_be_generator_bound() {
    let src = "relation edge : (Int, Int); relation p : (Int, Int); rule p(x, y) :- edge(x, x);";
    assert!(matches!(
        check_rules_src(src),
        Err(RuleError::Unbound { ref var, .. }) if var == "y"
    ));
}

#[test]
fn an_impure_filter_is_rejected() {
    let src = "relation edge : (Int, Int);\
        relation ok : (Int);\
        fn noisy(x: Int) -> Bool { print(x); true }\
        rule ok(x) :- edge(x, y) and noisy(x);";
    assert!(matches!(
        check_rules_src(src),
        Err(RuleError::Impure { .. })
    ));
}

#[test]
fn a_recursive_filter_is_rejected() {
    let src = "relation r : (Int);\
        relation s : (Int);\
        fn loops(x: Int) -> Bool = loops(x);\
        rule s(x) :- r(x) and loops(x);";
    assert!(matches!(
        check_rules_src(src),
        Err(RuleError::Recursive { .. })
    ));
}

#[test]
fn rule_bodies_are_type_checked() {
    // a filter must be Bool: here `x` is an Int used where a Bool is needed
    let mut it = Interpreter::new();
    it.load_program("relation r : (Int); rule r(x) :- r(x) and x;")
        .expect("parse");
    assert!(matches!(
        Checker::check_program(it.program()),
        Err(bridger::types::TyError::Mismatch { .. })
    ));
    // a well-typed rule checks
    let mut ok = Interpreter::new();
    ok.load_program("relation r : (Int); rule r(x) :- r(x) and x > 0; fn main() -> () = ();")
        .expect("parse");
    assert!(Checker::check_program(ok.program()).is_ok());
}

#[test]
fn a_filter_reaching_a_higher_order_function_is_rejected() {
    // `pos` calls `apply1`, which takes a function-typed parameter — so a filter
    // calling `pos` cannot be shown total.
    let src = "relation r : (Int);\
        relation s : (Int);\
        fn apply1(f: fn(Int) -> Bool, x: Int) -> Bool = f(x);\
        fn pos(x: Int) -> Bool = apply1(|y| y > 0, x);\
        rule s(x) :- r(x) and pos(x);";
    assert!(matches!(
        check_rules_src(src),
        Err(RuleError::HigherOrder { .. })
    ));
}

#[test]
fn a_filter_may_read_a_global_constant() {
    // `limit` is a top-level `let`, visible in the rule like anywhere else
    let src = "relation r : (Int); relation big : (Int);\
        let limit = 2;\
        rule big(x) :- r(x) and x > limit;\
        fn main() -> [Int] { add r(1); add r(2); add r(3); add r(4); solutions big(?x) }";
    assert!(check_rules_src(src).is_ok());
    let mut got = ints(&run_program(src).0);
    got.sort();
    assert_eq!(got, vec![3, 4]);
}

#[test]
fn a_filter_reading_a_global_cell_is_impure() {
    // a global `ref` is readable only through `deref`, which is an effect
    let src = "relation r : (Int); relation big : (Int);\
        let limit = ref 2;\
        rule big(x) :- r(x) and x > deref limit;";
    assert!(matches!(
        check_rules_src(src),
        Err(RuleError::Impure { .. })
    ));
}

#[test]
fn a_hole_free_solutions_is_unit_or_nothing() {
    let yes = format!("{GRAPH} fn main() -> [()] {{ setup(); solutions path(0, 3) }}");
    assert_eq!(run_program(&yes).0, vlist(vec![Value::Unit]));
    let no = format!("{GRAPH} fn main() -> [()] {{ setup(); solutions path(3, 0) }}");
    assert_eq!(run_program(&no).0, vlist(vec![]));
    // and it is typed `[()]`
    let mut it = Interpreter::new();
    it.load_program(&yes).expect("parse");
    assert!(Checker::check_program(it.program()).is_ok());
}

#[test]
fn a_holed_query_in_boolean_position_tests_existence() {
    // `edge(0, ?x)` as a condition: some `x` makes it hold
    let src =
        format!("{GRAPH} fn main() -> (Bool, Bool) {{ setup(); (edge(0, ?x), edge(3, ?x)) }}");
    assert_eq!(
        run_program(&src).0,
        vtuple(vec![Value::Bool(true), Value::Bool(false)])
    );
    let mut it = Interpreter::new();
    it.load_program(&src).expect("parse");
    assert!(Checker::check_program(it.program()).is_ok());
}

#[test]
fn add_takes_a_ground_fact() {
    let mut it = Interpreter::new();
    it.load_program("relation r : (Int, Int); fn main() -> () = add r(1, ?x);")
        .expect("parse");
    assert!(matches!(
        Checker::check_program(it.program()),
        Err(bridger::types::TyError::HoleInAdd { .. })
    ));
}

#[test]
fn a_relation_call_inside_a_filter_is_impure() {
    // A relation applied in call syntax is a query wherever it appears. As a
    // bare conjunct it is a generator; inside any other expression it is a
    // filter that reads the database, and every query is impure.
    let negated = "relation q : (Int); relation r : (Int); relation p : (Int);\
        rule p(x) :- q(x) and not r(x);";
    assert!(matches!(
        check_rules_src(negated),
        Err(RuleError::Impure { ref witness, .. }) if witness == &["r".to_string()]
    ));
    let nested = "relation q : (Int); relation r : (Int); relation p : (Int);\
        rule p(x) :- q(x) and (r(x) or x > 1);";
    assert!(matches!(
        check_rules_src(nested),
        Err(RuleError::Impure { .. })
    ));
    // through a function, too — the witness names the path
    let via_fn = "relation q : (Int); relation r : (Int); relation p : (Int);\
        fn has(x: Int) -> Bool = r(x);\
        rule p(x) :- q(x) and has(x);";
    assert!(matches!(
        check_rules_src(via_fn),
        Err(RuleError::Impure { ref witness, .. })
            if witness == &["has".to_string(), "r".to_string()]
    ));
}

#[test]
fn a_query_during_the_fixpoint_is_a_stuck_state() {
    // `eval_program` alone skips the rule checks, so the engine sees the
    // filter; it reports the query instead of recursing.
    use bridger::interp::RuntimeError;
    let src = "relation q : (Int); relation r : (Int); relation p : (Int);\
        rule p(x) :- q(x) and not r(x);\
        fn main() -> [Int] { add q(1); solutions p(?x) }";
    assert!(matches!(run_stuck(src), RuntimeError::QueryInFilter { .. }));
}

#[test]
fn a_call_form_query_in_a_global_orders_initialization() {
    // `seen` asks a hole-free query in call syntax; the rule for `p` reads the
    // global `limit`, declared later. `limit` must be initialized first.
    let src = "relation q : (Int); relation p : (Int);\
        rule p(x) :- q(x) and x > limit;\
        rule q(5);\
        let seen = p(5);\
        let limit = 3;\
        fn main() -> Bool = seen;";
    assert_eq!(run_program(src).0, Value::Bool(true));
}

#[test]
fn a_rule_closes_over_the_global_scope() {
    // `main` binds a local `limit` that shadows the global; the rule was
    // declared at top level and reads the global, whatever the query site has.
    let src = "relation r : (Int); relation big : (Int);\
        let limit = 2;\
        rule big(x) :- r(x) and x > limit;\
        fn main() -> [Int] {\
            add r(1); add r(2); add r(3);\
            let limit = 0;\
            if limit > 0 { [] } else { solutions big(?x) }\
        }";
    let mut got = ints(&run_program(src).0);
    got.sort();
    assert_eq!(got, vec![3]);
}

#[test]
fn a_local_shadows_a_relation_in_call_position() {
    // Names resolve through the environment: `edge` here is the closure, so
    // `edge(1, 2)` is an application, not a query — typed and untyped alike.
    let src = "relation edge : (Int, Int);\
        fn main() -> Int { add edge(1, 2); let edge = |a: Int, b: Int| a + b; edge(1, 2) }";
    assert_eq!(run_program(src).0, Value::Int(3));
    assert_eq!(run_typed(src).0, Value::Int(3));
    // and the relation is still there once the local is out of scope
    let outer = "relation edge : (Int, Int);\
        fn shadowed() -> Int { let edge = |a: Int, b: Int| a + b; edge(1, 2) }\
        fn main() -> Bool { add edge(1, 2); shadowed(); edge(1, 2) }";
    assert_eq!(run_typed(outer).0, Value::Bool(true));
}

#[test]
fn a_query_form_under_a_shadow_is_not_a_relation() {
    use bridger::interp::RuntimeError;
    use bridger::types::TyError;
    // `add` / `solutions` / a holed query name a relation; a local of that
    // name is in the way
    let src = "relation edge : (Int, Int);\
        fn main() -> [Int] { let edge = 0; solutions edge(1, ?x) }";
    assert!(matches!(
        run_stuck(src),
        RuntimeError::NotARelation { ref name, .. } if name == "edge"
    ));
    let mut it = Interpreter::new();
    it.load_program(src).expect("parse");
    assert!(matches!(
        Checker::check_program(it.program()),
        Err(TyError::NotARelation { .. })
    ));
    let add = "relation edge : (Int, Int); fn main() -> () { let edge = 0; add edge(1, 2) }";
    assert!(matches!(run_stuck(add), RuntimeError::NotARelation { .. }));
}

#[test]
fn a_relation_name_is_not_a_value() {
    use bridger::types::TyError;
    let src = "relation edge : (Int, Int); fn main() -> Int { let r = edge; 0 }";
    let mut it = Interpreter::new();
    it.load_program(src).expect("parse");
    assert!(matches!(
        Checker::check_program(it.program()),
        Err(TyError::RelationNotAValue { ref name, .. }) if name == "edge"
    ));
}

#[test]
fn a_relation_column_must_admit_equality() {
    use bridger::types::TyError;
    fn check(src: &str) -> Result<(), TyError> {
        let mut it = Interpreter::new();
        it.load_program(src).expect("parse");
        Checker::check_program(it.program()).map(|_| ())
    }
    // a function column, directly or inside a compound
    assert!(matches!(
        check("relation apply : (fn(Int) -> Bool, Int);"),
        Err(TyError::RelationColumn { ref name, .. }) if name == "apply"
    ));
    assert!(matches!(
        check("relation opt : (Option<fn(Int) -> Int>);"),
        Err(TyError::RelationColumn { .. })
    ));
    // data columns are fine, recursive ones included
    assert!(check(
        "type Tree = Leaf | Node(Tree, Tree); relation t : (Tree, [Int]); fn main() -> () = ();"
    )
    .is_ok());
}

#[test]
fn a_query_or_add_with_the_wrong_arity_is_stuck() {
    use bridger::interp::RuntimeError;
    let add = "relation edge : (Int, Int); fn main() -> () = add edge(1);";
    assert!(matches!(
        run_stuck(add),
        RuntimeError::ArityMismatch {
            expected: 2,
            found: 1,
            ..
        }
    ));
    let call = "relation edge : (Int, Int); fn main() -> Bool = edge(1, 2, 3);";
    assert!(matches!(
        run_stuck(call),
        RuntimeError::ArityMismatch {
            expected: 2,
            found: 3,
            ..
        }
    ));
    let holed = "relation edge : (Int, Int); fn main() -> [Int] = solutions edge(?x);";
    assert!(matches!(
        run_stuck(holed),
        RuntimeError::ArityMismatch {
            expected: 2,
            found: 1,
            ..
        }
    ));
}

#[test]
fn a_repeated_hole_is_one_variable() {
    use bridger::types::TyError;
    // `r(?x, ?x)` has one hole, so its solutions are values, not pairs
    let same = "relation r : (Int, Int);\
        fn main() -> [Int] { add r(1, 1); add r(1, 2); add r(3, 3); solutions r(?x, ?x) }";
    let mut got = ints(&run_program(same).0);
    got.sort();
    assert_eq!(got, vec![1, 3]);
    let mut it = Interpreter::new();
    it.load_program(same).expect("parse");
    assert!(Checker::check_program(it.program()).is_ok());
    // holes are ordered by first appearance: (x, y, z)
    let three = "relation r : (Int, Int, Int, Int, Int);\
        fn main() -> [(Int, Int, Int)] { add r(1, 2, 1, 3, 2); add r(1, 2, 9, 3, 2);\
            solutions r(?x, ?y, ?x, ?z, ?y) }";
    assert_eq!(
        run_program(three).0,
        vlist(vec![vtuple(vec![
            Value::Int(1),
            Value::Int(2),
            Value::Int(3)
        ])])
    );
    let mut it = Interpreter::new();
    it.load_program(three).expect("parse");
    assert!(Checker::check_program(it.program()).is_ok());
    // one variable, so its columns must agree in type
    let mixed = "relation r : (Int, String); fn main() -> [Int] = solutions r(?x, ?x);";
    let mut it = Interpreter::new();
    it.load_program(mixed).expect("parse");
    assert!(matches!(
        Checker::check_program(it.program()),
        Err(TyError::Mismatch { .. })
    ));
}

#[test]
fn add_clear_and_solutions_are_ordinary_names_elsewhere() {
    // keywords only before a relation atom; names everywhere else
    let fns = "fn add(a: Int, b: Int) -> Int = a + b;\
        fn clear(x: Int) -> Int = 0 * x;\
        fn solutions(x: Int) -> Int = x;\
        fn main() -> Int { let clear = clear(5); add(solutions(1), clear) + add(1, 1) }";
    assert_eq!(run_program(fns).0, Value::Int(3));
    assert_eq!(run_typed(fns).0, Value::Int(3));
    // (as method names, fields, and parameters: `m8.rs`)
    // and the relation forms still read as before
    let rel = "relation add : (Int); relation r : (Int);\
        fn main() -> ([Int], [()], Bool) {\
            add add(1); add r(2); let before = solutions r(?x); clear r;\
            (solutions add(?x), solutions r(2), r(2) or before == [2])\
        }";
    assert_eq!(
        run_typed(rel).0,
        vtuple(vec![
            vlist(vec![Value::Int(1)]),
            vlist(vec![]),
            Value::Bool(true)
        ])
    );
}

#[test]
fn a_call_through_a_global_is_impure() {
    // the closure a global holds cannot be analysed, so a filter may not call it
    let src = "relation e : (Int); relation r : (Int);\
        let f = |x: Int| x > 1;\
        rule r(x) :- e(x) and f(x);";
    assert!(matches!(
        check_rules_src(src),
        Err(RuleError::Impure { ref witness, .. }) if witness == &["f".to_string()]
    ));
    // an impure function passed as a value is as good as called
    let passed = "relation e : (Int); relation r : (Int);\
        fn noisy(a: Bool, x: Int) -> Bool { print(x); a }\
        rule r(x) :- e(x) and fold([x], true, noisy);";
    assert!(matches!(
        check_rules_src(passed),
        Err(RuleError::Impure { ref witness, .. }) if witness.contains(&"noisy".to_string())
    ));
    // the prelude's recursion is structural: `fold` with a pure lambda is fine
    let ok = "relation e : (Int); relation r : (Int);\
        rule r(x) :- e(x) and fold([x, x], true, |a: Bool, y: Int| a and y > 0);\
        fn main() -> [Int] { add e(1); add e(-1); solutions r(?x) }";
    assert!(check_rules_src(ok).is_ok());
    assert_eq!(run_typed(ok).0, vlist(vec![Value::Int(1)]));
}

#[test]
fn the_database_cannot_change_inside_a_fixpoint() {
    // `eval_program` skips the static checks; the engine still refuses
    use bridger::interp::RuntimeError;
    let src = "relation e : (Int); relation r : (Int);\
        let f = |x: Int| { add e(x + 1); x > 0 };\
        rule e(1); rule r(x) :- e(x) and f(x);\
        fn main() -> [Int] = solutions r(?x);";
    assert!(matches!(run_stuck(src), RuntimeError::QueryInFilter { .. }));
}

#[test]
fn solutions_come_in_canonical_order() {
    // the same set of facts, whatever the derivation order, gives one list
    let a = "relation e : (Int); rule e(3);\
        fn main() -> [Int] { add e(2); add e(1); solutions e(?x) }";
    let b = "relation e : (Int); rule e(3);\
        fn main() -> [Int] { add e(1); add e(2); solutions e(?x) }";
    let want = vlist(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
    assert_eq!(run_program(a).0, want);
    assert_eq!(run_program(b).0, want);
    // tuples order by the first hole, then the next
    let pairs = "relation p : (Int, String);\
        fn main() -> [(Int, String)] { add p(2, \"a\"); add p(1, \"z\"); add p(1, \"b\"); solutions p(?x, ?y) }";
    assert_eq!(
        run_program(pairs).0,
        vlist(vec![
            vtuple(vec![Value::Int(1), Value::Str("b".into())]),
            vtuple(vec![Value::Int(1), Value::Str("z".into())]),
            vtuple(vec![Value::Int(2), Value::Str("a".into())]),
        ])
    );
    // and `for q` walks the same order
    let walk = "relation e : (Int);\
        fn main() -> () { add e(2); add e(1); for e(?x) { println(x) } }";
    assert_eq!(run_program(walk).1, "1\n2\n");
}

#[test]
fn the_old_parenthesized_solutions_is_explained() {
    let mut it = Interpreter::new();
    let err = it
        .load_program("relation e : (Int); fn main() -> [Int] = solutions(e(?x));")
        .unwrap_err();
    assert!(err.to_string().contains("without parentheses"), "{err}");
    // hole-free, it parses as a call to an unbound `solutions`, with a hint
    let mut it = Interpreter::new();
    it.load_program("relation e : (Int); fn main() -> Bool = solutions(e(1));")
        .expect("parse");
    let err = it.eval_program().unwrap_err();
    assert!(err.to_string().contains("without parentheses"), "{err}");
}

// ---- purity: calls the analysis cannot follow, and control that it can ----

#[test]
fn a_call_through_a_local_or_a_callback_it_cannot_follow_is_impure() {
    // an alias of a global closure: the call goes through a local
    let alias = "relation s : (Int); relation r : (Int);\
        let noisy = |x: Int| { println(\"e\"); x > 0 };\
        rule r(x) :- s(x) and { let g = noisy; g(x) };";
    assert!(matches!(
        check_rules_src(alias),
        Err(RuleError::Impure { ref witness, .. }) if witness == &["g".to_string()]
    ));
    // the function handed to `fold` must be a named function, a pure native,
    // or a lambda — a global closure is none of these
    let handed = "relation s : (Int); relation r : (Int);\
        let noisy = |a: Bool, y: Int| { println(\"e\"); a };\
        rule r(x) :- s(x) and fold([x], true, noisy);";
    assert!(matches!(
        check_rules_src(handed),
        Err(RuleError::Impure { ref witness, .. }) if witness == &["noisy".to_string()]
    ));
    // while a pure native as the callback is fine
    let native = "relation s : ([Int]); relation r : ([Int]);\
        rule r(x) :- s(x) and contains(map(x, abs), 2);\
        fn main() -> [[Int]] { add s([1, -2]); add s([3]); solutions r(?x) }";
    assert!(check_rules_src(native).is_ok());
    assert_eq!(
        run_typed(native).0,
        vlist(vec![vlist(vec![Value::Int(1), Value::Int(-2)])])
    );
}

#[test]
fn return_is_control_and_a_function_using_it_is_pure() {
    let src = "relation s : (Int); relation r : (Int);\
        fn pos(x: Int) -> Bool { if x > 0 { return true; } false }\
        rule r(x) :- s(x) and pos(x);\
        fn main() -> [Int] { add s(1); add s(-1); solutions r(?x) }";
    assert!(check_rules_src(src).is_ok());
    assert_eq!(run_typed(src).0, vlist(vec![Value::Int(1)]));
    // directly in a filter there is no function to return from: a static
    // error from M5, and a stuck state for the unchecked engine
    use bridger::interp::RuntimeError;
    let bare = "relation s : (Int); relation r : (Int);\
        rule r(x) :- s(x) and { return true; };\
        fn main() -> [Int] { add s(1); solutions r(?x) }";
    assert!(matches!(
        run_stuck(bare),
        RuntimeError::ReturnOutsideFunction { .. }
    ));
}

#[test]
fn a_query_form_on_a_name_that_is_not_a_relation_says_so() {
    use bridger::types::{Checker, TyError};
    fn check_program(src: &str) -> Result<(), TyError> {
        let mut it = Interpreter::new();
        it.load_program(src).expect("parse");
        Checker::check_program(it.program()).map(|_| ())
    }
    for src in [
        "fn foo(x: Int) -> Bool = true; fn main() -> [()] = solutions foo(1);",
        "fn main() -> [()] = solutions abs(1);",
        "let k = 1; fn main() -> () = clear k;",
    ] {
        assert!(
            matches!(check_program(src), Err(TyError::NotARelation { .. })),
            "{src}"
        );
    }
    assert!(matches!(
        check_program("fn main() -> [()] = solutions nope(1);"),
        Err(TyError::UndeclaredRelation { .. })
    ));
}

#[test]
fn unchecked_stuck_states_in_rules_and_initializers() {
    use bridger::interp::RuntimeError;
    // a filter that is not a boolean is stuck, not silently false
    let src = "relation b : (Int); relation r : (Int);\
        rule b(1); rule r(x) :- b(x) and x;\
        fn main() -> [Int] = solutions r(?x);";
    assert!(matches!(run_stuck(src), RuntimeError::TypeError { .. }));
    // a `return` escaping a global initializer has no function to return from
    let init = "let q = { return 5 }; fn main() -> Int = q;";
    assert!(matches!(
        run_stuck(init),
        RuntimeError::ReturnOutsideFunction { .. }
    ));
}

#[test]
fn transitive_closure_over_a_long_chain_is_computed_once_and_reused() {
    // 150 edges in a chain: 150·151/2 paths. Each boolean query after the
    // first reads the closure the database remembers.
    let src = "relation edge : (Int, Int); relation path : (Int, Int);\
        rule path(a, b) :- edge(a, b);\
        rule path(a, c) :- edge(a, b) and path(b, c);\
        fn main() -> (Int, Int) {\
            for i in range(0, 150) { add edge(i, i + 1); }\
            ref hits = 0;\
            for i in range(0, 300) { if path(0, i) { hits := deref hits + 1; } }\
            (len(solutions path(?x, ?y)), deref hits) }";
    assert_eq!(
        run_typed(src).0,
        vtuple(vec![Value::Int(150 * 151 / 2), Value::Int(150)])
    );
}

#[test]
fn a_rules_mistake_is_reported_as_the_rules_before_any_typing() {
    // Plumbing: the help text is the reference's, not a student's; the payload
    // it reads is the graded part.
    use bridger::interp::{Interpreter, RunError};
    // an undeclared head relation, and a filter that only tests: both are
    // rule errors, reported before the type checker would blame a variable
    for (src, want) in [
        (
            "relation edge : (Int, Int); rule path(a, b) :- edge(a, b); fn main() -> () = ();",
            "NotARelation",
        ),
        (
            "relation w : (Int, Int, Int); relation d : (Int, Int, Int);\
             rule d(a, b, s) :- w(a, b, x) and s == x + 1; fn main() -> () = ();",
            "Unbound",
        ),
    ] {
        let mut it = Interpreter::new();
        it.load_program(src).expect("parse");
        let err = it.check().unwrap_err();
        let RunError::Rule(rule_err) = &err else {
            panic!("{src}: {err:?}");
        };
        let name = format!("{rule_err:?}");
        assert!(name.starts_with(want), "{name}");
        assert!(err.help().is_some());
    }
}

#[test]
fn an_undeclared_name_called_in_a_rule_body_is_reported_as_such() {
    let src = "relation s : (Int); relation r : (Int);\
        rule r(x) :- s(x) and is_big(x); fn main() -> () = ();";
    assert!(matches!(
        check_rules_src(src),
        Err(RuleError::NotARelation { ref name, .. }) if name == "is_big"
    ));
    let src = "relation r : (Int); rule r(x) :- bar(x); fn main() -> () = ();";
    assert!(matches!(
        check_rules_src(src),
        Err(RuleError::NotARelation { ref name, .. }) if name == "bar"
    ));
}

#[test]
fn a_later_load_invalidates_the_remembered_closure() {
    use bridger::interp::Interpreter;
    // `main` queries `p` and remembers its closure; a further load adds a
    // rule, and the next run must see it
    let mut it = Interpreter::new();
    it.load_program(
        "relation e : (Int, Int); relation p : (Int, Int);\
         rule p(x, y) :- e(x, y);\
         fn main() -> Int { add e(1, 2); add e(2, 3); len(solutions p(?x, ?y)) }",
    )
    .expect("parse");
    assert_eq!(it.eval_program().expect("run"), Value::Int(2));
    it.load_program("rule p(x, z) :- e(x, y) and p(y, z);")
        .expect("parse");
    assert_eq!(it.eval_program().expect("run again"), Value::Int(3));
}

// ---- seventh and eighth review rounds ----

#[test]
fn a_hole_in_add_is_its_own_stuck_state_unchecked() {
    assert!(matches!(
        run_stuck("relation r : (Int, Int); fn main() -> () = add r(1, ?x);"),
        bridger::interp::RuntimeError::HoleInAdd { .. }
    ));
}

#[test]
fn a_relation_handed_to_a_callback_native_is_not_a_function() {
    // unchecked, a relation value reaches `map`; it is refused as a callee
    // rather than looked up afresh by name
    assert!(matches!(
        run_stuck("relation edge : (Int); fn main() -> [Int] = map([1], edge);"),
        bridger::interp::RuntimeError::NotAFunction { .. }
    ));
}

#[test]
fn the_earliest_error_is_reported_across_the_static_stages() {
    use bridger::interp::RunError;
    // a type error on line 1 comes before a rule error on line 3, whichever
    // stage runs first
    let src = "fn f() -> Int = \"a\";\nrelation r : (Int); relation s : (Int);\nrule s(y) :- r(x);\nfn main() {}";
    let mut it = Interpreter::new();
    it.load_program(src).expect("parse");
    assert!(matches!(it.check(), Err(RunError::Type(_))));
    // and the other way round
    let src = "relation r : (Int); relation s : (Int);\nrule s(y) :- r(x);\nfn f() -> Int = \"a\";\nfn main() {}";
    let mut it = Interpreter::new();
    it.load_program(src).expect("parse");
    assert!(matches!(it.check(), Err(RunError::Rule(_))));
}

#[test]
fn a_rule_is_judged_in_place_and_blames_the_offending_term() {
    use bridger::types::TyError;
    fn check_program(src: &str) -> Result<(), TyError> {
        let mut it = Interpreter::new();
        it.load_program(src).expect("parse");
        Checker::check_program(it.program()).map(|_| ())
    }
    // a bound the rule fails is its error, reported before a later
    // declaration's
    let src = "relation nums : (Int); relation big : (Int); fn inc(n: Int) -> Int = n + 1;\
        rule big(x) :- nums(x) and to_string([inc]) == \"a\";\
        fn g(x: Int) -> Int = true; fn main() -> () = ();";
    assert!(matches!(
        check_program(src),
        Err(TyError::NotPrintable { .. })
    ));
    // a mismatch is at the head atom or the generator argument, not `rule`
    let src = "relation r : (Int, String); relation s : (String, Int);\
        rule s(x, y) :- r(x, y); fn main() -> Int = 1;";
    let err = check_program(src).unwrap_err();
    assert!(matches!(err, TyError::Mismatch { .. }), "{err:?}");
    assert_eq!(&src[err.span().start..err.span().end], "s(x, y)");
    let src = "relation r : (Int); relation s : (Int); rule s(x) :- r(\"a\") and x == 1;\
        fn main() -> Int = 1;";
    let err = check_program(src).unwrap_err();
    assert_eq!(&src[err.span().start..err.span().end], "\"a\"");
}

// ---- eleventh review round: relations sentences with no test ----

#[test]
fn equality_on_a_relation_value_is_stuck_unchecked() {
    assert!(matches!(
        run_stuck("relation r : (Int); fn main() -> Bool = r == r;"),
        bridger::interp::RuntimeError::NotComparable { .. }
    ));
}

#[test]
fn clear_leaves_a_bodiless_rules_fact() {
    let src = "relation r : (Int); rule r(7);\
        fn main() -> [Int] { add r(1); clear r; solutions r(?x) }";
    assert_eq!(run_program(src).0, vlist(vec![Value::Int(7)]));
}

#[test]
fn query_arguments_evaluate_once_per_form() {
    let src = "relation r : (Int, Int);\
        fn main() -> Int { add r(1, 1); add r(2, 1); ref n = 0;\
            for r(?x, { n := deref n + 1; 1 }) { () }; deref n }";
    assert_eq!(run_program(src).0, Value::Int(1));
}

#[test]
fn a_global_holding_solutions_keeps_the_list_as_it_was() {
    let src = "relation r : (Int); let g = solutions r(?x);\
        fn main() -> [Int] { add r(1); g }";
    assert_eq!(run_program(src).0, vlist(vec![]));
}

#[test]
fn an_anonymous_hole_binds_nothing() {
    let src = "relation r : (Int);\
        fn main() -> ([()], Int) { add r(1); add r(2); ref n = 0;\
            for r(?) { n := deref n + 1 }; (solutions r(?), deref n) }";
    assert_eq!(
        run_program(src).0,
        vtuple(vec![vlist(vec![Value::Unit]), Value::Int(1)])
    );
}

#[test]
fn booleans_order_false_before_true_in_solutions() {
    let src =
        "relation b : (Bool); fn main() -> [Bool] { add b(true); add b(false); solutions b(?x) }";
    assert_eq!(
        run_program(src).0,
        vlist(vec![Value::Bool(false), Value::Bool(true)])
    );
}

#[test]
fn a_return_inside_a_relational_for_abandons_the_loop() {
    let src = "relation r : (Int);\
        fn first() -> Int { for r(?x) { return x }; 0 }\
        fn main() -> Int { add r(5); add r(3); first() }";
    assert_eq!(run_program(src).0, Value::Int(3));
}

#[test]
fn a_return_in_a_rule_body_is_a_static_error() {
    use bridger::types::TyError;
    let mut it = Interpreter::new();
    it.load_program(
        "relation r : (Int); relation s : (Int); rule s(x) :- r(x) and { return true; };",
    )
    .expect("parse");
    assert!(matches!(
        Checker::check_program(it.program()),
        Err(TyError::ReturnOutsideFunction { .. })
    ));
}

#[test]
fn every_never_pure_form_makes_a_filter_impure() {
    for filter in [
        "{ let c = ref x; true }",
        "{ let c = ref 1; c := x; true }",
        "{ while false { () }; true }",
        "{ for y in [x] { () }; true }",
        "{ clear r; true }",
        "len(solutions r(?y)) > 0",
        "read_int() == x",
    ] {
        let src =
            format!("relation r : (Int); relation s : (Int); rule s(x) :- r(x) and {filter};");
        assert!(
            matches!(check_rules_src(&src), Err(RuleError::Impure { .. })),
            "{filter}"
        );
    }
}

#[test]
fn a_rule_atoms_name_is_the_relation_whatever_variable_shares_it() {
    let src = "relation e : (Int, Int); relation r : (Int); relation s : (Int);\
        rule s(e) :- r(e) and e(e, 3);\
        fn main() -> [Int] { add r(2); add e(2, 3); solutions s(?x) }";
    assert_eq!(run_program(src).0, vlist(vec![Value::Int(2)]));
}

#[test]
fn a_computed_generator_argument_is_not_a_term() {
    assert!(matches!(
        check_rules_src("relation r : (Int); relation s : (Int); rule s(x) :- r(x + 1);"),
        Err(RuleError::ArgumentNotATerm { .. })
    ));
}

#[test]
fn to_string_is_an_effect_for_the_purity_judgment() {
    // a reference prints as the value it holds, so `to_string` reads the
    // store — directly, or through any helper a filter reaches
    for src in [
        "relation r : (Int); relation s : (Int); rule s(x) :- r(x) and to_string(x) == \"1\";",
        "relation r : (Int); relation s : (Int); fn show(x: Int) -> String = to_string(x);\
         rule s(x) :- r(x) and show(x) == \"1\";",
        "relation r : (Int); relation s : (Int);\
         rule s(x) :- r(x) and contains(map([x], to_string), \"1\");",
    ] {
        assert!(
            matches!(check_rules_src(src), Err(RuleError::Impure { ref witness, .. })
                if witness.last().is_some_and(|w| w == "to_string")),
            "{src}"
        );
    }
}

// ---- twelfth review round ----

#[test]
fn a_local_named_like_a_function_is_a_local_to_the_purity_judgment() {
    // a call through it is a call through a local, whatever the function of
    // that name does
    let src = "relation i : (Int); relation a : (Int);\
        fn sq(x: Int) -> Int = x * x; let g = |x: Int| { println(\"boom\"); x };\
        rule a(x) :- i(x) and { let sq = g; sq(x) > 0 };";
    assert!(matches!(
        check_rules_src(src),
        Err(RuleError::Impure { ref witness, .. }) if witness == &["sq".to_string()]
    ));
    // and a variable named like an impure function is just a variable
    let src = "relation i : (Int); relation a : (Int);\
        fn noisy() -> Int { println(\"x\"); 1 }\
        rule a(noisy) :- i(noisy) and noisy > 0;";
    assert!(check_rules_src(src).is_ok());
}

#[test]
fn a_filter_may_not_observe_a_reference_through_to_string() {
    // the case that motivates the rule: the memoized closure would go stale
    // after a `:=` no `add` or `clear` follows
    let src = "relation c : (ref<Int>); relation five : (ref<Int>);\
        fn show<T: Print>(x: T) -> String = to_string(x);\
        rule five(r) :- c(r) and show(r) == \"ref(5)\";";
    assert!(matches!(
        check_rules_src(src),
        Err(RuleError::Impure { ref witness, .. })
            if witness == &["show".to_string(), "to_string".to_string()]
    ));
}

// ---- fifteenth review round: rule-check contracts the suite left open ----

#[test]
fn a_rule_heads_arity_must_match_its_relation() {
    // the head's own arity, not only a generator conjunct's, is checked.
    let src = "relation edge : (Int, Int); relation p : (Int);\
        rule p(x, y) :- edge(x, y);";
    assert!(matches!(check_rules_src(src), Err(RuleError::Arity { .. })));
}

#[test]
fn a_filter_whose_callback_is_a_local_cannot_be_followed_and_is_impure() {
    // a callback the analysis cannot follow — a local, not a lambda literal,
    // a named function, or a pure native — makes the filter impure.
    let src = "relation s : (Int); relation r : (Int);\
        rule r(x) :- s(x) and { let cb = |y: Int| { println(\"e\"); y > 0 };\
        filter([x], cb) != [] };";
    assert!(matches!(
        check_rules_src(src),
        Err(RuleError::Impure { .. })
    ));
}

#[test]
fn a_filter_may_not_use_a_variable_no_generator_binds() {
    // range restriction covers a filter's free variables, not only the head's.
    let src = "relation s : (Int); relation r : (Int); rule r(x) :- s(x) and y > 0;";
    assert!(matches!(
        check_rules_src(src),
        Err(RuleError::Unbound { .. })
    ));
}
