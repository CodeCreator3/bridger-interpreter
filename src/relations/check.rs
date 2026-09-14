//! Static checks on rules — YOUR file.  \[student\]  (Milestones M6, M7)
//!
//! A rule body is a list of expressions (see [`Rule`]); the parser has not
//! sorted them into generators and filters, and it could not have. These
//! checks are run by `Interpreter::check` (from M6), after the program is
//! loaded and before `main` runs, and together make the engine in `engine.rs`
//! safe to run: after them, every generator names a relation over variables
//! and literals, every variable a filter or the head uses is bound by a
//! generator, and every filter is pure, so it can be evaluated any number of
//! times in any order.
//!
//! Each check is its own function, called once per rule by the provided
//! driver (`check_rules` in `relations/mod.rs`); [`check_guards`] is a
//! separate M7 check over the whole program. Everything else in this file —
//! how you compute the impure functions, the rule's scope, a witness path — is
//! yours to shape; the driver calls only the four functions below.
//!
//! To list a milestone's holes:
//!
//! ```text
//! grep -rn 'todo_m6!' src
//! ```

use super::RuleError;
use crate::ast::{Program, Rule};

/// Which conjuncts of `rule` are generators: a call whose callee names a
/// relation declared in `program`. Everything else is a filter. A generator's
/// arguments must be variables or literals (`ArgumentNotATerm`), and its
/// count must match the relation's declared arity (`Arity`); the head must
/// name a declared relation of the right arity too.
pub fn check_generators(program: &Program, rule: &Rule) -> Result<Vec<bool>, RuleError> {
    let _ = (program, rule);
    todo_m6!("check_generators: classify conjuncts; terms only; head and generator arities")
}

/// Range restriction: every variable in the head, and every variable a
/// filter reads, is bound by some generator of the same rule — or names a
/// top-level definition (a global constant or a function), which every
/// definition may refer to. A generator-bound variable shadows a top-level
/// name; a variable bound by neither is `Unbound`. `generators` is
/// `check_generators`'s answer for this rule.
pub fn check_range_restriction(
    program: &Program,
    rule: &Rule,
    generators: &[bool],
) -> Result<(), RuleError> {
    let _ = (program, rule, generators);
    todo_m6!("check_range_restriction: head and filter variables are generator-bound")
}

/// A filter must be pure (reference, "Purity and effects"): a deterministic
/// function of the variables the generators bound. The never-pure forms are
/// `ref` / `deref` / `:=`, `add` / `clear`, every query (`solutions q`, a
/// bare `q`, `for q`), `while` / `for`, and the impure builtins (`print`,
/// `read_*`); a call is pure only if it names a function known pure or a pure
/// native, so a call through a parameter, a local, a global, or a computed
/// closure is impure — a locally bound name is a local wherever it appears,
/// whatever function shares its name; a method call reaches one head's
/// method when the declarations settle the receiver (`self`, a parameter, a
/// literal, a type or struct name, a generator variable, or a field chain
/// from one) and every impl's method otherwise, and a method with no impl
/// behind it is a built-in conformance, pure; the function handed to
/// `map`/`filter`/`fold` must be a named function, a pure native, or a
/// lambda, for the same reason; `e?` is pure when every
/// `impl From`'s `from` is (it is a `match` and a `return`, and `return` is
/// control, not an effect). A
/// function is pure when its body is, and the set of pure functions is the
/// **greatest** fixpoint: assume every function pure, strike out each whose
/// body contains a never-pure form or calls a struck-out function, repeat
/// until nothing changes. (Equivalently: the impure set is the least fixpoint
/// of "reaches an effect".) Two further conditions make a filter total, which
/// the fixpoint engine needs: the pure call graph is **acyclic** (no self- or
/// mutual recursion among filter-reachable functions), and no such function
/// takes a function-typed parameter. A rejected filter reports the chain of
/// calls from the filter to the effect it reaches (`Impure { witness }`).
/// `generators` is `check_generators`'s answer for this rule.
pub fn check_pure(program: &Program, rule: &Rule, generators: &[bool]) -> Result<(), RuleError> {
    let _ = (program, rule, generators);
    todo_m6!("check_pure: every filter is a pure, total function of the bound variables")
}

/// Every `match` guard in the program must be pure (M7): a guard is a test
/// the matcher runs while choosing an arm, and an effect there would make the
/// choice depend on how many arms were tried. The same purity judgment as a
/// rule filter's, including that no function it reaches takes a function-typed
/// parameter (the analysis cannot follow one); recursion is allowed, since a
/// guard runs once per arm and need not be total. Reports
/// `ImpureGuard { witness }`.
pub fn check_guards(program: &Program) -> Result<(), RuleError> {
    let _ = program;
    todo_m7!("check_guards: every match guard in the program is pure")
}
