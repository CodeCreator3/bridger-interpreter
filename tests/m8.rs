//! Milestone M8 — objects (public subset).  [frozen — do not edit]
//!
//! Struct literals, field access, and method calls dispatched on the receiver's
//! runtime head type. Whole-program tests run through `eval_program`; type
//! checks go through `Checker::check_program`.
//!
//! Gated on the `m8` feature: compiled at every milestone from M8 on.

#![cfg(feature = "m8")]

mod common;

use bridger::interp::{Interpreter, RuntimeError, Value};
use bridger::types::{Checker, TyError};
use common::*;
use std::rc::Rc;

fn vstruct(name: &str, fields: Vec<(&str, Value)>) -> Value {
    Value::Struct(
        name.to_string(),
        Rc::from(
            fields
                .into_iter()
                .map(|(f, v)| (f.to_string(), v))
                .collect::<Vec<_>>(),
        ),
    )
}
fn check_program(src: &str) -> Result<(), TyError> {
    let mut it = Interpreter::new();
    it.load_program(src).expect("program should parse");
    Checker::check_program(it.program()).map(drop)
}

const POINT: &str = "struct Point { x: Int, y: Int }";

// ---- struct literals and field access ----

#[test]
fn a_struct_literal_builds_a_value() {
    let src = format!("{POINT} fn main() -> Point = Point {{ x: 1, y: 2 }};");
    assert_eq!(
        run_program(&src).0,
        vstruct("Point", vec![("x", Value::Int(1)), ("y", Value::Int(2))])
    );
}

#[test]
fn field_access_reads_a_field() {
    let src = format!("{POINT} fn main() -> Int {{ let p = Point {{ x: 3, y: 4 }}; p.x + p.y }}");
    assert_eq!(run_program(&src).0, Value::Int(7));
}

// ---- methods ----

#[test]
fn a_method_runs_with_self_bound() {
    let src = format!(
        "{POINT} impl Point {{ fn sum(self) -> Int = self.x + self.y; }}\
         fn main() -> Int = Point {{ x: 3, y: 4 }}.sum();"
    );
    assert_eq!(run_program(&src).0, Value::Int(7));
}

#[test]
fn a_method_takes_arguments() {
    let src = format!(
        "{POINT} impl Point {{ fn scale(self, k: Int) -> Point =\
             Point {{ x: self.x * k, y: self.y * k }}; }}\
         fn main() -> Point = Point {{ x: 1, y: 2 }}.scale(3);"
    );
    assert_eq!(
        run_program(&src).0,
        vstruct("Point", vec![("x", Value::Int(3)), ("y", Value::Int(6))])
    );
}

#[test]
fn dispatch_is_by_the_receivers_runtime_type() {
    let src = "struct Dog { name: String }\
        struct Cat { name: String }\
        impl Dog { fn speak(self) -> String = \"woof\"; }\
        impl Cat { fn speak(self) -> String = \"meow\"; }\
        fn main() -> [String] {\
            let d = Dog { name: \"rex\" };\
            let c = Cat { name: \"felix\" };\
            [d.speak(), c.speak()]\
        }";
    assert_eq!(run_program(src).0, vlist(vec![vstr("woof"), vstr("meow")]));
}

#[test]
fn a_missing_method_is_stuck() {
    use bridger::interp::RuntimeError;
    let src = format!("{POINT} fn main() -> Int = Point {{ x: 1, y: 2 }}.nope();");
    assert!(matches!(run_stuck(&src), RuntimeError::NoSuchMethod { .. }));
}

#[test]
fn a_struct_prints_in_canonical_form() {
    let src = format!("{POINT} fn main() -> () {{ print(Point {{ x: 1, y: 2 }}); }}");
    assert_eq!(run_program(&src).1, "Point { x: 1, y: 2 }");
}

// ---- type checking ----

#[test]
fn a_well_typed_struct_program_checks() {
    let src = format!(
        "{POINT} impl Point {{ fn sum(self) -> Int = self.x + self.y; }}\
         fn main() -> Int = Point {{ x: 1, y: 2 }}.sum();"
    );
    assert!(check_program(&src).is_ok());
}

#[test]
fn a_missing_or_ill_typed_field_is_rejected() {
    let missing = format!("{POINT} fn main() -> Point = Point {{ x: 1 }};");
    assert!(matches!(
        check_program(&missing),
        Err(TyError::MissingField { .. })
    ));
    let wrong = format!("{POINT} fn main() -> Point = Point {{ x: true, y: 2 }};");
    assert!(matches!(
        check_program(&wrong),
        Err(TyError::Mismatch { .. })
    ));
}

#[test]
fn a_bound_requires_an_impl_and_resolves_the_method() {
    // `describe` calls `x.show()` through the `Show` bound; the call must supply
    // a type that implements `Show`.
    let ok = "trait Show { fn show(self) -> String; }\
        struct Point { x: Int, y: Int }\
        impl Show for Point { fn show(self) -> String = \"pt\"; }\
        fn describe<T: Show>(x: T) -> String = x.show();\
        fn main() -> String = describe(Point { x: 1, y: 2 });";
    assert!(check_program(ok).is_ok());

    // `Bare` has no `impl Show`, so the bound is unsatisfied at the call.
    let missing = "trait Show { fn show(self) -> String; }\
        struct Bare { n: Int }\
        fn describe<T: Show>(x: T) -> String = x.show();\
        fn main() -> String = describe(Bare { n: 1 });";
    assert!(matches!(
        check_program(missing),
        Err(TyError::UnsatisfiedBound { .. })
    ));
}

#[test]
fn try_converts_the_error_through_from() {
    // `f` fails with `Small`; `g` returns `Result<_, Big>`, so `f(x)?` must
    // convert the error through `impl From<Small> for Big` (M8 elaboration).
    let src = "\
        struct Small { code: Int }\
        struct Big { code: Int }\
        impl From<Small> for Big { fn from(s: Small) -> Big = Big { code: s.code + 100 }; }\
        fn f(x: Int) -> Result<Int, Small> = if x > 0 { Ok(x) } else { Err(Small { code: 1 }) };\
        fn g(x: Int) -> Result<Int, Big> { let y = f(x)?; Ok(y) }\
        fn main() -> Result<Int, Big> = g(-1);";
    // `run_typed` type-checks (installing the conversion) and then evaluates.
    let (v, _) = run_typed(src);
    assert_eq!(
        v,
        Value::Ctor(
            "Err".to_string(),
            Rc::from([vstruct("Big", vec![("code", Value::Int(101))])])
        )
    );
}

#[test]
fn an_unknown_field_or_method_is_rejected() {
    let field = format!("{POINT} fn main() -> Int {{ let p = Point {{ x: 1, y: 2 }}; p.z }}");
    assert!(matches!(
        check_program(&field).map_err(|e| matches!(e, TyError::NoSuchField { .. })),
        Err(true)
    ));
    let method = format!("{POINT} fn main() -> Int = Point {{ x: 1, y: 2 }}.nope();");
    assert!(matches!(
        check_program(&method).map_err(|e| matches!(e, TyError::NoSuchMethod { .. })),
        Err(true)
    ));
}

#[test]
fn self_in_a_method_signature_is_the_implemented_type() {
    let src = format!(
        "{POINT} impl Point {{\
            fn shift(self, by: Self) -> Self = Point {{ x: self.x + by.x, y: self.y + by.y }};\
            fn same(self, o: Self) -> Bool = self.x == o.x and self.y == o.y; }}\
         fn main() -> Bool =\
            Point {{ x: 1, y: 2 }}.shift(Point {{ x: 2, y: 1 }}).same(Point {{ x: 3, y: 3 }});"
    );
    assert!(check_program(&src).is_ok());
    assert_eq!(run_typed(&src).0, Value::Bool(true));
}

#[test]
fn the_preludes_ord_trait_can_be_implemented() {
    let src = format!(
        "{POINT} impl Ord for Point {{ fn cmp(self, other: Self) -> Ordering =\
            if self.x < other.x {{ Less }} else if self.x == other.x {{ Equal }} else {{ Greater }}; }}\
         fn main() -> Ordering = Point {{ x: 1, y: 0 }}.cmp(Point {{ x: 2, y: 0 }});"
    );
    assert!(check_program(&src).is_ok());
    assert_eq!(run_typed(&src).0, vctor("Less", vec![]));
}

#[test]
fn self_in_a_body_annotation_is_the_implemented_type() {
    let src = format!(
        "{POINT} impl Point {{\
            fn twice(self) -> Self {{\
                let dbl: Self = Point {{ x: self.x * 2, y: self.y * 2 }};\
                let same = |o: Self| o.x == dbl.x;\
                if same(dbl) {{ dbl }} else {{ self }} }} }}\
         fn main() -> Int = Point {{ x: 2, y: 3 }}.twice().x;"
    );
    assert!(check_program(&src).is_ok());
    assert_eq!(run_typed(&src).0, Value::Int(4));
}

#[test]
fn self_outside_an_impl_names_no_type() {
    let src = format!("{POINT} fn main() -> Int {{ let p: Self = 1; p }}");
    assert!(matches!(
        check_program(&src),
        Err(TyError::UnknownType { .. })
    ));
}

const ORD_P: &str = "struct P { v: Int }\
    impl Ord for P { fn cmp(self, other: Self) -> Ordering =\
        if self.v < other.v { Less } else if self.v == other.v { Equal } else { Greater }; }";

#[test]
fn a_user_impl_ord_flows_through_min_and_max() {
    let src = format!(
        "{ORD_P} fn main() -> (Int, Int) =\
        (min(P {{ v: 2 }}, P {{ v: 1 }}).v, max(P {{ v: 2 }}, P {{ v: 1 }}).v);"
    );
    assert!(check_program(&src).is_ok());
    assert_eq!(
        run_typed(&src).0,
        vtuple(vec![Value::Int(1), Value::Int(2)])
    );
}

#[test]
fn a_user_impl_ord_flows_through_minimum_and_maximum() {
    let src = format!(
        "{ORD_P} fn main() -> (Int, Int) {{\
        let xs = [P {{ v: 3 }}, P {{ v: 1 }}, P {{ v: 2 }}];\
        let lo = match minimum(xs) {{ Some(p) => p.v, None => 0 }};\
        let hi = match maximum(xs) {{ Some(p) => p.v, None => 0 }};\
        (lo, hi) }}"
    );
    assert!(check_program(&src).is_ok());
    assert_eq!(
        run_typed(&src).0,
        vtuple(vec![Value::Int(1), Value::Int(3)])
    );
}

#[test]
fn a_user_impl_len_flows_through_len() {
    let src = "struct Bag { items: [Int] }\
        impl Len for Bag { fn length(self) -> Int = len(self.items) * 10; }\
        fn main() -> Int = len(Bag { items: [1, 2, 3] });";
    assert!(check_program(src).is_ok());
    assert_eq!(run_typed(src).0, Value::Int(30));
}

#[test]
fn built_in_types_conform_to_the_prelude_traits() {
    let src =
        "fn main() -> (Ordering, Int, Ordering) = (1.cmp(2), \"abc\".length(), \"b\".cmp(\"a\"));";
    assert!(check_program(src).is_ok());
    assert_eq!(
        run_typed(src).0,
        vtuple(vec![
            vctor("Less", vec![]),
            Value::Int(3),
            vctor("Greater", vec![])
        ])
    );
}

#[test]
fn the_bounded_prelude_still_serves_the_built_in_types() {
    let src = "fn main() -> (Int, String, Option<Int>, Option<Int>, Int) =\
        (min(3, 7), max(\"a\", \"b\"), minimum([4, 2, 9]), maximum([]), len(\"héllo\"));";
    assert!(check_program(src).is_ok());
    assert_eq!(
        run_typed(src).0,
        vtuple(vec![
            Value::Int(3),
            vstr("b"),
            vctor("Some", vec![Value::Int(2)]),
            vctor("None", vec![]),
            Value::Int(5),
        ])
    );
}

#[test]
fn a_mixed_comparison_is_still_not_ordered() {
    // `min` runs `1.cmp("x")`, the built-in conformance, which has no order
    // over two types
    let err = run_stuck("fn main() -> Int = min(1, \"x\");");
    assert!(matches!(err, RuntimeError::NotOrdered { .. }));
}

#[test]
fn a_bounded_parameter_satisfies_its_own_bound() {
    // `T: Ord` in scope discharges `minimum`'s `Ord` obligation on `T`
    let src = "fn lo<T: Ord>(xs: [T]) -> Option<T> = minimum(xs);\
        fn main() -> Option<Int> = lo([3, 1, 2]);";
    assert!(check_program(src).is_ok());
    assert_eq!(run_typed(src).0, vctor("Some", vec![Value::Int(1)]));
    // without the bound, the obligation is unsatisfied
    let unbounded = "fn lo<T>(xs: [T]) -> Option<T> = minimum(xs); fn main() -> () = ();";
    assert!(matches!(
        check_program(unbounded),
        Err(TyError::UnsatisfiedBound { .. })
    ));
}

fn check_rules_src(src: &str) -> Result<(), bridger::relations::RuleError> {
    let mut it = Interpreter::new();
    it.load_program(src).expect("parse");
    bridger::relations::check_rules(it.program())?;
    bridger::relations::check_guards(it.program())
}

#[test]
fn a_method_call_in_a_filter_is_pure_when_every_impl_is() {
    let src = "relation r : (Int); relation s : (Int);\
        struct A { v: Int } struct B { v: Int }\
        impl A { fn big(self) -> Bool = self.v > 10; }\
        impl B { fn big(self) -> Bool = self.v > 100; }\
        rule s(x) :- r(x) and A { v: x }.big();";
    assert!(check_rules_src(src).is_ok());
}

#[test]
fn a_method_call_in_a_filter_is_impure_when_any_impl_is() {
    use bridger::relations::RuleError;
    // the receiver is a call's result, which the declarations do not settle,
    // so the call reaches every impl's `big`
    let src = "relation r : (Int); relation s : (Int);\
        struct A { v: Int } struct B { v: Int }\
        impl A { fn big(self) -> Bool = self.v > 10; }\
        impl B { fn big(self) -> Bool { print(self.v); self.v > 100 } }\
        fn id(a: A) -> A = a;\
        rule s(x) :- r(x) and id(A { v: x }).big();";
    assert!(matches!(
        check_rules_src(src),
        Err(RuleError::Impure { ref witness, .. }) if witness.first().is_some_and(|w| w == ".big")
    ));
}

#[test]
fn a_built_in_method_in_a_filter_is_pure() {
    let src = "relation r : (Int, Int); relation s : (Int);\
        rule s(x) :- r(x, y) and x.cmp(y) == Less;";
    assert!(check_rules_src(src).is_ok());
}

#[test]
fn a_struct_literal_or_pattern_names_each_field_once() {
    let literal = format!("{POINT} fn main() -> Point = Point {{ x: 1, x: 2 }};");
    assert!(matches!(
        check_program(&literal),
        Err(TyError::DuplicateField { .. })
    ));
    let pattern = format!(
        "{POINT} fn main() -> Int {{ let p = Point {{ x: 1, y: 2 }};\
         match p {{ Point {{ x: a, x: b }} => a + b }} }}"
    );
    assert!(matches!(
        check_program(&pattern),
        Err(TyError::DuplicateField { .. })
    ));
}

#[test]
fn a_struct_pattern_completes_its_match() {
    let src = format!(
        "{POINT} fn m(p: Point) -> Int = match p {{ Point {{ x: 0 }} => 0, Point {{ x: n }} => n }};\
         fn main() -> Int = m(Point {{ x: 3, y: 4 }});"
    );
    assert!(check_program(&src).is_ok());
    let gap = format!(
        "{POINT} fn m(p: Point) -> Int = match p {{ Point {{ x: 0 }} => 0 }};\
         fn main() -> Int = m(Point {{ x: 3, y: 4 }});"
    );
    assert!(matches!(
        check_program(&gap),
        Err(TyError::NonExhaustiveMatch { .. })
    ));
}

const SHOW: &str =
    "struct P { v: Int } trait Show { fn show(self) -> String; fn twice(self) -> String; }";

#[test]
fn an_impl_must_provide_every_trait_method() {
    // `twice` is missing — and a bounded call to it would otherwise get stuck
    let src = format!(
        "{SHOW} impl Show for P {{ fn show(self) -> String = \"p\"; }}\
         fn d<T: Show>(v: T) -> String = v.twice(); fn main() -> String = d(P {{ v: 1 }});"
    );
    assert!(matches!(
        check_program(&src),
        Err(TyError::MissingMethod { .. })
    ));
}

#[test]
fn an_impl_method_must_have_the_traits_signature() {
    let wrong_ret = "struct P { v: Int } impl Ord for P { fn cmp(self, o: Self) -> Int = 1; }\
                     fn main() -> () = ();";
    assert!(matches!(
        check_program(wrong_ret),
        Err(TyError::SignatureMismatch { .. })
    ));
    let wrong_arity = "struct P { v: Int } impl Ord for P { fn cmp(self) -> Ordering = Less; }\
                       fn main() -> () = ();";
    assert!(matches!(
        check_program(wrong_arity),
        Err(TyError::SignatureMismatch { .. })
    ));
    // the trait's type argument is substituted: `From<Small>` wants `from(x: Small)`
    let from_ok = "struct A { v: Int } struct B { v: Int }\
                   impl From<A> for B { fn from(x: A) -> Self = B { v: x.v }; } fn main() -> () = ();";
    assert!(check_program(from_ok).is_ok());
    let from_wrong = "struct A { v: Int } struct B { v: Int }\
                      impl From<A> for B { fn from(x: B) -> Self = x; } fn main() -> () = ();";
    assert!(matches!(
        check_program(from_wrong),
        Err(TyError::SignatureMismatch { .. })
    ));
}

#[test]
fn an_impl_may_not_add_methods_the_trait_lacks() {
    let src = format!(
        "{SHOW} impl Show for P {{ fn show(self) -> String = \"p\"; fn twice(self) -> String = \"pp\";\
             fn thrice(self) -> String = \"ppp\"; }} fn main() -> () = ();"
    );
    assert!(matches!(
        check_program(&src),
        Err(TyError::NotInTrait { .. })
    ));
}

#[test]
fn impls_are_coherent() {
    let twice = "struct P { v: Int }\
        impl Ord for P { fn cmp(self, o: Self) -> Ordering = Less; }\
        impl Ord for P { fn cmp(self, o: Self) -> Ordering = Greater; } fn main() -> () = ();";
    assert!(matches!(
        check_program(twice),
        Err(TyError::DuplicateImpl { .. })
    ));
    let same_method = "struct P { v: Int }\
        impl P { fn get(self) -> Int = self.v; } impl P { fn get(self) -> Int = 0; }\
        fn main() -> () = ();";
    assert!(matches!(
        check_program(same_method),
        Err(TyError::DuplicateMethod { .. })
    ));
    // two impl blocks with distinct methods are fine
    let split = "struct P { v: Int }\
        impl P { fn get(self) -> Int = self.v; } impl P { fn zero(self) -> Int = 0; }\
        fn main() -> Int = P { v: 3 }.get() + P { v: 3 }.zero();";
    assert!(check_program(split).is_ok());
    assert_eq!(run_typed(split).0, Value::Int(3));
}

#[test]
fn an_impl_names_a_declared_trait() {
    let src =
        "struct P { v: Int } impl Nope for P { fn f(self) -> Int = 1; } fn main() -> () = ();";
    assert!(matches!(
        check_program(src),
        Err(TyError::UnknownTrait { .. })
    ));
}

#[test]
fn built_in_conformances_cannot_be_redefined() {
    // `Int` is `Ord` by the prelude; a second `impl` is a duplicate
    let ord_int =
        "impl Ord for Int { fn cmp(self, o: Self) -> Ordering = Greater; } fn main() -> () = ();";
    assert!(matches!(
        check_program(ord_int),
        Err(TyError::BuiltinImpl { .. })
    ));
    // nor may an inherent impl redefine the built-in method
    let cmp_int = "impl Int { fn cmp(self, o: Int) -> Ordering = Less; } fn main() -> () = ();";
    assert!(matches!(
        check_program(cmp_int),
        Err(TyError::DuplicateMethod { .. })
    ));
    // other methods on a built-in type are fine
    let double = "impl Int { fn double(self) -> Int = self * 2; } fn main() -> Int = 3.double();";
    assert!(check_program(double).is_ok());
    assert_eq!(run_typed(double).0, Value::Int(6));
}

#[test]
fn a_user_trait_may_mention_self() {
    let src = "struct P { v: Int } trait Same { fn same(self, other: Self) -> Bool; }\
        impl Same for P { fn same(self, other: Self) -> Bool = self.v == other.v; }\
        fn main() -> Bool = P { v: 1 }.same(P { v: 1 });";
    assert!(check_program(src).is_ok());
    assert_eq!(run_typed(src).0, Value::Bool(true));
}

#[test]
fn a_struct_value_does_not_depend_on_literal_order() {
    // Fields are stored in declaration order whatever order the literal
    // wrote them, so `==`, `contains`, and printing agree.
    let src = "struct P { x: Int, y: Int }\
        fn main() -> (Bool, Bool) {\
            let a = P { x: 1, y: 2 };\
            let b = P { y: 2, x: 1 };\
            println(b);\
            (a == b, contains([a], b))\
        }";
    let (v, out) = run_program(src);
    assert_eq!(v, vtuple(vec![Value::Bool(true), Value::Bool(true)]));
    assert_eq!(out, "P { x: 1, y: 2 }\n");
}

#[test]
fn a_struct_literal_names_every_declared_field() {
    let missing = "struct P { x: Int, y: Int } fn main() -> P = P { x: 1 };";
    assert!(matches!(
        run_stuck(missing),
        RuntimeError::MissingField { ref field, .. } if field == "y"
    ));
    let extra = "struct P { x: Int } fn main() -> P = P { x: 1, z: 2 };";
    assert!(matches!(
        run_stuck(extra),
        RuntimeError::NoSuchField { ref field, .. } if field == "z"
    ));
    let empty = "struct E {} fn main() -> () = println(E {});";
    assert_eq!(run_program(empty).1, "E {}\n");
}

#[test]
fn equality_looks_through_struct_and_type_declarations() {
    fn check(src: &str) -> Result<(), TyError> {
        let mut it = Interpreter::new();
        it.load_program(src).expect("parse");
        Checker::check_program(it.program()).map(|_| ())
    }
    // a function-typed field anywhere inside makes `==` a type error
    let field = "struct S { f: fn(Int) -> Int }\
        fn main() -> Bool { let a = S { f: |x: Int| x }; a == a }";
    assert!(matches!(check(field), Err(TyError::NoEquality { .. })));
    let nested = "struct S { f: fn(Int) -> Int } type W = Wrap(S) | Empty;\
        fn main() -> Bool = Empty == Empty;";
    assert!(matches!(check(nested), Err(TyError::NoEquality { .. })));
    // generics are instantiated: `Option<fn>` is not data, `Option<Int>` is
    let opt_fn = "fn main() -> Bool { let o = Some(|x: Int| x); o == o }";
    assert!(matches!(check(opt_fn), Err(TyError::NoEquality { .. })));
    assert!(check("fn main() -> Bool = Some(1) == Some(1);").is_ok());
    // and so is a function inside a list or a tuple
    let list = "fn main() -> Bool { let l = [|x: Int| x]; l == l }";
    assert!(matches!(check(list), Err(TyError::NoEquality { .. })));
    let pair = "fn main() -> Bool { let p = (1, |x: Int| x); p == p }";
    assert!(matches!(check(pair), Err(TyError::NoEquality { .. })));
    // a recursive type is data when its other components are
    let tree = "type Tree = Leaf | Node(Tree, Int, Tree);\
        fn main() -> Bool = Node(Leaf, 1, Leaf) == Node(Leaf, 1, Leaf);";
    assert!(check(tree).is_ok());
    // a reference compares by identity whatever it holds
    let cell = "fn main() -> Bool { let r = ref (|x: Int| x); r == r }";
    assert!(check(cell).is_ok());
}

#[test]
fn a_method_call_with_the_wrong_arity_is_stuck() {
    let src = "struct P { x: Int }\
        impl P { fn shift(self, d: Int) -> Int = self.x + d; }\
        fn main() -> Int = P { x: 1 }.shift(1, 2);";
    assert!(matches!(
        run_stuck(src),
        RuntimeError::ArityMismatch {
            expected: 1,
            found: 2,
            ..
        }
    ));
}

#[test]
fn an_impl_extends_a_head_type_generically() {
    fn check(src: &str) -> Result<(), TyError> {
        let mut it = Interpreter::new();
        it.load_program(src).expect("parse");
        Checker::check_program(it.program()).map(|_| ())
    }
    const SHOW: &str = "trait Show { fn show(self) -> String; }";
    // no head type: a bare parameter, a tuple, a function, a reference
    for target in [
        "impl<T> Show for T",
        "impl Show for (Int, Int)",
        "impl Show for fn(Int) -> Int",
        "impl Show for ref<Int>",
    ] {
        let src =
            format!("{SHOW} {target} {{ fn show(self) -> String = \"x\"; }} fn main() -> () = ();");
        assert!(
            matches!(check(&src), Err(TyError::ImplTarget { .. })),
            "{target}"
        );
    }
    // a generic head at a particular instance is fine: the impl covers only
    // receivers of that instance
    for target in [
        "impl Show for [Int]",
        "impl Show for Option<Int>",
        "impl<T> Show for Pair<T, T>",
        "impl<T> Show for Option<[T]>",
    ] {
        let src = format!(
            "{SHOW} struct Pair<A, B> {{ a: A, b: B }} {target} {{ fn show(self) -> String = \"x\"; }} fn main() -> () = ();"
        );
        assert!(check(&src).is_ok(), "{target}");
    }
    // the generic forms, and the built-in and declared heads, are fine
    let ok = format!(
        "{SHOW} struct Pair<A, B> {{ a: A, b: B }} struct P {{ v: Int }}\
         impl<T> Show for [T] {{ fn show(self) -> String = \"list\"; }}\
         impl<T> Show for Option<T> {{ fn show(self) -> String = \"option\"; }}\
         impl<A, B> Show for Pair<A, B> {{ fn show(self) -> String = \"pair\"; }}\
         impl Show for P {{ fn show(self) -> String = \"p\"; }}\
         impl Show for Int {{ fn show(self) -> String = \"int\"; }}\
         fn main() -> [String] = [[1].show(), Some(1).show(), Pair {{ a: 1, b: true }}.show(), P {{ v: 1 }}.show(), 1.show()];"
    );
    assert!(check(&ok).is_ok());
    assert_eq!(
        run_typed(&ok).0,
        vlist(
            ["list", "option", "pair", "p", "int"]
                .iter()
                .map(|s| Value::Str((*s).into()))
                .collect()
        )
    );
}

#[test]
fn an_associated_function_is_called_on_its_type() {
    const PT: &str = "struct Point { x: Int, y: Int }\
        impl Point {\
            fn origin() -> Point = Point { x: 0, y: 0 };\
            fn norm1(self) -> Int = self.x + self.y;\
        }";
    // `Type.f(args)`: no `self`, the receiver is the type
    let src = format!("{PT} fn main() -> Int = Point.origin().norm1();");
    assert_eq!(run_program(&src).0, Value::Int(0));
    assert_eq!(run_typed(&src).0, Value::Int(0));
    // an associated function is not callable on a value …
    let on_value = format!("{PT} fn main() -> Point = Point {{ x: 1, y: 1 }}.origin();");
    assert!(matches!(
        run_stuck(&on_value),
        RuntimeError::NotAMethod { ref method, .. } if method == "origin"
    ));
    let mut it = Interpreter::new();
    it.load_program(&on_value).expect("parse");
    assert!(matches!(
        Checker::check_program(it.program()),
        Err(TyError::NotAMethod { .. })
    ));
    // … and a method is not callable on the type
    let on_type = format!("{PT} fn main() -> Int = Point.norm1();");
    assert!(matches!(
        run_stuck(&on_type),
        RuntimeError::NoReceiver { ref method, .. } if method == "norm1"
    ));
    let mut it = Interpreter::new();
    it.load_program(&on_type).expect("parse");
    assert!(matches!(
        Checker::check_program(it.program()),
        Err(TyError::NoReceiver { .. })
    ));
}

#[test]
fn associated_functions_come_from_traits_and_generic_impls_too() {
    // a trait signature without `self`, implemented and called on the type
    let dflt = "struct Point { x: Int, y: Int }\
        trait Default { fn default() -> Self; }\
        impl Default for Point { fn default() -> Self = Point { x: 7, y: 0 }; }\
        fn main() -> Int = Point.default().x;";
    assert_eq!(run_typed(dflt).0, Value::Int(7));
    // a generic type's associated function, its parameter fixed by the use
    let stack = "type Stack<T> = Empty | Push(T, Stack<T>);\
        impl<T> Stack<T> { fn new() -> Self = Empty; fn push(self, x: T) -> Self = Push(x, self); }\
        fn main() -> Stack<Int> = Stack.new().push(1);";
    assert_eq!(
        run_typed(stack).0,
        Value::Ctor(
            "Push".into(),
            Rc::from(vec![
                Value::Int(1),
                Value::Ctor("Empty".into(), Rc::from(vec![]))
            ])
        )
    );
    // through a type parameter there is no type to dispatch on
    let param = "trait Default { fn default() -> Self; }\
        fn make<T: Default>() -> T = T.default();";
    let mut it = Interpreter::new();
    it.load_program(param).expect("parse");
    assert!(matches!(
        Checker::check_program(it.program()),
        Err(TyError::TypeParamCall { .. })
    ));
}

#[test]
fn eq_and_print_are_structural_bounds() {
    fn check(src: &str) -> Result<(), TyError> {
        let mut it = Interpreter::new();
        it.load_program(src).expect("parse");
        Checker::check_program(it.program()).map(|_| ())
    }
    // a bound lets a generic function compare or print its parameter
    let member = "fn member<T: Eq>(x: T, xs: [T]) -> Bool = contains(xs, x);\
        fn main() -> (Bool, Bool) = (member(2, [1, 2]), member(\"a\", []));";
    assert_eq!(
        run_typed(member).0,
        vtuple(vec![Value::Bool(true), Value::Bool(false)])
    );
    let show = "fn show<T: Print>(x: T) -> String = to_string(x);\
        fn main() -> String = show([Some(1)]);";
    assert_eq!(run_typed(show).0, Value::Str("[Some(1)]".into()));
    // `Ord` implies `Eq`, since an `impl Ord` requires equality …
    assert!(check("fn same<T: Ord>(a: T, b: T) -> Bool = a == b; fn main() -> () = ();").is_ok());
    let bad_ord = "struct S { f: fn(Int) -> Int }\
        impl Ord for S { fn cmp(self, other: Self) -> Ordering = Equal; }\
        fn main() -> () = ();";
    assert!(matches!(check(bad_ord), Err(TyError::NoEquality { .. })));
    // … and the bound is checked at the call
    assert!(matches!(
        check("fn member<T: Eq>(x: T, xs: [T]) -> Bool = contains(xs, x); fn main() -> Bool = member(abs, []);"),
        Err(TyError::NoEquality { .. })
    ));
    // the structural traits cannot be implemented
    assert!(matches!(
        check("struct S { v: Int } impl Eq for S {} fn main() -> () = ();"),
        Err(TyError::ImplTarget { .. })
    ));
}

#[test]
fn a_struct_literal_missing_a_field_names_it() {
    let mut it = Interpreter::new();
    it.load_program("struct P { x: Int, y: Int } fn main() -> P = P { x: 1 };")
        .expect("parse");
    assert!(matches!(
        Checker::check_program(it.program()),
        Err(TyError::MissingField { ref field, .. }) if field == "y"
    ));
    // and a duplicate is a duplicate on the unchecked path too
    let err = run_stuck("struct P { x: Int, y: Int } fn main() -> P = P { x: 1, x: 2, y: 3 };");
    assert!(matches!(err, RuntimeError::DuplicateField { ref field, .. } if field == "x"));
}

#[test]
fn types_print_in_bridger_syntax() {
    // the types in a mismatch read as Bridger writes them; a type still
    // being inferred prints as `_`
    let err = check_program("fn main() -> Option<Int> = [1];").unwrap_err();
    let TyError::Mismatch {
        expected, found, ..
    } = err
    else {
        panic!("{err:?}");
    };
    assert_eq!(expected.to_string(), "Option<Int>");
    assert_eq!(found.to_string(), "[Int]");
    let err = check_program("fn main() -> () = println(deref 1);").unwrap_err();
    let TyError::Mismatch {
        expected, found, ..
    } = err
    else {
        panic!("{err:?}");
    };
    assert_eq!(expected.to_string(), "ref<_>");
    assert_eq!(found.to_string(), "Int");
}

// ---- method calls and the contextual keywords ----

#[test]
fn a_method_call_on_an_unknown_receiver_needs_an_annotation() {
    let mut it = Interpreter::new();
    it.load_program("fn main() -> Int { let f = |p| p.norm(); 1 }")
        .expect("parse");
    assert!(matches!(
        Checker::check_program(it.program()),
        Err(TyError::Ambiguous { .. })
    ));
}

#[test]
fn add_clear_and_solutions_are_ordinary_method_and_field_names() {
    // keywords only before a relation atom (`m6.rs`); here as method names,
    // fields, and parameters
    let methods = "struct Bag { add: Int }\
        impl Bag {\
            fn add(self, solutions: Int) -> Bag = Bag { add: self.add + solutions };\
            fn clear(self) -> Bag = Bag { add: 0 };\
        }\
        fn main() -> Int = Bag { add: 1 }.add(2).clear().add(5).add;";
    assert_eq!(run_typed(methods).0, Value::Int(5));
}

#[test]
fn a_try_is_pure_only_when_every_from_is() {
    let src = "struct E1 { c: Int } struct E2 { c: Int }\
        impl From<E1> for E2 { fn from(e: E1) -> E2 { println(\"c\"); E2 { c: e.c } } }\
        fn f(x: Int) -> Result<Int, E1> = if x > 0 { Ok(x) } else { Err(E1 { c: 0 }) };\
        fn g(x: Int) -> Result<Int, E2> { let v = match x { n if f(n)? > 0 => 1, _ => 2 }; Ok(v) }\
        fn main() -> Result<Int, E2> = g(1);";
    assert!(matches!(
        check_rules_src(src),
        Err(bridger::relations::RuleError::ImpureGuard { ref witness, .. })
            if witness.first().is_some_and(|w| w == ".from")
    ));
}

// ---- an impl's own bounds ----

#[test]
fn an_impls_bounds_are_part_of_what_it_covers() {
    const SHOW: &str = "trait Show { fn show(self) -> String; }\
        impl Show for Int { fn show(self) -> String = \"int\"; }\
        impl<T: Show> Show for [T] {\
            fn show(self) -> String = match self { [] => \"\", h :: t => h.show() ++ t.show() };\
        }";
    // `[Int]` and `[[Int]]` are `Show`; `[fn]` is not, at the call …
    let ok = format!("{SHOW} fn main() -> String = [1, 2].show() ++ [[3]].show();");
    assert_eq!(run_typed(&ok).0, vstr("intintint"));
    let call = format!("{SHOW} fn main() -> String = [|x: Int| x].show();");
    assert!(matches!(
        check_program(&call),
        Err(TyError::UnsatisfiedBound { .. })
    ));
    // … and through a bounded generic function
    let via = format!(
        "{SHOW} fn f<T: Show>(x: T) -> String = x.show();\
         fn main() -> String = f([|x: Int| x]);"
    );
    assert!(matches!(
        check_program(&via),
        Err(TyError::UnsatisfiedBound { .. })
    ));
}

#[test]
fn an_impls_bounds_are_read_in_its_own_scope() {
    // `impl<T: Ord> Ord for [T]` needs `[T]` to admit equality, which its own
    // `T: Ord` grants — whatever declaration happens to be checked last
    let src = "fn zzz<U>(x: U) -> U = x;\
        impl<T: Ord> Ord for [T] { fn cmp(self, other: Self) -> Ordering = Equal; }\
        fn main() -> [Int] = min([1], [2]);";
    assert!(check_program(src).is_ok());
    assert_eq!(run_typed(src).0, vlist(vec![Value::Int(1)]));
}

// ---- associated functions on built-in heads; bounds with arguments ----

#[test]
fn a_built_in_type_name_receives_associated_calls() {
    let src = "impl Int { fn zero() -> Int = 0; }\
        impl String { fn empty() -> String = \"\"; }\
        fn main() -> (Int, String) = (Int.zero(), String.empty());";
    assert_eq!(run_typed(src).0, vtuple(vec![Value::Int(0), vstr("")]));
}

#[test]
fn a_bound_names_a_declared_trait_with_its_arguments() {
    const CONV: &str = "trait Conv<T> { fn conv(self) -> T; }\
        impl Conv<Int> for Bool { fn conv(self) -> Int = 1; }";
    // an unknown trait, and a trait applied to the wrong number of arguments
    assert!(matches!(
        check_program("fn f<T: Nope>(x: T) -> T = x; fn main() -> () = ();"),
        Err(TyError::UnknownTrait { .. })
    ));
    assert!(matches!(
        check_program(&format!(
            "{CONV} fn f<T: Conv>(x: T) -> T = x; fn main() -> () = ();"
        )),
        Err(TyError::TraitArity { .. })
    ));
    // the arguments are part of the bound: `Conv<String>` is not met by
    // `impl Conv<Int>` …
    let wrong = format!(
        "{CONV} fn f<T: Conv<String>>(x: T) -> String = x.conv();\
         fn main() -> String = f(true);"
    );
    assert!(matches!(
        check_program(&wrong),
        Err(TyError::UnsatisfiedBound { .. })
    ));
    // … while `Conv<Int>` is, and the call through the bound is typed with
    // the trait's argument
    let right = format!(
        "{CONV} fn f<T: Conv<Int>>(x: T) -> Int = x.conv() + 1;\
         fn main() -> Int = f(true);"
    );
    assert_eq!(run_typed(&right).0, Value::Int(2));
}

#[test]
fn a_type_name_is_not_a_constructor_and_a_type_is_not_a_struct() {
    assert!(matches!(
        check_program("struct Point { x: Int } fn main() -> Point = Point(1);"),
        Err(TyError::NotAConstructor { .. })
    ));
    assert!(matches!(
        check_program("fn main() -> Int { let o = Option { x: 1 }; 1 }"),
        Err(TyError::NotAStruct { .. })
    ));
}

// ---- coherence by instance, resolution by call site ----

#[test]
fn a_user_impl_meets_a_bound_from_m8() {
    let src = "struct Bag { xs: [Int] }\
        impl Len for Bag { fn length(self) -> Int = 7; }\
        fn main() -> Int = len(Bag { xs: [1] });";
    assert_eq!(run_typed(src).0, Value::Int(7));
}

#[test]
fn several_from_impls_are_told_apart_by_their_argument_heads() {
    const ERRS: &str = "struct IoErr { c: Int } struct ParseErr { m: String } struct CfgErr { s: String }\
        impl From<IoErr> for CfgErr { fn from(e: IoErr) -> CfgErr = CfgErr { s: \"io\" }; }\
        impl From<ParseErr> for CfgErr { fn from(e: ParseErr) -> CfgErr = CfgErr { s: \"parse\" }; }\
        fn rd() -> Result<Int, IoErr> = Err(IoErr { c: 1 });\
        fn pr() -> Result<Int, ParseErr> = Err(ParseErr { m: \"m\" });";
    // `?` picks the impl by the error type …
    let src = format!(
        "{ERRS} fn cfg() -> Result<Int, CfgErr> = Ok(rd()? + pr()?);\
         fn main() -> String = match cfg() {{ Ok(_) => \"ok\", Err(e) => e.s }};"
    );
    assert_eq!(run_typed(&src).0, vstr("io"));
    // … and a direct call by its argument
    let direct = format!("{ERRS} fn main() -> String = CfgErr.from(ParseErr {{ m: \"x\" }}).s;");
    assert_eq!(run_typed(&direct).0, vstr("parse"));
    // a generic instance must be the only one
    let overlap = "struct W { v: Int }\
        impl<T> From<T> for W { fn from(x: T) -> W = W { v: 0 }; }\
        impl From<Int> for W { fn from(x: Int) -> W = W { v: x }; }\
        fn main() -> () = ();";
    assert!(matches!(
        check_program(overlap),
        Err(TyError::OverlappingImpls { .. })
    ));
    // instances distinguished by their heads, each at its own instance of
    // the head type
    let seq = "struct Seq<T> { xs: [T] } struct Map<K, V> { ps: [(K, V)] }\
        impl<T> From<[T]> for Seq<T> { fn from(xs: [T]) -> Seq<T> = Seq { xs: xs }; }\
        impl<K, V> From<Map<K, V>> for Seq<(K, V)> {\
            fn from(m: Map<K, V>) -> Seq<(K, V)> = Seq { xs: m.ps };\
        }\
        fn f() -> Result<Int, [Int]> = Err([1, 2]);\
        fn g() -> Result<Int, Map<Int, Bool>> = Err(Map { ps: [(1, true)] });\
        fn h() -> Result<Int, Seq<Int>> = Ok(f()?);\
        fn k() -> Result<Int, Seq<(Int, Bool)>> = Ok(g()?);\
        fn main() -> (Int, Int) = (\
            match h() { Err(s) => len(s.xs), Ok(_) => 0 },\
            match k() { Err(s) => len(s.xs), Ok(_) => 0 });";
    assert_eq!(run_typed(seq).0, vtuple(vec![Value::Int(2), Value::Int(1)]));
}

#[test]
fn a_call_through_a_bound_needs_one_instance() {
    const CONV: &str = "trait Conv<T> { fn conv(self) -> T; }\
        impl Conv<Int> for Bool { fn conv(self) -> Int = 1; }\
        impl Conv<String> for Bool { fn conv(self) -> String = \"s\"; }";
    // the running program dispatches on the receiver's head alone
    let via_bound = format!(
        "{CONV} fn g<T: Conv<String>>(x: T) -> String = x.conv();\
         fn main() -> String = g(true);"
    );
    assert!(matches!(
        check_program(&via_bound),
        Err(TyError::AmbiguousInstance { .. })
    ));
    // and a direct call whose arguments do not choose is ambiguous
    let direct = format!("{CONV} fn main() -> Int = true.conv();");
    assert!(matches!(
        check_program(&direct),
        Err(TyError::AmbiguousMethod { .. })
    ));
}

#[test]
fn an_impl_may_extend_a_particular_instance() {
    const SEQ: &str = "struct Seq<T> { xs: [T] }\
        impl Len for Seq<Int> { fn length(self) -> Int = 42; }";
    assert_eq!(
        run_typed(&format!(
            "{SEQ} fn main() -> Int = Seq {{ xs: [1] }}.length();"
        ))
        .0,
        Value::Int(42)
    );
    assert!(matches!(
        check_program(&format!(
            "{SEQ} fn main() -> Int = Seq {{ xs: [true] }}.length();"
        )),
        Err(TyError::NoSuchMethod { .. })
    ));
    assert!(matches!(
        check_program(&format!(
            "{SEQ} fn f<T: Len>(x: T) -> Int = len(x); fn main() -> Int = f(Seq {{ xs: [true] }});"
        )),
        Err(TyError::UnsatisfiedBound { .. })
    ));
    // two impls of a trait without arguments for one head are one too many
    assert!(matches!(
        check_program(&format!(
            "{SEQ} impl Len for Seq<Bool> {{ fn length(self) -> Int = 7; }} fn main() -> () = ();"
        )),
        Err(TyError::DuplicateImpl { .. })
    ));
}

#[test]
fn a_try_converts_through_the_impl_covering_its_error_types() {
    // the impl is chosen by the instantiated types, not by heads
    let wrong = "struct Box<T> { v: T } struct E { code: Int }\
        impl From<Box<Int>> for E { fn from(b: Box<Int>) -> E = E { code: b.v + 1 }; }\
        fn g() -> Result<Int, Box<String>> = Err(Box { v: \"s\" });\
        fn f() -> Result<Int, E> { let x = g()?; Ok(x) }\
        fn main() -> () = ();";
    assert!(matches!(
        check_program(wrong),
        Err(TyError::Mismatch { .. })
    ));
    // a generic impl is found
    let generic = "struct Wrap<T> { v: T }\
        impl<T> From<T> for Wrap<T> { fn from(x: T) -> Wrap<T> = Wrap { v: x }; }\
        fn g() -> Result<Int, String> = Err(\"e\");\
        fn f() -> Result<Int, Wrap<String>> { let x = g()?; Ok(x) }\
        fn main() -> String = match f() { Ok(_) => \"ok\", Err(w) => w.v };";
    assert_eq!(run_typed(generic).0, vstr("e"));
    // an error type still being inferred unifies with the function's
    let meta = "fn f() -> Result<Int, String> { let r = Ok(1); let x = r?; Ok(x + 1) }\
        fn main() -> Result<Int, String> = f();";
    assert_eq!(run_typed(meta).0, vctor("Ok", vec![Value::Int(2)]));
    // and a `from` body's globals are initialized before a global that `?`s
    let order = "struct E1 { code: Int } struct E2 { msg: String }\
        impl From<E1> for E2 { fn from(x: E1) -> E2 = E2 { msg: zz ++ to_string(x.code) }; }\
        fn step() -> Result<Int, E1> = Err(E1 { code: 42 });\
        fn go() -> Result<Int, E2> = Ok(step()?);\
        let a = go(); let zz = \"code \";\
        fn main() -> String = match a { Ok(_) => \"ok\", Err(e) => e.msg };";
    assert_eq!(run_typed(order).0, vstr("code 42"));
}

#[test]
fn a_non_regular_recursive_type_does_not_admit_equality() {
    let src = "type R<T> = Base(T) | Rec(R<fn(Int) -> Int>);\
        fn main() -> Bool { let a: R<Int> = Rec(Base(|x| x)); a == a }";
    assert!(matches!(
        check_program(src),
        Err(TyError::NoEquality { .. })
    ));
    // the regular case still admits it
    let ok = "type L<T> = Nil | Cons(T, L<T>);\
        fn main() -> Bool = Cons(1, Nil) == Cons(1, Nil);";
    assert_eq!(run_typed(ok).0, Value::Bool(true));
}

#[test]
fn unchecked_a_struct_literal_names_a_struct() {
    assert!(matches!(
        run_stuck("fn main() -> Int { let s = Foo { a: 1 }; 1 }"),
        RuntimeError::NotAStruct { .. }
    ));
}

#[test]
fn instance_uniqueness_holds_for_an_impls_own_bounds() {
    // `Wrap<Seq<Int>>: Show` is proved through `impl<T: Conv<Int>> Show for
    // Wrap<T>`, whose bound `Seq<Int>: Conv<Int>` a running program could not
    // dispatch — `Seq` implements `Conv` twice
    let src = "type Seq<T> = Empty | Cons(T, Seq<T>);\
        trait Conv<T> { fn conv(self) -> T; }\
        impl Conv<String> for Seq<String> { fn conv(self) -> String = \"\"; }\
        impl Conv<Int> for Seq<Int> { fn conv(self) -> Int = 0; }\
        struct Wrap<T> { v: T }\
        trait Show { fn show(self) -> Int; }\
        impl<T: Conv<Int>> Show for Wrap<T> { fn show(self) -> Int = self.v.conv(); }\
        fn show_it<S: Show>(s: S) -> Int = s.show();\
        fn main() -> Int = show_it(Wrap { v: Cons(1, Empty) });";
    assert!(matches!(
        check_program(src),
        Err(TyError::AmbiguousInstance { .. })
    ));
}

#[test]
fn a_lambda_argument_is_checked_against_the_chosen_candidate() {
    let src = "trait Apply<T> { fn apply(self, x: T, f: fn(T) -> Int) -> Int; }\
        struct Box { n: Int }\
        impl Apply<Int> for Box { fn apply(self, x: Int, f: fn(Int) -> Int) -> Int = f(x) + self.n; }\
        impl Apply<String> for Box { fn apply(self, x: String, f: fn(String) -> Int) -> Int = f(x) * self.n; }\
        fn main() -> Int = Box { n: 10 }.apply(\"abc\", |s| s.length());";
    assert_eq!(run_typed(src).0, Value::Int(30));
}

#[test]
fn instances_are_told_apart_by_headless_arguments_too() {
    let src = "struct E { s: String }\
        impl From<(Int, String)> for E { fn from(p: (Int, String)) -> E = E { s: p.1 }; }\
        impl From<Int> for E { fn from(n: Int) -> E = E { s: \"int\" }; }\
        fn main() -> (String, String) = (E.from((1, \"t\")).s, E.from(2).s);";
    assert_eq!(run_typed(src).0, vtuple(vec![vstr("t"), vstr("int")]));
}

#[test]
fn a_deep_regular_type_admits_equality_and_a_growing_one_is_settled_quickly() {
    // 40 nested structs: regular, so data
    let mut src = String::new();
    for i in 1..40 {
        src.push_str(&format!("struct S{i} {{ next: S{} }} ", i + 1));
    }
    src.push_str("struct S40 { v: Int }");
    let mut build = String::from("S40 { v: 1 }");
    for i in (1..40).rev() {
        build = format!("S{i} {{ next: {build} }}");
    }
    src.push_str(&format!(" fn main() -> Bool {{ let a = {build}; a == a }}"));
    assert_eq!(run_typed(&src).0, Value::Bool(true));
    // an instantiation that doubles at each level is given up on, promptly
    let nest = "type Nest<T> = NNil | NCons(T, Nest<(T, T)>);\
        fn main() -> () { let n: Nest<Int> = NCons(1, NNil); println(n) }";
    assert!(matches!(
        check_program(nest),
        Err(TyError::NotPrintable { .. })
    ));
}

#[test]
fn a_field_called_like_a_method_and_a_self_less_method_on_a_value_are_explained() {
    assert!(matches!(
        check_program(
            "struct H { f: fn(Int) -> Int }\
             fn main() -> Int { let h = H { f: |x: Int| x + 1 }; h.f(1) }"
        ),
        Err(TyError::FieldNotMethod { .. })
    ));
    match check_program(
        "struct P { x: Int } impl P { fn get() -> Int = 1; }\
         fn main() -> Int = P { x: 1 }.get();",
    ) {
        Err(TyError::NotAMethod { ty, .. }) => assert_eq!(ty, "P"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_bound_on_a_struct_or_type_parameter_is_required_at_construction() {
    assert!(matches!(
        check_program(
            "struct Box<T: Ord> { v: T } fn main() -> Int { let b = Box { v: |x: Int| x }; 1 }"
        ),
        Err(TyError::UnsatisfiedBound { .. })
    ));
    assert!(
        check_program("struct Box<T: Ord> { v: T } fn main() -> Int = Box { v: 1 }.v;").is_ok()
    );
    assert!(matches!(
        check_program("type W<T: Ord> = Wrap(T); fn main() -> Int { let w = Wrap(|x: Int| x); 1 }"),
        Err(TyError::UnsatisfiedBound { .. })
    ));
}

#[test]
fn trait_diagnostics_name_the_signature_the_instance_and_the_rule() {
    // Plumbing: the help text is the reference's, not a student's; the payload
    // it reads is the graded part.
    const SHOW: &str = "trait Show { fn show(self) -> String; } struct P { x: Int }";
    match check_program(&format!(
        "{SHOW} impl Show for P {{ fn show(self) -> Int = 1; }} fn main() -> () = ();"
    )) {
        Err(TyError::SignatureMismatch {
            expected, found, ..
        }) => {
            assert!(expected.contains("-> String"), "{expected}");
            assert!(found.contains("-> Int"), "{found}");
        }
        other => panic!("{other:?}"),
    }
    // a struct matched as a constructor
    assert!(matches!(
        check_program("struct P { x: Int } fn main() -> Int = match (P { x: 1 }) { P(x) => x };"),
        Err(TyError::NotAConstructor { .. })
    ));
    // the parameter-bound hint fires for a parameter, not for a one-letter struct
    let param =
        check_program("fn show<T>(x: T) -> () = println(x); fn main() -> () = ();").unwrap_err();
    assert!(param.help().is_some_and(|h| h.contains("T: Print")));
    let strukt = check_program(
        "struct S { f: fn(Int) -> Int } fn main() -> () = println(S { f: |x: Int| x });",
    )
    .unwrap_err();
    assert!(
        !strukt.help().is_some_and(|h| h.contains("Print")),
        "{:?}",
        strukt.help()
    );
}

// ---- seventh review round ----

#[test]
fn an_associated_function_reached_through_a_bound_needs_the_type() {
    // `x.make()` under `T: Def` would be stuck at run time: `make` takes no
    // `self`, so the pairing is checked on the bound path as on an impl's
    let src = "trait Def { fn make() -> Self; fn get(self) -> Int; } struct W { v: Int }\
        impl Def for W { fn make() -> W = W { v: 9 }; fn get(self) -> Int = self.v; }\
        fn f<T: Def>(x: T) -> T = x.make(); fn main() -> Int = f(W { v: 1 }).get();";
    assert!(matches!(
        check_program(src),
        Err(TyError::NotAMethod { is_param: true, .. })
    ));
}

#[test]
fn a_method_delegating_to_a_same_named_method_on_another_type_is_not_recursive() {
    use bridger::relations::RuleError;
    // `self.a` is an `Int` by declaration, so `self.a.cmp(o.a)` is `Int`'s
    // native `cmp`, not `P`'s
    let src = "struct P { a: Int }\
        impl Ord for P { fn cmp(self, o: P) -> Ordering = self.a.cmp(o.a); }\
        relation q : (Int); relation s : (Int);\
        rule s(x) :- q(x) and min(P { a: x }, P { a: 1 }).a == 1;\
        fn main() -> [Int] { add q(1); add q(2); solutions s(?a) }";
    assert_eq!(run_typed(src).0, vlist(vec![Value::Int(1), Value::Int(2)]));
    // `self.cmp(o)` is the recursion it looks like
    let src = "struct P { a: Int }\
        impl Ord for P { fn cmp(self, o: P) -> Ordering = self.cmp(o); }\
        relation q : (Int); relation s : (Int);\
        rule s(x) :- q(x) and min(P { a: x }, P { a: 1 }).a == 1; fn main() {}";
    assert!(matches!(
        check_rules_src(src),
        Err(RuleError::Recursive { .. })
    ));
    // a receiver the declarations settle reaches one impl's method; a name
    // the body rebinds is not trusted, and reaches every impl's
    let impls = "struct P { a: Int } struct Q { b: Int }\
        impl P { fn size(self) -> Int = self.a; }\
        impl Q { fn size(self) -> Int { println(\"hi\"); self.b } }\
        relation q : (Int); relation s : (Int);";
    let settled = format!(
        "{impls} fn sz(p: P) -> Int = p.size();\
         rule s(x) :- q(x) and sz(P {{ a: x }}) == 1; fn main() {{}}"
    );
    assert!(check_rules_src(&settled).is_ok());
    let rebound = format!(
        "{impls} fn sz(p: P) -> Int {{ let p = Q {{ b: 1 }}; p.size() }}\
         rule s(x) :- q(x) and sz(P {{ a: x }}) == 1; fn main() {{}}"
    );
    assert!(matches!(
        check_rules_src(&rebound),
        Err(RuleError::Impure { .. })
    ));
}

#[test]
fn a_method_name_belongs_to_one_trait_per_head() {
    // dispatch at run time is by head and name alone, so a second trait's
    // method of one name on a head — a built-in conformance's included —
    // would be what a bounded call runs
    for src in [
        "trait Cmp { fn cmp(self, o: Self) -> Int; }\
         impl Cmp for Bool { fn cmp(self, o: Bool) -> Int = 1; } fn main() {}",
        "impl Bool { fn cmp(self, o: Bool) -> Int = 1; } fn main() {}",
        "struct P { a: Int } trait A { fn m(self) -> Int; } trait B { fn m(self) -> String; }\
         impl B for P { fn m(self) -> String = \"b\"; } impl A for P { fn m(self) -> Int = 1; }\
         fn main() {}",
    ] {
        assert!(
            matches!(check_program(src), Err(TyError::DuplicateMethod { .. })),
            "{src}"
        );
    }
    // one trait at two instances provides its method twice, told apart by
    // the instance
    let src = "struct S<T> { v: T } trait Conv<U> { fn conv(self) -> U; }\
        impl Conv<Int> for S<Int> { fn conv(self) -> Int = 1; }\
        impl Conv<String> for S<String> { fn conv(self) -> String = \"s\"; }\
        fn main() -> Int = S { v: 1 }.conv();";
    assert!(check_program(src).is_ok(), "{src}");
}

#[test]
fn a_self_method_of_a_built_in_called_on_the_type_name_is_the_pairing_error() {
    assert!(matches!(
        check_program("fn main() -> Ordering = Int.cmp(1, 2);"),
        Err(TyError::NoReceiver { .. })
    ));
}

#[test]
fn an_impl_head_is_validated_before_its_trait_is_judged() {
    let src = "struct W<U> { v: U }\
        impl Ord for W<T> { fn cmp(self, o: W<T>) -> Ordering = Equal; } fn main() {}";
    assert!(matches!(
        check_program(src),
        Err(TyError::UnknownType { .. })
    ));
    // and a field read off a value that is not a struct names the type
    assert!(matches!(
        check_program("fn main() -> Int = (1).x;"),
        Err(TyError::FieldOfNonStruct { .. })
    ));
}

// ---- eleventh review round ----

#[test]
fn an_unchecked_try_converts_nothing() {
    let src = "struct A { n: Int } struct B { n: Int }\
        impl From<A> for B { fn from(a: A) -> B = B { n: a.n + 100 }; }\
        fn f() -> Result<Int, A> = Err(A { n: 1 });\
        fn g() -> Result<Int, B> { let y = f()?; Ok(y) }\
        fn main() -> Result<Int, B> = g();";
    assert_eq!(
        run_program(src).0,
        vctor("Err", vec![vstruct("A", vec![("n", Value::Int(1))])])
    );
    assert_eq!(
        run_typed(src).0,
        vctor("Err", vec![vstruct("B", vec![("n", Value::Int(101))])])
    );
}

#[test]
fn a_struct_pattern_naming_a_missing_field_fails_to_match_unchecked() {
    let src = format!("{POINT} fn main() -> Int = match (Point {{ x: 1, y: 2 }}) {{ Point {{ z: 1 }} => 1, _ => 2 }};");
    assert_eq!(run_program(&src).0, Value::Int(2));
}

#[test]
fn struct_literal_fields_evaluate_in_source_order_and_store_in_declaration_order() {
    let src = format!(
        "{POINT} fn p(v: Int) -> Int {{ println(v); v }}\
         fn main() -> () = print(Point {{ y: p(2), x: p(1) }});"
    );
    assert_eq!(run_program(&src).1, "2\n1\nPoint { x: 1, y: 2 }");
}

#[test]
fn a_return_is_caught_at_a_method_boundary() {
    let src = format!(
        "{POINT} impl Point {{ fn m(self) -> Int {{ if self.x > 0 {{ return 5; }}; 0 }} }}\
         fn main() -> Int = Point {{ x: 1, y: 2 }}.m();"
    );
    assert_eq!(run_typed(&src).0, Value::Int(5));
}

#[test]
fn bool_receives_associated_calls() {
    let src = "impl Bool { fn yes() -> Bool = true; } fn main() -> Bool = Bool.yes();";
    assert_eq!(run_typed(src).0, Value::Bool(true));
}

#[test]
fn structs_order_by_their_fields_in_solutions() {
    let src = format!(
        "{POINT} relation p : (Point);\
         fn main() -> [Point] {{ add p(Point {{ x: 2, y: 0 }}); add p(Point {{ x: 1, y: 9 }});\
             add p(Point {{ x: 1, y: 3 }}); solutions p(?q) }}"
    );
    assert_eq!(
        run_typed(&src).0,
        vlist(vec![
            vstruct("Point", vec![("x", Value::Int(1)), ("y", Value::Int(3))]),
            vstruct("Point", vec![("x", Value::Int(1)), ("y", Value::Int(9))]),
            vstruct("Point", vec![("x", Value::Int(2)), ("y", Value::Int(0))]),
        ])
    );
}

#[test]
fn min_max_and_minimum_break_ties_toward_the_first_and_compare_from_the_best_so_far() {
    let src = "struct P { k: Int, tag: Int }\
        impl Ord for P { fn cmp(self, o: P) -> Ordering {\
            println(to_string(self.tag) ++ \" \" ++ to_string(o.tag));\
            if self.k < o.k { Less } else if self.k == o.k { Equal } else { Greater } } }\
        fn main() -> (Int, Int, Int) {\
            let a = min(P { k: 1, tag: 1 }, P { k: 1, tag: 2 }).tag;\
            let b = max(P { k: 1, tag: 1 }, P { k: 1, tag: 2 }).tag;\
            let c = unwrap_or(minimum([P { k: 1, tag: 1 }, P { k: 2, tag: 2 }, P { k: 0, tag: 3 }]),\
                P { k: 0, tag: 0 }).tag;\
            (a, b, c) }";
    let (v, out) = run_typed(src);
    assert_eq!(v, vtuple(vec![Value::Int(1), Value::Int(1), Value::Int(3)]));
    // `minimum` folds `min` from the left: the best so far is the receiver
    assert_eq!(out, "1 2\n1 2\n1 2\n1 3\n");
}

#[test]
fn a_generator_variable_reaches_its_columns_method_alone() {
    // `p : P` by the relation's declaration, so `p.big()` is `P.big`, and
    // an impure `Q.big` does not count against the filter
    let src = "struct P { a: Int } struct Q { b: Int }\
        impl P { fn big(self) -> Bool = self.a > 1; }\
        impl Q { fn big(self) -> Bool { println(\"q\"); self.b > 1 } }\
        relation r : (P); relation big : (P);\
        rule big(p) :- r(p) and p.big();\
        fn main() -> [P] { add r(P { a: 2 }); add r(P { a: 1 }); solutions big(?x) }";
    assert_eq!(
        run_typed(src).0,
        vlist(vec![vstruct("P", vec![("a", Value::Int(2))])])
    );
}
