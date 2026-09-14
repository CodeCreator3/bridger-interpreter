//! The parser  [frozen — do not edit]
//!
//! Turns Bridger source text into the [`ast`](crate::ast). The concrete grammar
//! is Appendix B of the book; this module realises it as a hand-written lexer
//! ([`lexer`]) feeding an LALRPOP grammar (`grammar.lalrpop`, compiled at
//! build time). Two entry points:
//!
//!  - [`parse_program`] — a whole program: definitions only, no loose code;
//!  - [`parse_expr`] — a single expression, for tests and experiments.
//!
//! A handful of surface forms are resolved here rather than in the evaluator
//! (see the note at the top of `ast.rs`).

mod build;
pub mod lexer;

use crate::ast::{Expr, Program, Span, SrcId};
use lalrpop_util::lalrpop_mod;
use thiserror::Error;

pub use lexer::Token;

lalrpop_mod!(
    #[allow(clippy::all, dead_code, unused_imports, unused_variables)]
    #[rustfmt::skip]
    grammar,
    "/parser/grammar.rs"
);

/// A syntax error, with the span of the offending text. The `bridger` binary
/// renders it as a labelled source excerpt, with a hint from [`ParseError::help`]
/// where the mistake has a known shape.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ParseError {
    /// The lexer could not form a token here.
    #[error("{message}")]
    Lex { message: String, span: Span },
    /// The grammar had no rule for this token here.
    #[error("unexpected {found}; expected {expected}")]
    Unexpected {
        found: String,
        expected: String,
        span: Span,
    },
    /// The input ended where more was needed.
    #[error("unexpected end of input; expected {expected}")]
    Eof { expected: String, span: Span },
    /// The text parsed but breaks a rule the grammar cannot express (a repeated
    /// top-level name, a hole in a plain call, an out-of-range projection …).
    #[error("{message}")]
    Invalid { message: String, span: Span },
}

impl ParseError {
    /// The span of the offending text.
    pub fn span(&self) -> Span {
        match self {
            ParseError::Lex { span, .. }
            | ParseError::Unexpected { span, .. }
            | ParseError::Eof { span, .. }
            | ParseError::Invalid { span, .. } => *span,
        }
    }
}

type LalrpopError = lalrpop_util::ParseError<usize, Token, ParseError>;

fn expected_list(expected: Vec<String>) -> String {
    // LALRPOP quotes terminal names, e.g. `"\"fn\""` and `"\"ident\""`. The
    // contextual keywords are names wherever a name is expected, so they are
    // folded into "a name"; a long list is cut short.
    let mut names: Vec<String> = Vec::new();
    for e in &expected {
        let e = e.trim_matches('"');
        let shown = match e {
            "ident" => "a name",
            "add" | "clear" | "solutions" => continue,
            "UpperIdent" => "a type or constructor name",
            "int" => "an integer",
            "str" => "a string",
            other => other,
        };
        if !names.iter().any(|n| n == shown) {
            names.push(shown.to_string());
        }
    }
    // The tokens that end or close things are the ones a reader looks for.
    names.sort_by_key(|n| !matches!(n.as_str(), ";" | "}" | ")" | "]" | ","));
    match names.len() {
        0 => "nothing".to_string(),
        1 => names[0].clone(),
        n if n <= 8 => format!("one of {}", names.join(", ")),
        _ => format!("one of {}, …", names[..8].join(", ")),
    }
}

impl ParseError {
    /// A suggestion to print under the message, for the mistakes with a
    /// known shape.
    pub fn help(&self) -> Option<String> {
        let (found, expected) = match self {
            ParseError::Unexpected {
                found, expected, ..
            } => (found.trim_matches('`'), expected.as_str()),
            ParseError::Eof { .. } => {
                return Some(
                    "the program ends before this construct does: a `{`, `(`, or `[` opened \
                     earlier may never be closed, or a `;` may be missing"
                        .to_string(),
                )
            }
            _ => return None,
        };
        Some(match found {
            "then" => "Bridger's `if` takes braces: `if c { a } else { b }`".to_string(),
            "=" => "`=` is not an operator: `==` compares, and `:=` assigns through a `ref`"
                .to_string(),
            "fn" | "type" | "struct" | "trait" | "impl" | "relation" | "rule"
                if !expected.contains(found) =>
            {
                "definitions are top-level only; a local function is a lambda, `|x| …`".to_string()
            }
            "(" if expected.starts_with("a name") || expected.contains("one of a name") => {
                "a binding names one variable; take a tuple apart with `.0` and `.1`, or \
                 with `match`"
                    .to_string()
            }
            // `let mut x`: `mut` is reserved for exactly this hint.
            "mut" => "`mut` is reserved and has no use: bindings are immutable; a mutable cell \
                 is `ref x = e;`, read with `deref x` and written with `x := e`"
                .to_string(),
            "?" => "a `?` hole belongs in a relation query, `r(?x, 1)`".to_string(),
            // `P { a:-1 }`: `:-` is one token, the rule operator.
            ":-" if expected.split(", ").any(|t| t == ":") => {
                "`:-` is the rule operator; a negative field value is written `a: -1`, with a space"
                    .to_string()
            }
            _ if expected.contains('{')
                && expected.contains("if")
                && !expected.contains("a name") =>
            {
                "`else` takes a block, `else { … }`, or another `if`".to_string()
            }
            _ if expected.split(", ").any(|t| t == ";" || t == "one of ;") => {
                "a `;` may be missing before this".to_string()
            }
            _ => return None,
        })
    }
}

fn convert(err: LalrpopError, src_len: usize, src_id: SrcId) -> ParseError {
    match err {
        LalrpopError::User { error } => error,
        LalrpopError::InvalidToken { location } => ParseError::Lex {
            message: "invalid token".into(),
            span: Span {
                src: src_id,
                start: location,
                end: location + 1,
            },
        },
        // LALRPOP reports the end of the last token; point at the end of the text.
        LalrpopError::UnrecognizedEof { expected, .. } => ParseError::Eof {
            expected: expected_list(expected),
            span: Span {
                src: src_id,
                start: src_len,
                end: src_len,
            },
        },
        LalrpopError::UnrecognizedToken {
            token: (start, tok, end),
            expected,
        } => ParseError::Unexpected {
            found: tok.to_string(),
            expected: expected_list(expected),
            span: Span {
                src: src_id,
                start,
                end,
            },
        },
        LalrpopError::ExtraToken {
            token: (start, tok, end),
        } => ParseError::Unexpected {
            found: tok.to_string(),
            expected: "end of input".into(),
            span: Span {
                src: src_id,
                start,
                end,
            },
        },
    }
}

/// Parse a whole program: a sequence of top-level definitions. `src_id`
/// identifies this source among those the interpreter has loaded, so every span
/// the parse produces knows which file it points into; pass
/// [`SrcId::SYNTHETIC`](crate::ast::SrcId::SYNTHETIC) for a one-off string with
/// no place in that list.
pub fn parse_program(src: &str, src_id: SrcId) -> Result<Program, ParseError> {
    grammar::ProgramParser::new()
        .parse(src_id, lexer::Lexer::new(src, src_id))
        .map_err(|e| convert(e, src.len(), src_id))
}

/// Parse one expression. See [`parse_program`] for `src_id`.
pub fn parse_expr(src: &str, src_id: SrcId) -> Result<Expr, ParseError> {
    grammar::ExprParser::new()
        .parse(src_id, lexer::Lexer::new(src, src_id))
        .map_err(|e| convert(e, src.len(), src_id))
}

/// The hint for `add`, `clear`, and `solutions` used as ordinary names where
/// the relation form was probably meant: they are keywords only when a
/// relation name follows, so `solutions(r(?x))` is a call to a function named
/// `solutions`.
pub fn keyword_hint(name: &str) -> &'static str {
    match name {
        "add" => " (as a keyword, `add` is followed by a relation atom: `add r(1, 2)`)",
        "clear" => " (as a keyword, `clear` is followed by a relation name: `clear r`)",
        "solutions" => {
            " (as a keyword, `solutions` is followed by its query, without parentheses: \
             `solutions r(?x)`)"
        }
        _ => "",
    }
}
