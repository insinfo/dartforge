//! Source map (source map v3), as dart-sass's `SourceMapBuffer`
//! (`lib/src/util/source_map_buffer.dart`) builds it and the `SingleMapping`
//! of the `source_maps` 0.10 package writes it.
//!
//! The serializer writes into a `Vec<u8>`; the [`SourceMapBuffer`] follows
//! that text from behind (`sync`), counting lines and columns in UTF-16
//! units — those of a Dart `String` —, and stores the entries at the same
//! points where `SourceMapBuffer` stores them: at the start of each
//! `forSpan` and at the start of each line written inside one.
use std::collections::HashMap;

use codemap::{CodeMap, Span};

use crate::sass_builder::{map_source, Ativo};

/// A point of the map: source (URL, line, column) → target (line, column,
/// offset). Lines and columns start at 0; columns count UTF-16 units. The
/// target offset is in bytes (it only serves for comparisons).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Entry {
    pub source: usize,
    pub source_line: usize,
    pub source_column: usize,
    pub target_line: usize,
    pub target_column: usize,
    pub target_offset: usize,
}

/// The tracking of the written text, and the entries.
#[derive(Debug, Default)]
pub(crate) struct SourceMapBuffer {
    entries: Vec<Entry>,
    urls: Vec<String>,
    indices: HashMap<String, usize>,
    line: usize,
    column: usize,
    scanned: usize,
    in_span: bool,
}

impl SourceMapBuffer {
    /// Processes what the serializer wrote since the last time: each `\n`
    /// is the `_writeLine` of `SourceMapBuffer`.
    pub fn sync(&mut self, buffer: &[u8]) {
        while self.scanned < buffer.len() {
            let b = buffer[self.scanned];
            self.scanned += 1;
            if b == b'\n' {
                self.write_line();
            } else if b & 0xC0 != 0x80 {
                // Start of a UTF-8 character: 4 bytes are a surrogate pair
                // in UTF-16.
                self.column += if b >= 0xF0 { 2 } else { 1 };
            }
        }
    }

    /// `_writeLine`: drops the entry left at the end of the line and, inside
    /// a `span`, opens the new line pointing to the same source.
    fn write_line(&mut self) {
        if let Some(u) = self.entries.last() {
            if u.target_line == self.line && u.target_column == self.column {
                self.entries.pop();
            }
        }
        self.line += 1;
        self.column = 0;
        if self.in_span {
            if let Some(u) = self.entries.last().copied() {
                self.entries.push(Entry {
                    target_line: self.line,
                    target_column: 0,
                    target_offset: self.scanned,
                    ..u
                });
            }
        }
    }

    /// Start of a `forSpan(span, …)`: `_addEntry(span.start, target)`.
    /// Returns the previous `_inSpan`, for [`SourceMapBuffer::end_span`].
    pub fn start_span(&mut self, buffer: &[u8], map: &CodeMap, span: Span) -> bool {
        self.sync(buffer);
        let (source, source_line, source_column) = self.location(map, span);
        let new = Entry {
            source,
            source_line,
            source_column,
            target_line: self.line,
            target_column: self.column,
            target_offset: self.scanned,
        };
        let redundant = self.entries.last().is_some_and(|u| {
            // Same source line and same target line (the file does not take
            // part in dart-sass's comparison), or the same target point.
            (u.source_line == new.source_line && u.target_line == new.target_line)
                || u.target_offset == new.target_offset
        });
        if !redundant {
            self.entries.push(new);
        }
        std::mem::replace(&mut self.in_span, true)
    }

    /// End of the `forSpan`: the text written inside it is processed with
    /// `_inSpan` on, and the previous state comes back.
    pub fn end_span(&mut self, buffer: &[u8], previous: bool) {
        self.sync(buffer);
        self.in_span = previous;
    }

    /// The URL, the line and the column (UTF-16) of the start of `span`.
    fn location(&mut self, map: &CodeMap, span: Span) -> (usize, usize, usize) {
        let file = map.find_file(span.low());
        let line = file.find_line(span.low());
        let line_start = file.line_span(line).low();
        let text = file.source_slice(
            file.span
                .subspan(line_start - file.span.low(), span.low() - file.span.low()),
        );
        let column = text.encode_utf16().count();
        let name = file.name().to_owned();
        let n = self.urls.len();
        let i = *self.indices.entry(name.clone()).or_insert(n);
        if i == n {
            self.urls.push(name);
        }
        (i, line, column)
    }

    /// The map with the prefix (`@charset` or BOM) applied, like
    /// `buildSourceMap(prefix:)`.
    pub fn build(mut self, buffer: &[u8], prefix: &str) -> SourceMap {
        self.sync(buffer);
        let lines = prefix.matches('\n').count();
        let column = match prefix.rfind('\n') {
            Some(i) => prefix[i + 1..].encode_utf16().count(),
            None => prefix.encode_utf16().count(),
        };
        let mut entries = self.entries;
        for e in &mut entries {
            if e.target_line == 0 {
                e.target_column += column;
            }
            e.target_line += lines;
        }
        SourceMap {
            urls: self.urls,
            entries,
        }
    }
}

/// The finished map: the canonical URLs of the sources and the points.
#[derive(Debug, Clone, Default)]
pub struct SourceMap {
    pub(crate) urls: Vec<String>,
    pub(crate) entries: Vec<Entry>,
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn vlq(output: &mut String, value: i64) {
    let mut v = if value < 0 {
        ((-value) << 1) | 1
    } else {
        value << 1
    };
    loop {
        let mut digit = (v & 31) as usize;
        v >>= 5;
        if v > 0 {
            digit |= 32;
        }
        output.push(BASE64[digit] as char);
        if v == 0 {
            break;
        }
    }
}

/// `json.encode` of a Dart `String`.
fn json_string(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            '\u{8}' => o.push_str("\\b"),
            '\u{c}' => o.push_str("\\f"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

impl SourceMap {
    /// The JSON of the map (`SingleMapping.fromEntries(...).toJson()`), with
    /// each source URL passed through `source`.
    pub fn json(&self, source: impl Fn(&str) -> String) -> String {
        // `fromEntries`: the entries in target order; the URLs in the order
        // they appear.
        let mut entries = self.entries.clone();
        entries.sort_by_key(|e| e.target_offset);
        let mut order: Vec<usize> = Vec::new();
        let mut new_index: HashMap<usize, usize> = HashMap::new();
        for e in &entries {
            if !new_index.contains_key(&e.source) {
                new_index.insert(e.source, order.len());
                order.push(e.source);
            }
        }
        let mut m = String::new();
        let (mut line, mut column, mut fl, mut fc, mut fu) = (0usize, 0i64, 0i64, 0i64, 0i64);
        let mut first = true;
        for e in &entries {
            if e.target_line > line {
                for _ in line..e.target_line {
                    m.push(';');
                }
                line = e.target_line;
                column = 0;
                first = true;
            }
            if !first {
                m.push(',');
            }
            first = false;
            vlq(&mut m, e.target_column as i64 - column);
            column = e.target_column as i64;
            let u = new_index[&e.source] as i64;
            vlq(&mut m, u - fu);
            fu = u;
            vlq(&mut m, e.source_line as i64 - fl);
            fl = e.source_line as i64;
            vlq(&mut m, e.source_column as i64 - fc);
            fc = e.source_column as i64;
        }
        let sources: Vec<String> = order
            .iter()
            .map(|&i| json_string(&source(&self.urls[i])))
            .collect();
        format!(
            "{{\"version\":3,\"sourceRoot\":\"\",\"sources\":[{}],\"names\":[],\"mappings\":{}}}",
            sources.join(","),
            json_string(&m)
        )
    }

    /// The JSON of the `.css.map` that `sass_builder` writes.
    pub fn json_sass_builder(&self, input: &Ativo) -> String {
        self.json(|u| map_source(u, input))
    }
}
