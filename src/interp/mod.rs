//! The interpreter driver  [frozen — do not edit]
//!
//! `Interpreter` holds the program loaded so far and the mutable run state your
//! evaluator threads through it — the store (M3) and the relation database
//! (M6) — and carries the provided orchestration. You never edit this file; you
//! add the one method that matters, [`Interpreter::eval_expr`], in `eval.rs`.
//!
//! The evaluation judgment reads `env ⊢ ⟨e, σ, Δ⟩ ⇓ ⟨r, σ', Δ'⟩`: the
//! environment is an input, passed down and extended per scope, while the store
//! σ and the relation database Δ are threaded — changed by one subexpression,
//! seen by the next. `eval_expr(&mut self, e, env)` is that shape in Rust:
//! `env` is the input, and `self` carries σ and Δ.

use crate::ast::{Decl, Expr, Name, Program, SrcId, Ty, TyKind};
use crate::parser::{self, ParseError};
use crate::relations::{RelationDb, RuleError};
use crate::types::Conversions;
use crate::types::TyError;
use std::collections::HashMap;
use std::rc::Rc;
use thiserror::Error;

pub mod env;
pub mod error;
mod eval;
pub mod prelude;

/// The most nested Bridger calls allowed at once: deeper recursion is the
/// stuck state `StackOverflow` rather than a process abort. The `bridger`
/// binary's 2 GiB stack has room for this many frames.
pub const MAX_DEPTH: usize = 100_000;
pub mod store;
pub mod value;

pub use env::Env;
pub use error::{Control, RuntimeError};
pub use store::{Loc, Store};
pub use value::{type_of, Closure, Value};

/// The interpreter. Call [`Interpreter::eval_expr`] (defined in `eval.rs`) to
/// evaluate an expression; the store and the relation database live here so
/// `ref` / `deref` / `:=` (M3) and `add` / `clear` / queries (M6) have somewhere
/// to keep their state.
pub struct Interpreter {
    /// Mutable cells backing `ref` values. Your `eval_expr` reaches for this
    /// from M3 on (`self.store.alloc(..)`, `.read(..)`, `.write(..)`).
    #[allow(dead_code)] // read by your `eval_expr` starting at M3
    pub(crate) store: Store,
    /// Dynamic facts: what `add` inserted and `clear` has not removed. Your
    /// `eval_expr` reaches for this from M6 on (`self.db.add(..)`, `.facts(..)`).
    #[allow(dead_code)] // read by your `eval_expr` starting at M6
    pub(crate) db: RelationDb,
    /// Every declaration loaded so far, unioned across [`Interpreter::load_program`]
    /// calls; `eval_program` runs `main` from here.
    pub(crate) program: Program,
    /// The source text of each load, in order, for diagnostics.
    #[allow(dead_code)] // read by the diagnostics pass
    sources: Vec<String>,
    /// Everything `print` / `println` have written. Whole-program tests read it
    /// back with [`Interpreter::output`]; the `bridger` binary streams it to
    /// stdout as it is written instead (see [`Interpreter::stream_output`]).
    output: String,
    /// Whether `print` / `println` write straight to stdout, line by line,
    /// rather than into `output` — so a run that is killed or aborts still
    /// shows what it printed.
    stream: bool,
    /// Whether the last thing printed left its line unfinished (no `\n`).
    line_open: bool,
    /// Nested Bridger calls in progress, held below [`MAX_DEPTH`] so runaway
    /// recursion is a clean error rather than a stack overflow.
    #[allow(dead_code)] // read from M4 on (enter_call / leave_call)
    pub(crate) depth: usize,
    /// The methods the type checker chose by span: the impl's method for a
    /// call whose receiver head alone could not pick between instances of a
    /// trait, and (M8) the `from` a `?` converts through. Empty until
    /// [`Interpreter::run`] type-checks, so a `?` with no entry — including
    /// every `?` under `eval_program` alone — converts nothing.
    #[allow(dead_code)] // filled from M5 (check), read from M7/M8 (`?`, dispatch)
    pub(crate) conversions: Conversions,
    /// The methods of every `impl`, by head type and name — what a method
    /// call dispatches on. Built when a program is loaded; the first impl of
    /// a head to provide a name is the one a head-only lookup finds (the
    /// checker's choice, where several instances exist, is in `conversions`).
    pub(crate) methods: HashMap<(String, Name), Rc<crate::ast::Method>>,
    /// The program's input, consumed by the `read_*` prelude family through a
    /// byte cursor. The `bridger` binary fills it from stdin; tests set it with
    /// [`Interpreter::set_input`].
    pub(crate) input: String,
    pub(crate) input_pos: usize,
    /// Set while the rule engine computes a fixpoint (M6). A query asked in
    /// that window came from a rule filter, which the static checks reject;
    /// the engine reports it as a stuck state rather than recursing.
    #[allow(dead_code)] // read by your engine starting at M6
    pub(crate) materializing: bool,
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

impl Interpreter {
    /// A fresh interpreter, with the Bridger-written prelude (`prelude.brg`)
    /// already loaded. The native primitives are installed into the environment
    /// by [`Interpreter::eval_program`]; expression-level tests that call
    /// `eval_expr` directly do not need either and can build their own `Env`.
    pub fn new() -> Self {
        let mut it = Interpreter {
            store: Store::new(),
            db: RelationDb::new(),
            program: Program::default(),
            sources: Vec::new(),
            output: String::new(),
            conversions: Conversions::new(),
            methods: HashMap::new(),
            stream: false,
            line_open: false,
            depth: 0,
            input: String::new(),
            input_pos: 0,
            materializing: false,
        };
        // From M8 the bounded prelude functions are Bridger over the traits
        // (`prelude-bounds.brg`); before it they are native. Both files load as
        // the one prelude source, so both are trusted alike.
        #[cfg(not(feature = "m8"))]
        let prelude = include_str!("../../prelude.brg").to_string();
        #[cfg(feature = "m8")]
        let prelude = format!(
            "{}\n{}",
            include_str!("../../prelude.brg"),
            include_str!("../../prelude-bounds.brg")
        );
        it.load_program(&prelude).expect("the prelude must parse");
        it
    }

    /// Set the program's input — what the `read_*` prelude functions consume,
    /// read as whitespace-delimited tokens (`read_int` / `read_bool`) or whole
    /// lines (`read_line`). The `bridger` binary fills this from stdin.
    pub fn set_input(&mut self, input: String) {
        self.input = input;
        self.input_pos = 0;
    }

    /// Parse `src` and union its definitions into the program. Files may be
    /// loaded in any order and may refer to one another's declarations; a name
    /// declared in two loads is an error, reported at the second site, and so
    /// is a top-level declaration of a prelude name — Bridger-written or
    /// native (`print`, `len`, …). Nothing is resolved here — names are looked
    /// up when the program runs.
    pub fn load_program(&mut self, src: &str) -> Result<(), ParseError> {
        // The id this source will hold in `self.sources`, so its spans point
        // back at it. A parse that fails pushes nothing, and the next load
        // reuses the id — harmless, since a failed load's spans are discarded.
        let src_id = SrcId(self.sources.len() as u32);
        let new = parser::parse_program(src, src_id)?;
        // One namespace for every top-level name. A constructor is referred
        // to by its bare name, like a function or a type, so it is a top-level
        // name too and may not coincide with any other — within its type,
        // across types, or with a declaration of another kind.
        let mut taken: HashMap<Name, String> = HashMap::new();
        for (name, decl) in &self.program.decls {
            let where_ = match self.program.spans.get(name) {
                Some(s) if s.src == SrcId(0) => "defined by the prelude",
                _ => "defined in an earlier file",
            };
            taken.insert(name.clone(), where_.to_string());
            if let Decl::Type(_, variants) = decl {
                for v in variants {
                    taken.insert(
                        v.name.clone(),
                        format!("a constructor of `{name}` {where_}"),
                    );
                }
            }
        }
        let mut claim = |name: &Name, what: String, span| -> Result<(), ParseError> {
            if prelude::native_names().any(|n| n == name) {
                return Err(ParseError::Invalid {
                    message: format!("`{name}` is a prelude primitive and cannot be redefined"),
                    span,
                });
            }
            if matches!(name.as_str(), "Int" | "Bool" | "String" | "Self") {
                return Err(ParseError::Invalid {
                    message: format!("`{name}` is a built-in type and cannot be declared"),
                    span,
                });
            }
            if let Some(prior) = taken.insert(name.clone(), what) {
                return Err(ParseError::Invalid {
                    message: format!("`{name}` is already {prior}"),
                    span,
                });
            }
            Ok(())
        };
        // In source order, so the reported site is the later declaration.
        let mut in_order: Vec<&Name> = new.decls.keys().collect();
        in_order.sort_by_key(|name| new.spans[*name].start);
        for name in in_order {
            let decl = &new.decls[name];
            claim(name, "defined".to_string(), new.spans[name])?;
            if let Decl::Type(_, variants) = decl {
                for v in variants {
                    claim(&v.name, format!("a constructor of `{name}`"), v.span)?;
                }
            }
        }
        self.program.decls.extend(new.decls);
        self.program.spans.extend(new.spans);
        self.program.impls.extend(new.impls);
        self.methods.clear();
        for imp in &self.program.impls {
            let Some(head) = impl_head_key(&imp.ty) else {
                continue;
            };
            for m in &imp.methods {
                self.methods
                    .entry((head.clone(), m.name.clone()))
                    .or_insert_with(|| Rc::new(m.clone()));
            }
        }
        self.program.rules.extend(new.rules);
        // New rules change M(P, Δ): a closure remembered from an earlier
        // query is stale.
        self.db.invalidate();
        self.sources.push(src.to_string());
        Ok(())
    }

    /// The program loaded so far.
    pub fn program(&self) -> &Program {
        &self.program
    }

    /// Everything the program has printed so far.
    pub fn output(&self) -> &str {
        &self.output
    }

    /// Whether the program's last output left its line unfinished — a
    /// `print` with no newline after it.
    pub fn output_line_open(&self) -> bool {
        self.line_open
    }

    /// Build the starting environment and run `main`. The recursive knot is tied
    /// here: every top-level `fn` (the program's and the prelude's) becomes a
    /// closure capturing the one root scope, so they can all call one another;
    /// the native primitives are installed alongside them; then each `global`
    /// initializer is evaluated, and finally `main()` is called.
    ///
    /// Globals run in dependency order ([`Program::initialization_order`]): a
    /// global may name a later one, and a cycle among initializers is an
    /// [`RuntimeError::InitializationCycle`] before anything runs. A `return`
    /// from `main` is caught here and becomes the program's result; a stuck
    /// evaluation surfaces as the inner [`RuntimeError`].
    ///
    /// This is the evaluation core only. Static pre-passes (type checking at M5,
    /// rule checks at M6, guard purity at M7) run in [`Interpreter::run`], which
    /// calls this last.
    pub fn eval_program(&mut self) -> Result<Value, RuntimeError> {
        // `main` exists and takes no parameters — checked here, where the
        // program is about to run, so a fragment or library can still be
        // loaded and type-checked without one.
        let main_span = self
            .program
            .spans
            .get("main")
            .copied()
            .unwrap_or(crate::ast::Span::new(SrcId::SYNTHETIC, 0, 0));
        let bad_main = match self.program.decls.get("main") {
            Some(Decl::Fn(generics, params, _, _))
                if params.is_empty() && generics.params.is_empty() =>
            {
                None
            }
            Some(Decl::Fn(..)) => Some("`main` takes no parameters and no type parameters"),
            Some(_) => Some("`main` must be a function"),
            None => Some("no `main` function: execution starts at `fn main()`"),
        };
        if let Some(reason) = bad_main {
            return Err(RuntimeError::Main {
                reason: reason.to_string(),
                span: main_span,
            });
        }
        // A tree deeper than the evaluator's recursion can hold would abort the
        // process; reject it first, with the same bound the type checker uses
        // from M5 (which reaches such a tree before this point). See
        // [`Program::too_deeply_nested`].
        if let Some(span) = self.program.too_deeply_nested(crate::types::MAX_NESTING) {
            return Err(RuntimeError::TooDeep {
                limit: crate::types::MAX_NESTING,
                span,
            });
        }
        let env = Env::new();

        // Native primitives first, then every top-level `fn` (prelude + program)
        // as a closure over the shared root — the mutual-recursion knot.
        for (name, value) in prelude::native_bindings() {
            env.bind_root(name, value);
        }
        for (name, decl) in &self.program.decls {
            match decl {
                Decl::Fn(_, params, _, body) => {
                    let params = params.iter().map(|p| p.name.clone()).collect();
                    env.add_function(name.clone(), params, (**body).clone());
                }
                // A relation is a root binding too, so a query resolves its
                // name the way a call resolves a function's.
                Decl::Relation(_) => env.bind_root(name.clone(), Value::Relation(name.clone())),
                _ => {}
            }
        }

        // Global initializers, in dependency order (see the doc comment).
        let order =
            self.program
                .initialization_order()
                .map_err(|c| RuntimeError::InitializationCycle {
                    names: c.names,
                    span: c.span,
                })?;
        for name in order {
            let Some(Decl::Global(_, init)) = self.program.decls.get(&name) else {
                continue;
            };
            let init = (**init).clone();
            // No function boundary here: a `return` escaping an initializer is
            // a static error from M5 and a stuck state when unchecked.
            let v = match self.eval_expr(&init, &env) {
                Ok(v) => v,
                Err(Control::Return(_)) => {
                    return Err(RuntimeError::ReturnOutsideFunction { span: init.span() })
                }
                Err(Control::Raise(e)) => return Err(e),
            };
            env.bind_root(name, v);
        }

        // Call `main()`, whose presence and shape were checked above.
        let call = Expr::Call(
            Box::new(Expr::Var("main".to_string(), main_span)),
            Vec::new(),
            main_span,
        );
        finish(self.eval_expr(&call, &env))
    }

    /// The text of a loaded source, for diagnostics: `SrcId(0)` is the
    /// prelude, `SrcId(1)` the first program loaded.
    pub fn source(&self, id: SrcId) -> Option<&str> {
        self.sources.get(id.0 as usize).map(String::as_str)
    }

    /// Stream `print` / `println` output to stdout as it happens, instead of
    /// collecting it for [`Interpreter::output`]. The `bridger` binary does
    /// this, so a program that is interrupted or aborts still shows what it
    /// printed before.
    pub fn stream_output(&mut self) {
        self.stream = true;
    }

    /// Write program output: to stdout when streaming, else into the buffer.
    pub(crate) fn emit(&mut self, s: &str) {
        if !s.is_empty() {
            self.line_open = !s.ends_with('\n');
        }
        if self.stream {
            use std::io::Write;
            let mut out = std::io::stdout().lock();
            let written = out.write_all(s.as_bytes()).and_then(|()| {
                if s.contains('\n') {
                    out.flush()
                } else {
                    Ok(())
                }
            });
            // The reader has gone (a closed pipe): there is no one to print
            // for, so the run ends quietly, as a shell tool does.
            if written.is_err() {
                std::process::exit(0);
            }
        } else {
            self.output.push_str(s);
        }
    }

    /// Enter a Bridger call (a function, method, or `from` body): one more
    /// frame, refused past [`MAX_DEPTH`]. Pair with [`Interpreter::leave_call`].
    #[allow(dead_code)] // called from M4 on (the `Call` arm and method bodies)
    pub(crate) fn enter_call(&mut self, span: crate::ast::Span) -> Result<(), RuntimeError> {
        if self.depth >= MAX_DEPTH {
            return Err(RuntimeError::StackOverflow {
                limit: MAX_DEPTH,
                span,
            });
        }
        self.depth += 1;
        Ok(())
    }

    /// Leave a Bridger call entered with [`Interpreter::enter_call`].
    #[allow(dead_code)] // called from M4 on (paired with enter_call)
    pub(crate) fn leave_call(&mut self) {
        self.depth -= 1;
    }

    /// Run the static pre-passes the loaded program's milestone has turned on,
    /// **installing the `?` error conversions** the evaluator needs. Each pass is
    /// gated on its milestone feature, so the interpreter at milestone *N* runs
    /// exactly the checks *N* implements: type checking (and its `?` elaboration)
    /// at M5, the rule / purity checks at M6, the `match`-guard purity check at
    /// M7. `run` calls this before evaluating;
    /// a test can call it, then [`Interpreter::eval_program`], to exercise a
    /// program's elaboration (e.g. a cross-type `?`) without the whole pipeline.
    pub fn check(&mut self) -> Result<(), RunError> {
        // Every stage runs, and the error reported is the earliest in the
        // source (Appendix D: the first error in source order); at one
        // position the rule checks win, since their messages name the rule's
        // mistake (an undeclared relation, an unbound logic variable) where
        // the type checker would report the same program less helpfully.
        #[allow(unused_mut)] // nothing is pushed before M5
        let mut errors: Vec<RunError> = Vec::new();
        #[cfg(feature = "m6")]
        if let Err(e) = crate::relations::check_rules(self.program()) {
            errors.push(e.into());
        }
        #[cfg(feature = "m5")]
        match crate::types::Checker::check_and_elaborate(self.program()) {
            // The checker's choices — the impl's method where the receiver's
            // head alone could not pick one, and from M8 the `from` a `?`
            // converts through — are what the evaluator dispatches on.
            Ok((_types, conversions)) => self.conversions = conversions,
            Err(e) => errors.push(e.into()),
        }
        #[cfg(feature = "m7")]
        if let Err(e) = crate::relations::check_guards(self.program()) {
            errors.push(e.into());
        }
        match errors
            .into_iter()
            .min_by_key(|e| (e.span().src.0, e.span().start))
        {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// Load `src`, [`check`](Interpreter::check) it, then evaluate it — the whole
    /// pipeline behind the `bridger` binary. Any stage's error surfaces through
    /// [`RunError`].
    pub fn run(&mut self, src: &str) -> Result<Value, RunError> {
        self.load_program(src)?;
        self.check()?;
        Ok(self.eval_program()?)
    }
}

/// The head-type key of an `impl`'s type, to match against a value's — how the
/// loader groups the methods a value's head can reach. Provided: the evaluator's
/// method dispatch reads the table this builds, so it needs no head key itself.
fn impl_head_key(ty: &Ty) -> Option<String> {
    match &ty.kind {
        TyKind::Named(n, _) => Some(n.clone()),
        TyKind::Int => Some("Int".to_string()),
        TyKind::Bool => Some("Bool".to_string()),
        TyKind::Str => Some("String".to_string()),
        TyKind::Unit => Some("()".to_string()),
        TyKind::List(_) => Some("[]".to_string()),
        _ => None,
    }
}

/// Collapse an `eval_expr` outcome to a plain value: a `Return` yields its value
/// (this is the `main` / global boundary), a `Raise` its inner error.
fn finish(r: Result<Value, Control>) -> Result<Value, RuntimeError> {
    match r {
        Ok(v) | Err(Control::Return(v)) => Ok(v),
        Err(Control::Raise(e)) => Err(e),
    }
}

/// A failure anywhere in the [`Interpreter::run`] pipeline: parsing, one of the
/// static pre-passes, or evaluation. Which variants can occur depends on the
/// milestone — the `Type` and `Rule` stages are gated on their features — but
/// all four are always defined so the type is stable across milestones.
#[derive(Debug, Error)]
pub enum RunError {
    #[error(transparent)]
    Parse(#[from] ParseError),
    #[error(transparent)]
    Type(#[from] TyError),
    #[error(transparent)]
    Rule(#[from] RuleError),
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
}

impl RunError {
    /// Where the error is: the span of the node it blames.
    pub fn span(&self) -> crate::ast::Span {
        match self {
            RunError::Parse(e) => e.span(),
            RunError::Type(e) => e.span(),
            RunError::Rule(e) => e.span(),
            RunError::Runtime(e) => e.span(),
        }
    }

    /// A suggestion to print under the message, for the errors that have one.
    pub fn help(&self) -> Option<String> {
        match self {
            RunError::Parse(e) => e.help(),
            RunError::Type(e) => e.help(),
            RunError::Rule(e) => e.help(),
            RunError::Runtime(e) => e.help(),
        }
    }
}
