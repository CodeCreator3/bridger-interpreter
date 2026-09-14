//! `Interpreter::load_program`: parsing and unioning files.  [frozen — do not edit]

use bridger::ast::*;
use bridger::interp::Interpreter;
use bridger::parser::ParseError;

#[test]
fn declarations_and_rules_union_across_files() {
    let mut it = Interpreter::new();
    // `new` seeds the prelude, so count declarations relative to that baseline.
    let base = it.program().decls.len();
    it.load_program("struct P { x: Int } impl P { fn get(self) -> Int = self.x; }")
        .unwrap();
    it.load_program("rule path(x, y) :- edge(x, y) and ok(x);")
        .unwrap();
    it.load_program("relation edge : (Int, Int); relation path : (Int, Int); fn main() -> () { }")
        .unwrap();
    let p = it.program();
    assert_eq!(p.decls.len(), base + 4);
    assert_eq!(p.impls.len(), 1);
    assert_eq!(p.rules.len(), 1);
    // a rule body is plain expressions; nothing is resolved at load time
    assert_eq!(
        p.rules[0].head.terms,
        vec![Term::Var("x".into()), Term::Var("y".into())]
    );
    assert!(matches!(
        &p.rules[0].body[..],
        [Expr::Call(..), Expr::Call(..)]
    ));
}

#[test]
fn names_are_unique_across_files() {
    let mut it = Interpreter::new();
    it.load_program("fn f() -> Int = 1;").unwrap();
    let src = "let a = 0; relation f : (Int);";
    let err = it.load_program(src).unwrap_err();
    assert!(matches!(err, ParseError::Invalid { ref message, span }
            if message.contains("`f`") && span.start == src.find("relation").unwrap()));
    // the failed load left nothing behind
    assert!(!it.program().decls.contains_key("a"));
}

#[test]
fn a_clash_with_a_prelude_constructor_says_where_it_comes_from() {
    let mut it = Interpreter::new();
    let err = it
        .load_program("type X = Some(Int) | Other; fn main() {}")
        .unwrap_err();
    assert!(err.to_string().contains("defined by the prelude"), "{err}");
}

#[test]
fn a_parse_error_in_a_later_file_leaves_earlier_loads_intact() {
    let mut it = Interpreter::new();
    let base = it.program().decls.len();
    it.load_program("fn f() -> Int = 1;").unwrap();
    assert!(it.load_program("fn g() -> Int = ;").is_err());
    assert_eq!(it.program().decls.len(), base + 1);
}
