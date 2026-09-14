//! The lexer  [frozen — do not edit]
//!
//! Turns Bridger source text into the token stream the LALRPOP grammar
//! consumes. Follows Appendix B's lexical productions: `snake_case` and
//! `UpperCamelCase` identifiers, decimal integers with optional thousands
//! grouping, escaped strings, line comments, and nesting block comments.

use super::ParseError;
use crate::ast::{Span, SrcId};

/// One token, with its payload where it has one. Spans travel beside the
/// token as `(start, token, end)` byte offsets.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Ident(String),
    UpperIdent(String),
    /// An integer literal, sign included: `-` immediately followed by digits
    /// is one token wherever a `-` cannot be subtraction (see
    /// `Lexer::after_operand`).
    Int(i64),
    Str(String),
    // keywords
    Fn,
    Let,
    If,
    Else,
    While,
    Match,
    Type,
    Struct,
    Trait,
    Impl,
    For,
    In,
    Relation,
    Rule,
    Add,
    Clear,
    Solutions,
    Not,
    And,
    Or,
    True,
    False,
    Ref,
    Deref,
    SelfValue,
    SelfType,
    Return,
    /// Reserved so that `let mut x` can be explained; it has no use.
    Mut,
    // punctuation
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Comma,
    Semi,
    Colon,
    ColonColon,
    ColonDash,
    Dot,
    DotDotDot,
    FatArrow,
    Arrow,
    Eq,
    EqEq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Plus,
    PlusPlus,
    Minus,
    Star,
    Slash,
    Percent,
    Assign,
    Pipe,
    Question,
    Underscore,
}

impl Token {
    /// Whether an operand can end with this token, so that a `-` right after
    /// it is subtraction rather than a prefix.
    fn ends_operand(&self) -> bool {
        use Token::*;
        matches!(
            self,
            Ident(_)
                | UpperIdent(_)
                | Int(_)
                | Str(_)
                | True
                | False
                | SelfValue
                | RParen
                | RBracket
                | RBrace
                | Question
        )
    }
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Token::*;
        let s = match self {
            Ident(x) | UpperIdent(x) => return write!(f, "`{x}`"),
            Int(n) => return write!(f, "`{n}`"),
            Str(s) => return write!(f, "{s:?}"),
            Fn => "fn",
            Let => "let",
            If => "if",
            Else => "else",
            While => "while",
            Match => "match",
            Type => "type",
            Struct => "struct",
            Trait => "trait",
            Impl => "impl",
            For => "for",
            In => "in",
            Relation => "relation",
            Rule => "rule",
            Add => "add",
            Clear => "clear",
            Solutions => "solutions",
            Not => "not",
            And => "and",
            Or => "or",
            True => "true",
            False => "false",
            Ref => "ref",
            Deref => "deref",
            SelfValue => "self",
            SelfType => "Self",
            Return => "return",
            Mut => "mut",
            LParen => "(",
            RParen => ")",
            LBracket => "[",
            RBracket => "]",
            LBrace => "{",
            RBrace => "}",
            Comma => ",",
            Semi => ";",
            Colon => ":",
            ColonColon => "::",
            ColonDash => ":-",
            Dot => ".",
            DotDotDot => "...",
            FatArrow => "=>",
            Arrow => "->",
            Eq => "=",
            EqEq => "==",
            Ne => "!=",
            Lt => "<",
            Le => "<=",
            Gt => ">",
            Ge => ">=",
            Plus => "+",
            PlusPlus => "++",
            Minus => "-",
            Star => "*",
            Slash => "/",
            Percent => "%",
            Assign => ":=",
            Pipe => "|",
            Question => "?",
            Underscore => "_",
        };
        write!(f, "`{s}`")
    }
}

fn keyword(word: &str) -> Option<Token> {
    use Token::*;
    Some(match word {
        "fn" => Fn,
        "let" => Let,
        "if" => If,
        "else" => Else,
        "while" => While,
        "match" => Match,
        "type" => Type,
        "struct" => Struct,
        "trait" => Trait,
        "impl" => Impl,
        "for" => For,
        "in" => In,
        "relation" => Relation,
        "rule" => Rule,
        "add" => Add,
        "clear" => Clear,
        "solutions" => Solutions,
        "not" => Not,
        "and" => And,
        "or" => Or,
        "true" => True,
        "false" => False,
        "ref" => Ref,
        "deref" => Deref,
        "self" => SelfValue,
        "Self" => SelfType,
        "return" => Return,
        "mut" => Mut,
        _ => return None,
    })
}

/// The token stream over one source text. Implements `Iterator`, yielding
/// `(start, token, end)` triples or a lexical [`ParseError`].
pub struct Lexer<'a> {
    src: &'a str,
    src_id: SrcId,
    pos: usize,
    /// Whether the previous token ends an operand — an identifier, a literal,
    /// `self`, a closing bracket, or `?`. Bridger has no juxtaposition, so a
    /// `-` after one of those can only be subtraction; anywhere else it is a
    /// prefix, and directly followed by digits it is a negative literal
    /// (Appendix B: `int_lit ::= [ '-' ] ( '0' | pos_int )`).
    after_operand: bool,
}

/// A lexer's item: the shape LALRPOP expects from an external lexer.
pub type Spanned = Result<(usize, Token, usize), ParseError>;

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str, src_id: SrcId) -> Self {
        Lexer {
            src,
            src_id,
            pos: 0,
            after_operand: false,
        }
    }

    fn peek(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn peek_at(&self, n: usize) -> Option<char> {
        self.src[self.pos..].chars().nth(n)
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn err(&self, start: usize, message: impl Into<String>) -> ParseError {
        ParseError::Lex {
            message: message.into(),
            span: Span {
                src: self.src_id,
                start,
                end: self.pos.max(start + 1).min(self.src.len().max(start + 1)),
            },
        }
    }

    /// Skip whitespace and comments. Returns an error for an unterminated
    /// block comment.
    fn skip_trivia(&mut self) -> Result<(), ParseError> {
        loop {
            match (self.peek(), self.peek_at(1)) {
                (Some(c), _) if c.is_whitespace() => {
                    self.bump();
                }
                (Some('/'), Some('/')) => {
                    while let Some(c) = self.peek() {
                        if c == '\n' {
                            break;
                        }
                        self.bump();
                    }
                }
                (Some('/'), Some('*')) => {
                    let start = self.pos;
                    self.bump();
                    self.bump();
                    let mut depth = 1usize;
                    while depth > 0 {
                        match (self.peek(), self.peek_at(1)) {
                            (Some('/'), Some('*')) => {
                                depth += 1;
                                self.bump();
                                self.bump();
                            }
                            (Some('*'), Some('/')) => {
                                depth -= 1;
                                self.bump();
                                self.bump();
                            }
                            (Some(_), _) => {
                                self.bump();
                            }
                            (None, _) => return Err(self.err(start, "unterminated block comment")),
                        }
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    fn lex_word(&mut self, start: usize) -> Spanned {
        while matches!(self.peek(), Some(c) if c.is_ascii_alphanumeric() || c == '_') {
            self.bump();
        }
        let word = &self.src[start..self.pos];
        if let Some(kw) = keyword(word) {
            return Ok((start, kw, self.pos));
        }
        let first = word.chars().next().unwrap();
        let tok = if first == '_' {
            if word == "_" {
                Token::Underscore
            } else {
                return Err(self.err(
                    start,
                    format!("`{word}`: an identifier may not begin with `_`"),
                ));
            }
        } else if first.is_ascii_uppercase() {
            if word.contains('_') {
                return Err(self.err(
                    start,
                    format!("`{word}`: type, constructor, and trait names are UpperCamelCase, without `_`"),
                ));
            }
            Token::UpperIdent(word.to_string())
        } else {
            if word.chars().any(|c| c.is_ascii_uppercase()) {
                return Err(self.err(
                    start,
                    format!(
                        "`{word}`: variable, function, field, and relation names are snake_case"
                    ),
                ));
            }
            if word.ends_with('_') || word.contains("__") {
                return Err(self.err(
                    start,
                    format!("`{word}`: an identifier may not end with `_` or contain `__`"),
                ));
            }
            Token::Ident(word.to_string())
        };
        Ok((start, tok, self.pos))
    }

    /// An integer literal starting at `start`, where the character there is
    /// a digit or a `-` directly before one. The sign is part of the literal,
    /// so the whole text is converted at once and `-9223372036854775808` is
    /// in range while `9223372036854775808` is not.
    fn lex_number(&mut self, start: usize) -> Spanned {
        if self.peek() == Some('-') {
            self.bump();
        }
        let digits_start = self.pos;
        while matches!(self.peek(), Some(c) if c.is_ascii_digit() || c == '_') {
            self.bump();
        }
        let text = &self.src[start..self.pos];
        let unsigned = &self.src[digits_start..self.pos];
        // A literal echoed in a message is cut short when it is very long.
        let shown = if text.len() > 24 {
            format!("{}…{}", &text[..12], &text[text.len() - 8..])
        } else {
            text.to_string()
        };
        let digits: String = text.chars().filter(|c| *c != '_').collect();
        let well_formed = if unsigned.contains('_') {
            // thousands grouping: 1–3 leading digits, then groups of exactly three
            let mut groups = unsigned.split('_');
            let head = groups.next().unwrap_or("");
            (1..=3).contains(&head.len()) && groups.all(|g| g.len() == 3)
        } else {
            true
        };
        if !well_formed {
            return Err(self.err(
                start,
                format!("`{shown}`: `_` may only group digits by thousands"),
            ));
        }
        let body = digits.strip_prefix('-').unwrap_or(&digits);
        if body.len() > 1 && body.starts_with('0') {
            return Err(self.err(
                start,
                format!("`{shown}`: an integer literal has no leading zeros"),
            ));
        }
        match digits.parse::<i64>() {
            Ok(n) => Ok((start, Token::Int(n), self.pos)),
            Err(_) => Err(self.err(start, format!("`{shown}`: integer literal out of range"))),
        }
    }

    fn lex_string(&mut self, start: usize) -> Spanned {
        self.bump(); // opening quote
        let mut out = String::new();
        loop {
            let c = match self.bump() {
                Some(c) => c,
                None => return Err(self.err(start, "unterminated string literal")),
            };
            match c {
                '"' => return Ok((start, Token::Str(out), self.pos)),
                '\\' => {
                    let esc_start = self.pos - 1;
                    match self.bump() {
                        Some('n') => out.push('\n'),
                        Some('t') => out.push('\t'),
                        Some('r') => out.push('\r'),
                        Some('0') => out.push('\0'),
                        Some('"') => out.push('"'),
                        Some('\\') => out.push('\\'),
                        Some('u') if self.peek() == Some('{') => {
                            self.bump();
                            let hex_start = self.pos;
                            while matches!(self.peek(), Some(c) if c.is_ascii_hexdigit()) {
                                self.bump();
                            }
                            let hex = &self.src[hex_start..self.pos];
                            if self.bump() != Some('}') || hex.is_empty() {
                                return Err(self.err(esc_start, "malformed `\\u{…}` escape"));
                            }
                            match u32::from_str_radix(hex, 16).ok().and_then(char::from_u32) {
                                Some(ch) => out.push(ch),
                                None => {
                                    return Err(self.err(
                                        esc_start,
                                        format!("`\\u{{{hex}}}` is not a Unicode scalar value"),
                                    ))
                                }
                            }
                        }
                        _ => return Err(self.err(esc_start, "unknown escape sequence")),
                    }
                }
                '\n' => return Err(self.err(start, "unterminated string literal")),
                c if c.is_control() => {
                    return Err(self.err(
                        self.pos - c.len_utf8(),
                        "a string literal may not contain a control character; use an escape",
                    ))
                }
                c => out.push(c),
            }
        }
    }

    fn lex_punct(&mut self, start: usize) -> Spanned {
        use Token::*;
        let c = self.bump().unwrap();
        let two = |lexer: &mut Self, next: char, yes: Token, no: Token| {
            if lexer.peek() == Some(next) {
                lexer.bump();
                yes
            } else {
                no
            }
        };
        let tok = match c {
            '(' => LParen,
            ')' => RParen,
            '[' => LBracket,
            ']' => RBracket,
            '{' => LBrace,
            '}' => RBrace,
            ',' => Comma,
            ';' => Semi,
            '|' => Pipe,
            '?' => Question,
            '*' => Star,
            '/' => Slash,
            '%' => Percent,
            '+' => two(self, '+', PlusPlus, Plus),
            '-' => two(self, '>', Arrow, Minus),
            '<' => two(self, '=', Le, Lt),
            '>' => two(self, '=', Ge, Gt),
            '=' => match self.peek() {
                Some('=') => {
                    self.bump();
                    EqEq
                }
                Some('>') => {
                    self.bump();
                    FatArrow
                }
                _ => Eq,
            },
            ':' => match self.peek() {
                Some(':') => {
                    self.bump();
                    ColonColon
                }
                Some('=') => {
                    self.bump();
                    Assign
                }
                Some('-') => {
                    self.bump();
                    ColonDash
                }
                _ => Colon,
            },
            '.' => {
                if self.peek() == Some('.') && self.peek_at(1) == Some('.') {
                    self.bump();
                    self.bump();
                    DotDotDot
                } else {
                    Dot
                }
            }
            '!' => {
                if self.peek() == Some('=') {
                    self.bump();
                    Ne
                } else {
                    return Err(self.err(start, "unexpected `!`; negation is spelled `not`"));
                }
            }
            other => {
                let shown: String = other.escape_default().collect();
                return Err(self.err(start, format!("unexpected character `{shown}`")));
            }
        };
        Ok((start, tok, self.pos))
    }
}

impl Iterator for Lexer<'_> {
    type Item = Spanned;

    fn next(&mut self) -> Option<Spanned> {
        if let Err(e) = self.skip_trivia() {
            self.pos = self.src.len();
            return Some(Err(e));
        }
        let start = self.pos;
        let c = self.peek()?;
        let negative_literal = c == '-'
            && !self.after_operand
            && self.src[self.pos + 1..].starts_with(|d: char| d.is_ascii_digit());
        let item = if c.is_ascii_alphabetic() || c == '_' {
            self.lex_word(start)
        } else if c.is_ascii_digit() || negative_literal {
            self.lex_number(start)
        } else if c == '"' {
            self.lex_string(start)
        } else {
            self.lex_punct(start)
        };
        if let Ok((_, tok, _)) = &item {
            self.after_operand = tok.ends_operand();
        }
        Some(item)
    }
}

/// `text` as an integer literal under the lexer's rules — an optional `-`,
/// digits grouped by thousands with `_` or not at all, no leading zeros, in
/// range — or `None`. What `read_int` accepts.
pub(crate) fn parse_int_literal(text: &str) -> Option<i64> {
    let unsigned = text.strip_prefix('-').unwrap_or(text);
    if unsigned.is_empty() || !unsigned.chars().all(|c| c.is_ascii_digit() || c == '_') {
        return None;
    }
    if unsigned.contains('_') {
        let mut groups = unsigned.split('_');
        let head = groups.next().unwrap_or("");
        if !(1..=3).contains(&head.len()) || !groups.all(|g| g.len() == 3) {
            return None;
        }
    }
    let digits: String = text.chars().filter(|c| *c != '_').collect();
    let body = digits.strip_prefix('-').unwrap_or(&digits);
    if body.len() > 1 && body.starts_with('0') {
        return None;
    }
    digits.parse::<i64>().ok()
}
