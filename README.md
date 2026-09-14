# Bridger

An interpreter for **Bridger**, built one evaluation rule at a time. The plumbing that fights the
borrow checker — the environment, the store, the relation database, the parser — is provided and
frozen; you write the *meaning* of the language in `src/interp/eval.rs`.

The concepts behind each milestone are developed in the book,
[*A Practical Introduction to Programming Language Design*](https://www.cs.montana.edu/users/murphy/books/pl-design/).

## Build and check

```bash
cargo build
cargo m1                                  # the tests in force at Milestone 1
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

The tests in `tests/` are gated by milestone and cumulative: `cargo m3` (short for
`cargo test --features m3`) runs every test in force at Milestone 3, which includes the M1 and
M2 tests that M3 does not revise. Plain `cargo test` runs the M1 set. A stuck test checks the
error's variant and span, never its message.

## Running a program

`bridger::parser::parse_program` turns source text into the `Program` the interpreter runs, and
`parse_expr` parses one expression — handy for trying a rule in a test. The parser is provided; you
never edit it.

## Where your work lives

You implement one method, `eval_expr`, in `src/interp/eval.rs`, growing it a little each milestone.
From M5 the type checker's judgments in `src/types/check.rs`: `check_expr` and `unify`, then
`check_rule` at M6. From M6 the relations sublanguage in `src/relations/`: `Subst::unify`
(`unify.rs`), the `solutions` engine (`engine.rs`), and each static check as its own function in
`check.rs` — `check_generators`, `check_range_restriction`, `check_pure`, and (M7) `check_guards`.
Everything else — the `Checker`'s tables and type-structure helpers, the relation database, and the
provided driver that calls the functions above — is provided, and calling into your functions is how
that driver works.
Every place you must write code is a milestone-tagged macro — `todo_m1!`, …, `todo_m8!`. To list a
milestone's holes:

```bash
grep -rn 'todo_m3!' src
```

Replace each `todo_mN!(..)` with the rule's implementation. The crate always compiles: an unfilled
hole type-checks anywhere and panics with its milestone tag if reached at run time.

## Milestones

The interpreter grows one milestone at a time; each adds language features and the code that
gives them meaning, and the tests in force at milestone *N* are cumulative (`cargo mN`). What you
implement at each:

- **M1 — Expressions.** Literals, arithmetic and comparison, boolean `and` / `or`, string and
  list concatenation, `::` cons, tuples, lists, and tuple projection — the evaluator's arms for
  these forms in `eval_expr`.
- **M2 — Binding.** Variables and blocks: `let` bindings and sequencing, evaluated in a lexical
  environment.
- **M3 — State and control.** `if`, `while`, `for`, assignment, `return`, and mutable references
  (`ref` / `deref` / `:=`) backed by the store.
- **M4 — Functions.** Lambdas and calls: closures over the environment, and dispatch between a
  Bridger closure and a native prelude primitive.
- **M5 — Types.** A bidirectional type checker over the M1–M4 language: `check_expr` (infer or
  check each form) and `unify` (structural agreement that also solves inference variables), in
  `types/check.rs`.
- **M6 — Relations.** The Datalog-style sublanguage: `Subst::unify` (match a generator against a
  fact), the `solutions` least-fixpoint engine, the static rule checks (`check_generators`,
  `check_range_restriction`, `check_pure`), and the type checker's `check_rule`; the evaluator's
  `Relation` and `ForQuery` arms.
- **M7 — Algebraic data types.** Constructors, `match` (patterns, exhaustiveness, and pure guards
  via `check_guards`), and `?` for error propagation.
- **M8 — Objects.** Structs, field access, and methods dispatched on the receiver's head type,
  with trait-style bounds.

## Layout

```
src/
  main.rs         the `bridger` command: run a .brg file, report errors
  lib.rs          crate wiring and orientation
  macros.rs       the todo_mN! hole markers
  ast.rs          the syntax tree every pass works over
  parser/         source text → AST (provided): mod.rs, lexer.rs, grammar.lalrpop, build.rs
  interp/         evaluation
    mod.rs        the Interpreter: load_program, eval_program (provided)
    value.rs      runtime values
    env.rs        the lexical environment (lookup / extend)
    store.rs      mutable cells behind `ref` / `deref` / `:=`
    error.rs      runtime errors and control flow
    prelude.rs    built-in primitives
    eval.rs       eval_expr — your code (M1 on)
  types/          the static type checker (M5 on)
    mod.rs        the Checker: tables, context, driver, type-structure helpers (provided)
    error.rs      type errors
    check.rs      check_expr, unify, check_rule — your code
  relations/      the Datalog fragment (M6)
    mod.rs        check_rules driver; re-exports (provided)
    db.rs         the dynamic facts behind `add` / `clear` (provided)
    term.rs       terms and substitutions (provided)
    error.rs      errors in a program's rules (provided)
    unify.rs      unify — your code: one generator against one fact
    engine.rs     solutions — your code: the least-fixpoint engine
    check.rs      check_generators, check_range_restriction, check_pure, check_guards — your code
tests/
  common/         helpers: Expr builders, value_of / stuck
  m1.rs           Milestone 1 tests (m2.rs, … follow, one file per milestone)
  parser.rs       the provided parser (ungated)
  load.rs         loading a program (ungated)
  programs.rs     whole programs with expected output, tests/programs/*.brg (M8)
prelude.brg       the Bridger-level prelude; prelude-bounds.brg joins it at M8
```
