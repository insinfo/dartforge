//! Debug information (J05): the DWARF line tables (CodeView on
//! Windows) from the positions the lowering recorded in each function
//! (`hir::FunctionDebugInfo`).
//!
//! Each function with positions gets a `DISubprogram` (Dart name and symbol) and
//! each of its instructions a `!dbg` with the `DILocation` of the statement that generated it.
//! Before the instruction whose position changes, the emitter writes the line
//! `  ; df.pos <line> <column>`; at the end of the function, [`DebugInfo::annotate`]
//! replaces those markers with the `!dbg` of every instruction (one without a
//! position inherits the previous one; the prologue, that of the declaration). Annotating the
//! already emitted text also covers the auxiliary instructions (coercions, roots frame,
//! exception check) without touching every emission point, and LLVM requires
//! `!dbg` on every call of a function with a `DISubprogram`.
//!
//! The result is what a native debugger needs: stop at
//! `file.dart:line` and show the stack with the Dart names and lines
//! (`gdb`, `lldb`, the Visual Studio debugger). Dart variables and types are not
//! described; the unit is `FullDebug` because only there LLVM emits the
//! `DW_TAG_subprogram` of each function (with `LineTablesOnly`, the stack shows
//! the link symbol).

use crate::hir::{FunctionDebugInfo, Function};
use std::collections::HashMap;
use std::fmt::Write as _;

/// The prefix of the position marker in the text of a function.
pub(super) const MARKER: &str = "  ; df.pos ";
/// The position marker of an instruction copied by HIR inlining:
/// `line column context` (the index into `FunctionDebugInfo::embedded_set`). Only the
/// stack trace reads it (`llvm/stack_trace.rs`); for the line tables it is a comment.
pub(super) const EMBEDDED_MARKER: &str = "  ; df.emb ";

/// The debug metadata of the module, numbered in creation order.
pub(super) struct DebugInfo {
    /// `!N = …`, in number order.
    metadata: Vec<String>,
    files: HashMap<String, u32>,
    locals: HashMap<(u32, u32, u32), u32>,
    /// The file of the compilation unit: that of the first annotated function.
    unit_file: Option<u32>,
}

/// The number of the `DICompileUnit` and of the (empty) subroutine type, reserved.
const UNIT: u32 = 0;
const TYPE: u32 = 1;

impl DebugInfo {
    pub(super) fn new_one() -> DebugInfo {
        DebugInfo {
            // The first two are filled in by `finalize`.
            metadata: vec![String::new(), "!DISubroutineType(types: !{})".to_string()],
            files: HashMap::new(),
            locals: HashMap::new(),
            unit_file: None,
        }
    }

    fn new(&mut self, text: String) -> u32 {
        self.metadata.push(text);
        (self.metadata.len() - 1) as u32
    }

    fn file(&mut self, path: &str) -> u32 {
        if let Some(&n) = self.files.get(path) {
            return n;
        }
        let p = std::path::Path::new(path);
        let name = p.file_name().map_or_else(|| path.to_string(), |n| n.to_string_lossy().into_owned());
        let dir = p.parent().map(|d| d.display().to_string()).unwrap_or_default();
        let n = self.new(format!("!DIFile(filename: \"{}\", directory: \"{}\")", ir_text(&name), ir_text(&dir)));
        self.files.insert(path.to_string(), n);
        self.unit_file.get_or_insert(n);
        n
    }

    fn local(&mut self, (line, column): (u32, u32), scope: u32) -> u32 {
        if let Some(&n) = self.locals.get(&(line, column, scope)) {
            return n;
        }
        let n = self.new(format!("!DILocation(line: {line}, column: {column}, scope: !{scope})"));
        self.locals.insert((line, column, scope), n);
        n
    }

    /// Annotates the text of `func` (from `define` to `}`): the `DISubprogram` on the
    /// definition and the `!dbg` on each instruction; the markers go away.
    pub(super) fn annotate(&mut self, text: &str, func: &Function, d: &FunctionDebugInfo) -> String {
        let file = self.file(&d.file);
        let sp = self.new(format!(
            "distinct !DISubprogram(name: \"{}\", scope: !{file}, file: !{file}, line: {}, \
             type: !{TYPE}, scopeLine: {}, spFlags: DISPFlagDefinition, unit: !{UNIT})",
            ir_text(&dart_name(func)),
            d.line,
            d.line,
        ));
        let mut current = self.local((d.line, 1), sp);
        let mut exit = String::with_capacity(text.len() + text.len() / 4);
        for line in text.lines() {
            if line.starts_with("define ") {
                match line.strip_suffix(" {") {
                    Some(head) => writeln!(exit, "{head} !dbg !{sp} {{").unwrap(),
                    None => writeln!(exit, "{line}").unwrap(),
                }
            } else if let Some(pos) = line.strip_prefix(MARKER) {
                let mut parts = pos.split(' ').filter_map(|x| x.parse::<u32>().ok());
                if let (Some(l), Some(c)) = (parts.next(), parts.next()) {
                    current = self.local((l, c), sp);
                }
            } else if line.starts_with("  ") && !line.starts_with("  ;") && !line.trim().is_empty() {
                writeln!(exit, "{line}, !dbg !{current}").unwrap();
            } else {
                writeln!(exit, "{line}").unwrap();
            }
        }
        exit
    }

    /// The module metadata (nothing, if no function was annotated).
    pub(super) fn finish(mut self, out: &mut String) {
        let Some(file) = self.unit_file else { return };
        self.metadata[UNIT as usize] = format!(
            "distinct !DICompileUnit(language: DW_LANG_C99, file: !{file}, producer: \"dartforge\", \
             isOptimized: false, runtimeVersion: 0, emissionKind: FullDebug)"
        );
        let version = self.new(match crate::target::target_os() {
            crate::target::Os::Windows => "!{i32 2, !\"CodeView\", i32 1}".to_string(),
            // Xcode's `dsymutil` and `lldb` read DWARF 4 without caveats.
            crate::target::Os::MacOs => "!{i32 7, !\"Dwarf Version\", i32 4}".to_string(),
            crate::target::Os::Linux => "!{i32 7, !\"Dwarf Version\", i32 5}".to_string(),
        });
        let format = self.new("!{i32 2, !\"Debug Info Version\", i32 3}".to_string());
        out.push_str("\n; Informação de depuração (J05)\n");
        writeln!(out, "!llvm.dbg.cu = !{{!{UNIT}}}").unwrap();
        writeln!(out, "!llvm.module.flags = !{{!{version}, !{format}}}").unwrap();
        for (n, m) in self.metadata.iter().enumerate() {
            writeln!(out, "!{n} = {m}").unwrap();
        }
    }
}

/// The name the debugger shows: the qualified Dart one (`Account.deposit`,
/// `double_of`), taken from the stable symbol `df.<library>.<class>.<member>`
/// (the library comes escaped, without dots); without the prefix, the HIR one. The
/// symbol stays in the symbol table of the executable.
pub(super) fn dart_name(func: &Function) -> String {
    if func.symbol == "dart_main" {
        return "main".to_string();
    }
    func.symbol
        .strip_prefix("df.")
        .and_then(|rest| rest.split_once('.'))
        .map(|(_, name)| name.trim_start_matches('.').to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| func.name.clone())
}

/// Quoted text in the IR: `"`, `\` and anything that is not visible ASCII as `\XX`.
fn ir_text(s: &str) -> String {
    let mut r = String::with_capacity(s.len());
    for b in s.bytes() {
        if b == b'"' || b == b'\\' || !(0x20..0x7f).contains(&b) {
            write!(r, "\\{b:02X}").unwrap();
        } else {
            r.push(b as char);
        }
    }
    r
}
