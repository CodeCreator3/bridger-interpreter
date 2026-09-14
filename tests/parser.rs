//! The provided parser: source text to AST. These tests are plumbing checks,  [frozen — do not edit]
//! not milestone properties, so they are not gated on a milestone feature.

use bridger::ast::*;

use bridger::parser::{parse_expr, parse_program, ParseError};

fn e(src: &str) -> Expr {
    parse_expr(src, SrcId::SYNTHETIC).unwrap_or_else(|err| panic!("{src:?}: {err}"))
}

fn fails(src: &str) -> ParseError {
    match parse_expr(src, SrcId::SYNTHETIC) {
        Ok(ast) => panic!("{src:?} parsed as {ast:?}"),
        Err(err) => err,
    }
}

/// Strip spans so trees can be compared for shape.
fn shape(e: &Expr) -> String {
    use Expr::*;
    let s = |e: &Expr| shape(e);
    let list = |es: &[Expr]| es.iter().map(shape).collect::<Vec<_>>().join(", ");
    match e {
        Lit(l, _) => format!("{l:?}"),
        Var(x, _) => x.clone(),
        Unary(op, e, _) => format!("({op:?} {})", s(e)),
        Binary(op, a, b, _) => format!("({op:?} {} {})", s(a), s(b)),
        If(c, t, e, _) => format!(
            "(if {} {} {})",
            s(c),
            s(t),
            e.as_ref().map(|e| s(e)).unwrap_or_default()
        ),
        Block(stmts, tail, _) => {
            let stmts: Vec<String> = stmts
                .iter()
                .map(|st| match st {
                    Stmt::Let(x, _, e, _) => format!("let {x} = {}", s(e)),
                    Stmt::Expr(e) => s(e),
                })
                .collect();
            format!(
                "{{{}; {}}}",
                stmts.join("; "),
                tail.as_ref().map(|t| s(t)).unwrap_or_default()
            )
        }
        Assign(a, b, _) => format!("(:= {} {})", s(a), s(b)),
        Call(f, args, _) => format!("(call {} [{}])", s(f), list(args)),
        Method(r, m, args, _) => format!("(method {} {m} [{}])", s(r), list(args)),
        Lambda(ps, body, _) => format!(
            "(lambda [{}] {})",
            ps.iter()
                .map(|p| match &p.ty {
                    Some(t) => format!("{}: {t:?}", p.name),
                    None => p.name.clone(),
                })
                .collect::<Vec<_>>()
                .join(", "),
            s(body)
        ),
        Match(x, arms, _) => format!("(match {} {} arms)", s(x), arms.len()),
        While(c, b, _) => format!("(while {} {})", s(c), s(b)),
        For(x, xs, b, _) => format!("(for {x} in {} {})", s(xs), s(b)),
        ForQuery(q, b, _) => format!("(for-query {} {})", q.name, s(b)),
        Return(e, _) => format!("(return {})", s(e)),
        Try(e, _) => format!("(try {})", s(e)),
        Tuple(es, _) => format!("(tuple {})", list(es)),
        List(es, _) => format!("[{}]", list(es)),
        Field(e, f, _) => format!("(field {} {f})", s(e)),
        Proj(e, i, _) => format!("(proj {} {i})", s(e)),
        Ctor(c, args, _) => format!("(ctor {c} [{}])", list(args)),
        Struct(n, fs, _) => format!(
            "(struct {n} {})",
            fs.iter()
                .map(|(f, e)| format!("{f}: {}", s(e)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Relation(rel, _) => match rel {
            Rel::Add(q) => format!("(add {})", q.name),
            Rel::Clear(r) => format!("(clear {r})"),
            Rel::Solutions(q) => format!("(solutions {})", q.name),
            Rel::Query(q) => format!("(query {} {} args)", q.name, q.args.len()),
        },
    }
}

fn assert_shape(src: &str, expected: &str) {
    assert_eq!(shape(&e(src)), expected, "source: {src}");
}

// ---- literals and operators ----

#[test]
fn literals() {
    assert_shape("42", "Int(42)");
    assert_shape("1_000_000", "Int(1000000)");
    assert_shape("true", "Bool(true)");
    assert_shape(r#""hi\n\u{1F600}""#, "Str(\"hi\\n😀\")");
    assert_shape("()", "Unit");
    assert_shape("-5", "Int(-5)");
    assert_shape("- x", "(Neg x)");
}

#[test]
fn precedence_and_associativity() {
    assert_shape("1 + 2 * 3", "(Add Int(1) (Mul Int(2) Int(3)))");
    assert_shape("1 - 2 - 3", "(Sub (Sub Int(1) Int(2)) Int(3))");
    assert_shape("a ++ b ++ c", "(Concat a (Concat b c))");
    assert_shape("1 :: 2 :: []", "(Cons Int(1) (Cons Int(2) []))");
    assert_shape("x :: xs ++ ys", "(Cons x (Concat xs ys))");
    assert_shape("not a == b", "(Eq (Not a) b)");
    assert_shape("a < b and c or d", "(Or (And (Lt a b) c) d)");
    assert_shape("-a * b", "(Mul (Neg a) b)");
    assert_shape("r := deref r + 1", "(:= r (Add (Deref r) Int(1)))");
    assert_shape("deref ref x", "(Deref (Ref x))");
}

#[test]
fn comparisons_do_not_chain() {
    let err = fails("a < b < c");
    assert!(matches!(err, ParseError::Unexpected { found, .. } if found == "`<`"));
}

// ---- postfix forms ----

#[test]
fn calls_fields_methods_projection() {
    assert_shape("f(1, 2)", "(call f [Int(1), Int(2)])");
    assert_shape("f(1)(2)", "(call (call f [Int(1)]) [Int(2)])");
    assert_shape("p.x", "(field p x)");
    assert_shape("p.x.y", "(field (field p x) y)");
    assert_shape("o.m(1)", "(method o m [Int(1)])");
    assert_shape("(o.m)(1)", "(call (field o m) [Int(1)])");
    assert_shape("t.0.1", "(proj (proj t 0) 1)");
    assert_shape("e?", "(try e)");
    assert_shape("f(x)?.y", "(field (try (call f [x])) y)");
}

#[test]
fn constructors_and_structs() {
    assert_shape("None", "(ctor None [])");
    assert_shape("Some(1)", "(ctor Some [Int(1)])");
    assert_shape(
        "Node(Leaf, 3, Leaf)",
        "(ctor Node [(ctor Leaf []), Int(3), (ctor Leaf [])])",
    );
    assert_shape(
        "Point { x: 1, y: 2, }",
        "(struct Point x: Int(1), y: Int(2))",
    );
    assert_shape("Point { x: 1 }.x", "(field (struct Point x: Int(1)) x)");
}

#[test]
fn tuples_and_lists() {
    assert_shape("(1, true)", "(tuple Int(1), Bool(true))");
    assert_shape("(1)", "Int(1)");
    assert_shape("[]", "[]");
    assert_shape("[1, 2 + 3]", "[Int(1), (Add Int(2) Int(3))]");
}

// ---- control flow and blocks ----

#[test]
fn if_while_for() {
    assert_shape("if c { 1 } else { 2 }", "(if c {; Int(1)} {; Int(2)})");
    assert_shape(
        "if a { 1 } else if b { 2 } else { 3 }",
        "(if a {; Int(1)} (if b {; Int(2)} {; Int(3)}))",
    );
    assert_shape("if c { print(1); }", "(if c {(call print [Int(1)]); } )");
    // an `if` without `else` ends at its block, not at the next token
    let src = "{ if c { 1 } x }";
    let Expr::Block(stmts, _, _) = e(src) else {
        panic!()
    };
    let Stmt::Expr(Expr::If(_, _, None, span)) = &stmts[0] else {
        panic!()
    };
    assert_eq!(
        (span.start, span.end),
        (src.find("if").unwrap(), src.find(" x").unwrap())
    );
    assert_shape(
        "while deref i <= n { i := deref i + 1; }",
        "(while (Le (Deref i) n) {(:= i (Add (Deref i) Int(1))); })",
    );
    assert_shape("for x in xs { x }", "(for x in xs {; x})");
    assert_shape("for path(0, ?d) { d }", "(for-query path {; d})");
    assert_shape(
        "1 + if c { 2 } else { 3 }",
        "(Add Int(1) (if c {; Int(2)} {; Int(3)}))",
    );
}

#[test]
fn blocks_bind_and_yield() {
    assert_shape(
        "{ let x = 1; let y = x + 1; x * y }",
        "{let x = Int(1); let y = (Add x Int(1)); (Mul x y)}",
    );
    assert_shape("{ f(); }", "{(call f []); }");
    assert_shape("{ }", "{; }");
    // ref sugar: `ref x = e;` is `let x = ref e;`
    assert_shape(
        "{ ref acc = 0; deref acc }",
        "{let acc = (Ref Int(0)); (Deref acc)}",
    );
    // a braced form ending a block with no `;` is its value; earlier it is a statement
    assert_shape(
        "{ if c { 1 } else { 2 } }",
        "{; (if c {; Int(1)} {; Int(2)})}",
    );
    assert_shape(
        "{ if c { 1 } else { 2 } 3 }",
        "{(if c {; Int(1)} {; Int(2)}); Int(3)}",
    );
    assert_shape("{ while c { } x }", "{(while c {; }); x}");
    assert_shape("{ { 1 } }", "{; {; Int(1)}}");
    assert_shape("{ ref x; }", "{(Ref x); }");
}

#[test]
fn lambdas_and_return() {
    assert_shape("|x, y| x + y", "(lambda [x, y] (Add x y))");
    assert_shape("|| 0", "(lambda [] Int(0))");
    assert_shape("|x: Int, y| x", "(lambda [x: Int, y] x)");
    assert_shape(
        "f(|x| x * x, 0)",
        "(call f [(lambda [x] (Mul x x)), Int(0)])",
    );
    assert_shape("(|x| x)(1)", "(call (lambda [x] x) [Int(1)])");
    assert_shape("return x + 1", "(return (Add x Int(1)))");
}

/// Strip spans from a pattern so shapes can be compared.
fn pat_shape(p: &Pattern) -> String {
    use Pattern::*;
    let list = |ps: &[Pattern]| ps.iter().map(pat_shape).collect::<Vec<_>>().join(", ");
    match p {
        Wild(_) => "_".into(),
        Lit(l, _) => format!("{l:?}"),
        Var(x, _) => x.clone(),
        Ctor(c, ps, _) => format!("{c}({})", list(ps)),
        Tuple(ps, _) => format!("({})", list(ps)),
        List(ps, rest, _) => match rest {
            Some(r) => format!("[{}, ...{r}]", list(ps)),
            None => format!("[{}]", list(ps)),
        },
        Cons(h, t, _) => format!("({} :: {})", pat_shape(h), pat_shape(t)),
        Struct(s, fs, _) => format!(
            "{s} {{ {} }}",
            fs.iter()
                .map(|(f, p)| format!("{f}: {}", pat_shape(p)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Or(a, b, _) => format!("({} | {})", pat_shape(a), pat_shape(b)),
    }
}

#[test]
fn match_arms_and_patterns() {
    let src = "match xs { [] => 0, [h, ...t] if h > 0 => h, h :: t => 1, Some(1 | 2) | None => 2, (a, _) => a, P { x, y: 0 } => x, -1 => 9, }";
    let m = e(src);
    let Expr::Match(_, arms, _) = m else {
        panic!("not a match")
    };
    let shapes: Vec<String> = arms.iter().map(|a| pat_shape(&a.pat)).collect();
    assert_eq!(
        shapes,
        [
            "[]",
            "[h, ...t]",
            "(h :: t)",
            "(Some((Int(1) | Int(2))) | None())",
            "(a, _)",
            "P { x: x, y: Int(0) }",
            "Int(-1)",
        ]
    );
    assert!(arms[1].guard.is_some());
    // spans: the third arm's pattern is `h :: t`
    let span = arms[2].pat.span();
    assert_eq!(&src[span.start..span.end], "h :: t");
    // a shorthand struct field binds a variable at the field name's span
    let Pattern::Struct(_, fs, _) = &arms[5].pat else {
        panic!()
    };
    let Pattern::Var(_, s) = &fs[0].1 else {
        panic!()
    };
    assert_eq!(&src[s.start..s.end], "x");
}

// ---- relations ----

#[test]
fn queries_and_relation_ops() {
    assert_shape("path(0, ?d)", "(query path 2 args)");
    assert_shape("path(0, 3)", "(call path [Int(0), Int(3)])");
    assert_shape("add edge(e.0, e.1)", "(add edge)");
    assert_shape("clear edge", "(clear edge)");
    assert_shape(
        "len(solutions path(src, ?))",
        "(call len [(solutions path)])",
    );
    let err = fails("f(x)(?y)");
    assert!(matches!(err, ParseError::Invalid { .. }));
}

#[test]
fn rules_split_into_conjuncts() {
    let p = parse_program(
        "relation edge : (Int, Int);
         rule path(x, y) :- edge(x, y);
         rule path(x, z) :- edge(x, y) and path(y, z) and x != z and is_ok(x);",
        SrcId::SYNTHETIC,
    )
    .unwrap();
    assert!(matches!(p.decls["edge"], Decl::Relation(ref tys) if tys.len() == 2));
    assert_eq!(p.rules.len(), 2);
    assert_eq!(p.rules[0].head.name, "path");
    let body = &p.rules[1].body;
    assert_eq!(body.len(), 4);
    // every conjunct is a plain expression; generator vs. filter is name
    // resolution's call when the rules are installed (Part VII)
    assert!(matches!(&body[0], Expr::Call(..)));
    assert!(matches!(&body[2], Expr::Binary(BinOp::Ne, ..)));
    assert!(matches!(&body[3], Expr::Call(..)));
}

// ---- definitions ----

#[test]
fn function_type_struct_trait_impl_definitions() {
    let src = r#"
        fn sum_sq(n: Int) -> Int {
            ref acc = 0;
            ref i = 1;
            while deref i <= n {
                acc := deref acc + deref i * deref i;
                i := deref i + 1;
            }
            deref acc
        }
        type Tree<T> = Leaf | Node(Tree<T>, T, Tree<T>);
        type Option<T> = | None | Some(T);
        fn sum_tree(t: Tree<Int>) -> Int =
            match t {
                Leaf => 0,
                Node(l, v, r) => sum_tree(l) + v + sum_tree(r),
            };
        let xs: [Int] = [1, 2, 3, 4];
        ref counter: Int = 0;
        struct Point { x: Int, y: Int, }
        trait Show { fn show(self) -> String; fn zero() -> Self; }
        impl Show for Point {
            fn show(self) -> String = "point";
            fn zero() -> Self = Point { x: 0, y: 0 };
        }
        impl<T: Show> Point { fn norm(self, k: T) -> Int { self.x + self.y } }
        fn id<T>(x: T) -> T = x;
        fn apply(f: fn(Int) -> Int, r: ref<Int>, p: (Int, Bool)) = f(deref r);
        fn main() -> () { print(sum_sq(3)); }
    "#;
    let p = parse_program(src, SrcId::SYNTHETIC).unwrap_or_else(|err| panic!("{err}"));
    assert!(
        matches!(&p.decls["sum_sq"], Decl::Fn(g, ps, ret, _) if g.params.is_empty() && ps.len() == 1 && *ret == Ty::int())
    );
    assert!(matches!(&p.decls["Tree"], Decl::Type(g, vs) if g.params.len() == 1 && vs.len() == 2));
    assert!(matches!(&p.decls["Option"], Decl::Type(_, vs) if vs.len() == 2));
    assert!(
        matches!(&p.decls["xs"], Decl::Global(Some(ty), _) if matches!(ty.kind, TyKind::List(_)))
    );
    assert!(
        matches!(&p.decls["counter"], Decl::Global(Some(ty), e) if matches!(ty.kind, TyKind::Ref(_)) && matches!(**e, Expr::Unary(UnOp::Ref, ..)))
    );
    assert!(matches!(&p.decls["Point"], Decl::Struct(_, fs) if fs.len() == 2));
    assert!(
        matches!(&p.decls["Show"], Decl::Trait(_, sigs) if sigs.len() == 2 && sigs[0].has_self && !sigs[1].has_self)
    );
    assert!(
        matches!(&p.decls["apply"], Decl::Fn(_, ps, ret, _) if ps.len() == 3 && *ret == Ty::unit())
    );
    assert_eq!(p.impls.len(), 2);
    assert_eq!(
        p.impls[0].trait_.as_ref().map(|t| t.name.as_str()),
        Some("Show")
    );
    assert_eq!(p.impls[0].ty, Ty::named("Point".into(), vec![]));
    // written types carry their span; the declaration table carries each declaration's
    let ret_span = match &p.decls["sum_sq"] {
        Decl::Fn(_, _, ret, _) => ret.span.unwrap(),
        _ => panic!(),
    };
    assert_eq!(&src[ret_span.start..ret_span.end], "Int");
    let d = p.spans["Point"];
    assert_eq!(&src[d.start..d.end], "struct Point { x: Int, y: Int, }");
    let Decl::Struct(_, fs) = &p.decls["Point"] else {
        panic!()
    };
    assert_eq!(&src[fs[1].span.start..fs[1].span.end], "y: Int");
    assert!(p.impls[1].trait_.is_none());
    assert_eq!(
        p.impls[1].generics.params[0]
            .bound
            .as_ref()
            .map(|b| b.name.as_str()),
        Some("Show")
    );
    let Decl::Fn(_, _, _, body) = &p.decls["main"] else {
        panic!()
    };
    assert!(matches!(**body, Expr::Block(..)));
}

#[test]
fn repeated_top_level_names_are_rejected() {
    let err = parse_program("fn f() = 1; fn f() = 2;", SrcId::SYNTHETIC).unwrap_err();
    assert!(matches!(err, ParseError::Invalid { ref message, .. } if message.contains("`f`")));
}

// ---- lexical errors ----

#[test]
fn lexical_rules_are_enforced() {
    for bad in [
        "fooBar", "_x", "foo__bar", "bar_", "My_Type", "007", "1_00", "\"open", "!x", "/* /* */",
    ] {
        let err = parse_expr(bad, SrcId::SYNTHETIC).unwrap_err();
        assert!(
            matches!(err, ParseError::Lex { .. }),
            "{bad:?} gave {err:?}"
        );
    }
    assert_shape(
        "x /* a /* nested */ comment */ + 1 // tail",
        "(Add x Int(1))",
    );
}

#[test]
fn a_minus_before_digits_is_a_literal_unless_it_follows_an_operand() {
    // the lexer glues `-` to the digits only where a `-` cannot be binary
    assert_shape("-9223372036854775808", "Int(-9223372036854775808)");
    assert_shape(
        "0 + -9223372036854775808",
        "(Add Int(0) Int(-9223372036854775808))",
    );
    assert_shape("-123_456", "Int(-123456)");
    assert_shape("x -1", "(Sub x Int(1))");
    assert_shape("x - 1", "(Sub x Int(1))");
    assert_shape("- 1", "(Neg Int(1))");
    assert_shape("--1", "(Neg Int(-1))");
    // the magnitude alone is out of range, and so is a spaced or binary minus
    // applied to it — those are negation and subtraction, not a literal
    for bad in [
        "9223372036854775808",
        "1 - 9223372036854775808",
        "- 9223372036854775808",
        "9223372036854775809",
    ] {
        fails(bad);
    }
}

#[test]
fn errors_carry_spans() {
    let err = fails("1 + ");
    assert!(matches!(err, ParseError::Eof { span, .. } if span.start == 4));
    let err = fails("1 + )");
    assert!(matches!(err, ParseError::Unexpected { span, .. } if span.start == 4 && span.end == 5));
    let bad = "\"abc";
    let err = parse_expr(bad, SrcId::SYNTHETIC).unwrap_err();
    assert_eq!(err.span().start, 0);
}

#[test]
fn names_within_a_declaration_are_distinct() {
    for (src, what) in [
        ("fn f(x: Int, x: Int) -> Int = x;", "parameter `x`"),
        ("fn f<T, T>(x: T) -> T = x;", "type parameter `T`"),
        ("struct P { x: Int, x: Bool }", "field `x`"),
        (
            "trait Tr { fn m(self) -> Int; fn m(self) -> Int; }",
            "method `m`",
        ),
        (
            "impl Int { fn m(self, a: Int, a: Int) -> Int = a; }",
            "parameter `a`",
        ),
    ] {
        let err = parse_program(src, SrcId::SYNTHETIC).unwrap_err();
        assert!(
            matches!(&err, ParseError::Invalid { message, .. } if message.contains(what)),
            "{src}: {err}"
        );
    }
    let err = fails("|y, y| y");
    assert!(matches!(&err, ParseError::Invalid { message, .. } if message.contains("`y`")));
}

#[test]
fn a_match_has_an_arm_and_a_constructor_call_an_argument() {
    fails("match x {}");
    fails("None()");
    assert_shape("match x { _ => 1, }", "(match x 1 arms)");
}

#[test]
fn a_negative_literal_has_no_leading_zeros_either() {
    fails("-007");
    assert_shape("-0", "Int(0)");
}

#[test]
fn an_offending_character_is_shown_escaped() {
    let err = parse_program("\u{feff}fn main() -> () = ();", SrcId::SYNTHETIC).unwrap_err();
    assert!(
        matches!(&err, ParseError::Lex { message, .. } if message.contains("\\u{feff}")),
        "{err}"
    );
}

#[test]
fn common_syntax_mistakes_get_a_hint() {
    for (src, hint) in [
        ("if c then 1 else 2", "braces"),
        ("if x = 3 { 1 } else { 2 }", "`==` compares"),
        ("{ fn f() -> Int = 1; 1 }", "top-level only"),
    ] {
        let err = fails(src);
        assert!(
            err.help().is_some_and(|h| h.contains(hint)),
            "{src}: {err} / {:?}",
            err.help()
        );
    }
    // the contextual keywords never show up as expected tokens
    let err = fails("let (a, b) = p;");
    assert!(!err.to_string().contains("solutions"), "{err}");
}

#[test]
fn a_missing_semicolon_is_suggested_and_a_long_literal_is_cut_short() {
    let err = parse_program(
        "fn main() -> () { let x = 1 println(x); }",
        SrcId::SYNTHETIC,
    )
    .unwrap_err();
    assert!(
        err.help().is_some_and(|h| h.contains("`;`")),
        "{err} / {:?}",
        err.help()
    );
    let long = "9".repeat(1000);
    let err = fails(&long);
    assert!(err.to_string().len() < 120, "{}", err.to_string().len());
    assert!(err.to_string().contains('…'));
}

#[test]
fn an_unterminated_string_and_a_duplicate_name_are_blamed_precisely() {
    let src = "fn main() -> () { let x = \"abc;\n println(x); }";
    let err = parse_program(src, SrcId::SYNTHETIC).unwrap_err();
    assert!(err.to_string().contains("unterminated"), "{err}");
    assert_eq!(&src[err.span().start..err.span().start + 1], "\"");
    let src = "fn f() -> Int = 1; fn f() -> Int = 2;";
    let err = parse_program(src, SrcId::SYNTHETIC).unwrap_err();
    assert_eq!(&src[err.span().start..err.span().end], "f");
    assert_eq!(err.span().start, src.rfind("f()").unwrap());
}

#[test]
fn let_mut_is_explained_and_a_repeated_lambda_parameter_is_blamed() {
    // `mut` is reserved so the hint can fire on it, and only on it
    let src = "fn main() -> () { let mut x = 1; }";
    let err = parse_program(src, SrcId(1)).unwrap_err();
    assert!(err.help().is_some_and(|h| h.contains("immutable")), "{err}");
    assert_eq!(&src[err.span().start..err.span().end], "mut");
    let src = "fn main() -> () { let x\n  println(x); }";
    let err = parse_program(src, SrcId(1)).unwrap_err();
    assert!(err.help().is_none_or(|h| !h.contains("immutable")), "{err}");
    // the second `x`, not the whole lambda
    let src = "fn main() -> () { let f = |x, x| x; }";
    let err = parse_program(src, SrcId(1)).unwrap_err();
    assert_eq!(err.span().start, src.rfind("x|").unwrap());
}

#[test]
fn a_built_in_type_name_takes_no_arguments() {
    // Appendix B gives `Int`, `Bool`, and `String` no argument list
    let err = fails("{ let x: Int<Int> = 1; x }");
    assert!(
        matches!(&err, ParseError::Invalid { message, .. } if message.contains("no type arguments")),
        "{err}"
    );
}

#[test]
fn the_rule_operator_inside_a_struct_literal_is_explained() {
    let err = fails("P { a:-1 }");
    assert!(err.help().is_some_and(|h| h.contains("a: -1")), "{err}");
}
