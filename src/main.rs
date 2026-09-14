//! The `bridger` command-line interpreter.  [frozen — do not edit]
//!
//! `bridger FILE.brg` loads the prelude and `FILE`, runs the static pre-passes
//! its milestone has turned on, evaluates `main`, and prints whatever the
//! program wrote. Which pre-passes run is fixed at build time by the milestone
//! feature (see the crate's `[features]`): `cargo run` is the M1 interpreter,
//! `cargo run --features m6` adds type and rule checking. This binary is a thin
//! wrapper over [`Interpreter::run`]; the interpreter itself is the library.

use bridger::ast::SrcId;
use bridger::interp::{Interpreter, RunError};
use miette::{NamedSource, SourceSpan};
use std::io::{IsTerminal, Read};
use std::process::ExitCode;

/// The interpreter's stack: a Bridger call is several nested `eval_expr`
/// frames, so deep recursion needs far more than a thread's default. Virtual
/// memory, committed only as the program actually recurses.
const STACK_SIZE: usize = 2 << 30;

const USAGE: &str = "usage: bridger FILE.brg
  Runs the Bridger program in FILE.brg: its static checks, then `main`.
  The program's input is standard input, read whole when it is not a terminal.
  Exit status 0 when the program ran, 1 when it was rejected or got stuck.";

/// A line longer than this is shown as a window around the span.
const MAX_EXCERPT_LINE: usize = 200;

fn main() -> ExitCode {
    // Run on a thread with a large stack; the main thread's size is fixed by
    // the OS (8 MiB on macOS and Linux) and cannot be raised from inside.
    let worker = std::thread::Builder::new()
        .name("bridger".into())
        .stack_size(STACK_SIZE)
        .spawn(run)
        .expect("spawn interpreter thread");
    worker.join().unwrap_or(ExitCode::FAILURE)
}

fn run() -> ExitCode {
    // Colour and hyperlinks only when a person is looking; a pipe gets plain
    // text with the same layout.
    // `NO_COLOR` set to anything non-empty turns colour off (no-color.org).
    let on_terminal = std::io::stderr().is_terminal()
        && std::env::var_os("NO_COLOR").is_none_or(|v| v.is_empty());
    let _ = miette::set_hook(Box::new(move |_| {
        Box::new(
            miette::MietteHandlerOpts::new()
                .color(on_terminal)
                .terminal_links(false)
                .width(100)
                .build(),
        )
    }));
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = match args.as_slice() {
        [p] if p == "--help" || p == "-h" => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        [p] => p.clone(),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    let mut src = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("bridger: cannot read {path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    // A byte-order mark, which some editors write, is not part of the program.
    if let Some(rest) = src.strip_prefix('\u{feff}') {
        src = rest.to_owned();
    }

    let mut interp = Interpreter::new();
    interp.stream_output();
    // Read the program's input from stdin when it is piped/redirected (the
    // batch model the `read_*` functions consume); skip a terminal so a program
    // that reads nothing does not block waiting for end-of-input.
    if !std::io::stdin().is_terminal() {
        let mut input = String::new();
        match std::io::stdin().read_to_string(&mut input) {
            Ok(_) => interp.set_input(input),
            Err(e) => {
                eprintln!("bridger: cannot read input: {e}");
                return ExitCode::FAILURE;
            }
        }
    }
    let result = interp.run(&src);
    // Output was streamed as it was printed; a final partial line is pushed
    // out before any diagnostic so the two do not interleave.
    print!("{}", interp.output());
    let _ = std::io::Write::flush(&mut std::io::stdout());
    match result {
        Ok(_) => ExitCode::SUCCESS,
        Err(err) => {
            // A partial output line gets its newline, so the diagnostic
            // starts on a line of its own.
            if interp.output_line_open() {
                eprintln!();
            }
            report(&err, &path, &src, &interp);
            ExitCode::FAILURE
        }
    }
}

/// Report a pipeline failure as a labeled excerpt of the source: the message,
/// the file and position, the offending line with the span underlined, and a
/// suggestion where the error has one.
fn report(err: &RunError, path: &str, file: &str, interp: &Interpreter) {
    let span = err.span();
    // A program that failed to parse was never stored by the interpreter, so
    // the file's own text stands in for it.
    let source = if span.src == SrcId::SYNTHETIC {
        None
    } else if span.src == SrcId(0) {
        interp
            .source(span.src)
            .map(|text| ("<prelude>".to_string(), text.to_string()))
    } else {
        Some((
            path.to_string(),
            interp.source(span.src).unwrap_or(file).to_string(),
        ))
    };
    // A very long line is shown as a window around the span, so a diagnostic
    // on a one-line program of megabytes stays readable.
    // When the span's line was cut, the header's column no longer counts
    // from the line's start, so the true position goes into the message.
    let mut position = String::new();
    let source = source.map(|(name, text)| {
        let (shown, start, end, cut) = window(&text, span.start, span.end);
        if cut {
            let (line, col) = line_col(&text, span.start);
            position = format!(" (line {line}, column {col})");
        }
        (name, shown, start, end)
    });
    let diagnostic = Diagnostic {
        message: if span.src == SrcId::SYNTHETIC {
            format!("{path}: {err}")
        } else {
            format!("{err}{position}")
        },
        help: err.help(),
        src: source
            .as_ref()
            .map(|(name, text, _, _)| NamedSource::new(name, text.clone())),
        // An empty span (the end of the input) is shown as the last character,
        // so the excerpt still points somewhere.
        at: source.map(|(_, text, start, end)| {
            let (start, len) = if end > start {
                (start, end - start)
            } else {
                (start.min(text.len().saturating_sub(1)), 1)
            };
            SourceSpan::new(start.into(), len)
        }),
    };
    eprintln!("{:?}", miette::Report::new(diagnostic));
}

/// The text to show for a span, with every line longer than
/// [`MAX_EXCERPT_LINE`] cut down: the span's line to a window around the span
/// (a span longer than the window is cut too), any other to its head — each
/// cut marked `…` — and the span moved to match. Returns the text, the moved
/// span, and whether the span's own line was cut.
fn window(text: &str, start: usize, end: usize) -> (String, usize, usize, bool) {
    let at = start.min(text.len());
    let mut shown = String::with_capacity(text.len().min(1 << 20));
    let (mut new_start, mut new_end, mut cut_span_line) = (start, end, false);
    let mut pos = 0;
    loop {
        let line_end = text[pos..].find('\n').map_or(text.len(), |i| pos + i);
        let line = &text[pos..line_end];
        let has_span = (pos..=line_end).contains(&at);
        if line.len() <= MAX_EXCERPT_LINE {
            if has_span {
                new_start = shown.len() + (at - pos);
                new_end = shown.len() + (end.clamp(at, line_end) - pos);
            }
            shown.push_str(line);
        } else if has_span {
            let margin = MAX_EXCERPT_LINE / 2;
            let mut wstart = pos.max(at.saturating_sub(margin));
            let mut wend = line_end.min(at + 2 * margin);
            while !text.is_char_boundary(wstart) {
                wstart += 1;
            }
            while !text.is_char_boundary(wend) {
                wend -= 1;
            }
            let before = if wstart > pos { "…" } else { "" };
            let after = if wend < line_end { "…" } else { "" };
            let base = shown.len() + before.len();
            new_start = base + (at.clamp(wstart, wend) - wstart);
            new_end = base + (end.clamp(wstart, wend) - wstart);
            shown.push_str(before);
            shown.push_str(&text[wstart..wend]);
            shown.push_str(after);
            cut_span_line = !before.is_empty() || !after.is_empty();
        } else {
            let mut head = pos + MAX_EXCERPT_LINE;
            while !text.is_char_boundary(head) {
                head -= 1;
            }
            shown.push_str(&text[pos..head]);
            shown.push('…');
        }
        if line_end == text.len() {
            break;
        }
        shown.push('\n');
        pos = line_end + 1;
    }
    (shown, new_start, new_end, cut_span_line)
}

/// The 1-based line and column of byte offset `at` in `text`.
fn line_col(text: &str, at: usize) -> (usize, usize) {
    let at = at.min(text.len());
    let line = text[..at].matches('\n').count() + 1;
    let col = text[..at].rfind('\n').map_or(at, |i| at - i - 1) + 1;
    (line, col)
}

/// One error, shaped for `miette`'s renderer.
#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("{message}")]
struct Diagnostic {
    message: String,
    #[help]
    help: Option<String>,
    #[source_code]
    src: Option<NamedSource<String>>,
    #[label("here")]
    at: Option<SourceSpan>,
}
