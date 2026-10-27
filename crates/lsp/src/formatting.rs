//! Document formatting: `textDocument/formatting`,
//! `textDocument/rangeFormatting` and `textDocument/onTypeFormatting`, like
//! the Dart 3.6.2 server's `FormattingHandler`, `FormatRangeHandler` and
//! `FormatOnTypeHandler` (`generateEditsForFormatting`,
//! `lsp/source_edits.dart`).
//!
//! The text is formatted by the `dart_style` port ([`dartforge_formatter`]);
//! the response is the minimal whitespace edits between tokens
//! ([`dartforge_formatter::edits::minimal_edits`]). Without changes, or
//! with a syntax error, the response is `null`.
//!
//! Formatting cannot be interrupted halfway, and pathological inputs (an
//! argument list with thousands of arguments) take seconds, as they do in
//! `dart format`. So the server runs each formatting on its own thread
//! ([`start`]): when it does not finish within a short budget the request is
//! answered later, while other messages keep being dispatched, and a
//! `$/cancelRequest` answers it at once (the result is discarded).

use dartforge_formatter::edits::minimal_edits;
use dartforge_formatter::{FormatOptions, format};
use serde_json::{Value, json};

use std::sync::mpsc;
use std::time::Duration;

use crate::utf16::LineTable;

/// Stack of a formatting thread: the parser and the formatter recurse over
/// the syntax tree, as on the dispatcher thread (address space reserved, not
/// memory used).
const STACK: usize = 1 << 30;

/// A formatting still running on its own thread.
#[derive(Debug)]
pub(crate) struct Background {
    receiver: mpsc::Receiver<Value>,
}

impl Background {
    /// The result once the thread is done: `Some(None)` if it panicked,
    /// `None` while it is still running.
    pub(crate) fn poll(&self) -> Option<Option<Value>> {
        match self.receiver.try_recv() {
            Ok(v) => Some(Some(v)),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(None),
        }
    }
}

/// What [`start`] gives back.
#[derive(Debug)]
pub(crate) enum Outcome {
    /// The [`edits`] (`None` when the computation panicked).
    Ready(Option<Value>),
    /// Still running after the budget; `wake` is called when it finishes.
    Running(Background),
}

/// Computes [`edits`] on a thread of its own and waits for it up to
/// `budget` (`None`: until it finishes; zero: not at all). `wake` is called
/// from that thread once the result is available.
pub(crate) fn start(
    text: String,
    width: Option<u64>,
    range: Option<(usize, usize)>,
    budget: Option<Duration>,
    wake: Option<crate::typed::Wake>,
) -> Outcome {
    let (sender, receiver) = mpsc::channel();
    let text = std::sync::Arc::new(text);
    let shared = std::sync::Arc::clone(&text);
    let spawned = std::thread::Builder::new()
        .name("format".to_string())
        .stack_size(STACK)
        .spawn(move || {
            let result = edits(&shared, width, range);
            // The request may have been cancelled meanwhile: nobody listens.
            let _ = sender.send(result);
            if let Some(w) = wake {
                (w.0)();
            }
        });
    if spawned.is_err() {
        // No thread: formats here, blocking, as before.
        return Outcome::Ready(Some(edits(&text, width, range)));
    }
    let received = match budget {
        None => receiver
            .recv()
            .map_err(|_| mpsc::RecvTimeoutError::Disconnected),
        Some(b) if b.is_zero() => Err(mpsc::RecvTimeoutError::Timeout),
        Some(b) => receiver.recv_timeout(b),
    };
    match received {
        Ok(v) => Outcome::Ready(Some(v)),
        Err(mpsc::RecvTimeoutError::Disconnected) => Outcome::Ready(None),
        Err(mpsc::RecvTimeoutError::Timeout) => Outcome::Running(Background { receiver }),
    }
}

/// The first character that triggers formatting while typing, and the others
/// (`dartTypeFormattingCharacters`, `constants.dart:46`).
pub const TRIGGER_CHARACTERS: [&str; 2] = ["}", ";"];

/// The formatting edits of `text`, as JSON (`List<TextEdit>?`).
///
/// `width` is the client's `dart.lineLength` (80 without it). `range`, in
/// bytes, restricts the result to the edits entirely inside it
/// (`rangeFormatting`).
///
/// ```
/// let e = dartforge_lsp::formatting::edits("main( ){}", None, None);
/// assert_eq!(e.as_array().unwrap().len(), 3);
/// assert!(dartforge_lsp::formatting::edits("main() {}\n", None, None).is_null());
/// ```
pub fn edits(text: &str, width: Option<u64>, range: Option<(usize, usize)>) -> Value {
    let options = FormatOptions {
        page_width: width.map_or(80, |l| l as usize),
        ..FormatOptions::default()
    };
    let Ok(Ok(formatted)) = std::panic::catch_unwind(|| format(text, &options)) else {
        return Value::Null;
    };
    if formatted == text {
        return Value::Null;
    }
    let table = LineTable::build(text);
    // The edits come in increasing offset order: a sequential cursor keeps
    // the conversion linear on long lines.
    let mut cursor = table.cursor(text);
    let mut position = |o: usize| {
        let (l, c) = cursor.position(o);
        json!({"line": l, "character": c})
    };
    let list: Vec<Value> = minimal_edits(text, &formatted, range)
        .into_iter()
        .map(|e| json!({"range": {"start": position(e.start), "end": position(e.end)}, "newText": e.text}))
        .collect();
    json!(list)
}

/// The Dart server's `toOffset(lineInfo, position)`, in bytes: `None` for a
/// line past the last one; the column (UTF-16) is added to the line start
/// without stopping at the line break, saturating at the end of the text.
///
/// ```
/// use dartforge_lsp::formatting::offset_without_limit;
/// assert_eq!(offset_without_limit("ab\ncd\n", 0, 4), Some(4));
/// assert_eq!(offset_without_limit("ab\ncd\n", 3, 0), None);
/// ```
pub fn offset_without_limit(text: &str, line: u64, column: u64) -> Option<usize> {
    let table = LineTable::build(text);
    if line as usize >= table.total_lines() {
        return None;
    }
    let mut o = table.line_start(line as usize);
    let mut remaining = column as usize;
    for c in text[o..].chars() {
        if remaining == 0 {
            break;
        }
        remaining = remaining.saturating_sub(c.len_utf16());
        o += c.len_utf8();
    }
    Some(o)
}
