//! The stack trace in the VM format (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
//! §13.14): the address → (function, line, column) table the runtime uses
//! to write `#0      f (url:line:column)`.
//!
//! Before each call that may appear in a stack trace, the emitter puts an
//! `asm sideeffect` that generates no instruction: it defines a label in the code and
//! appends to the stack trace section a 12-byte entry,
//!
//! ```text
//! i32  label − address of the field
//! i32  function record − address of the field
//! u32  line << 12 | column        (0: unknown)
//! ```
//!
//! The return address of a call comes after its label, and
//! no other label fits between the two (the label comes right before the
//! call, after the arguments); the runtime finds the entry by the largest
//! label below the return address, within its function. The label
//! follows the instruction wherever the optimizer puts it: in a body copied by
//! inlining it goes along (the entry still names the origin function);
//! in a deleted call, an entry is left that no return finds.
//!
//! The function record is a module constant: the offset to the url
//! (a zero-terminated string, shared by the functions of the same
//! unit), a byte of marks (`hir::stack_trace_marks`) and the qualified
//! name, zero-terminated.
//!
//! **Inlined frames.** A call inside a body copied by HIR inlining
//! has one entry per frame, all with the same label, from the inner
//! function to the outer one: the copied one at the position of the call, and each outer
//! function at the position of the call that copied it (the VM shows them through the
//! inlining table). LLVM inlining does not leave that trace: with the stack trace on,
//! a direct call to a Dart function is emitted `noinline`.
//!
//! **The function group.** The two low bits of the record field (the
//! record is 4-aligned) give the kind of the entry: 0, a call
//! point; 1, the identity of a function through the uniform entry of its
//! closures (the label is the address of the entry, the word is the token); 2, the `await`
//! of each state of an `async` body, in order (the word is the position); 3, the
//! link of a closure with whoever awaits (the record field is only the kind, the
//! word is `position << 2 | direct << 1 | cell` in the environment). The runtime
//! uses them to follow the chain of awaiters and write
//! `<asynchronous suspension>` (`RT/stack_trace.rs`).
//!
//! The section, per format: `.dfpcl$m` in COFF (the linker merges `$a`, `$m` and
//! `$z` into `.dfpcl`, and the `$a`/`$z` sentinels give the start and the end); `dfpcl`
//! in ELF, with `SHF_GNU_RETAIN` (`R`) and in the group of the function (`?`), so that the
//! entry of a discarded `comdat` function goes away with it; and
//! `__DATA,__dfpcl` in Mach-O, `no_dead_strip`, with an `l…` symbol
//! per entry (`${:uid}`, unique in the object): the Mach-O writer of
//! aarch64 only relocates a subtraction whose both sides have a non-temporary
//! symbol before them in the section, and the linker does not carry the `l…` ones into the
//! executable. An anchor in `module asm` would do the same with a single symbol,
//! but a local symbol in `module asm` removes from ThinLTO importing every
//! function of the module that has `asm`.
//!
//! The text of the function is annotated after it is emitted, like the line tables
//! of J05 (`llvm/debug_info.rs`): the positions come from the
//! `  ; df.pos <line> <column>` markers.
//! Written without compiling or running (2026-10-05).

use crate::hir::{Function, FunctionDebugInfo, stack_trace_marks};
use crate::target::Os;
use std::collections::HashMap;
use std::fmt::Write as _;

/// The stack trace table of a module being emitted.
pub(super) struct StackTrace {
    /// The urls and the records of the functions, at the end of the module.
    definitions: String,
    urls: HashMap<String, usize>,
    /// The records already defined, by `(name, url, marks)`.
    records: HashMap<(String, String, u8), usize>,
}

/// A 12-byte entry of the section (see the start of the file).
struct Entry {
    /// The label expression: `42b` (the call point) or `${n:c}` (the
    /// operand `n`, a function).
    label: String,
    /// The operand of the record, or none (the field is only the kind).
    record: Option<usize>,
    /// The kind, in the two low bits of the record field.
    kind: u32,
    word: u32,
}

/// `line << 12 | column`: line up to 2²⁰ − 1 and column up to 4095; otherwise,
/// 0 (the unknown line, or only the line).
pub(super) fn line_and_column((line, column): (u32, u32)) -> u32 {
    if line == 0 || line >= 1 << 20 {
        return 0;
    }
    let column = if column < 1 << 12 { column } else { 0 };
    line << 12 | column
}

/// The target of the call on the IR line, if it is a call: `Some(Some(name))`
/// for a direct one and `Some(None)` for an indirect one. The `asm` is not a target.
pub(super) fn call_target(line: &str) -> Option<Option<&str>> {
    let mut l = line.trim_start();
    if l.starts_with('%') {
        l = &l[l.find(" = ")? + 3..];
    }
    for prefix in ["tail ", "musttail ", "notail "] {
        if let Some(r) = l.strip_prefix(prefix) {
            l = r;
        }
    }
    let rest = l
        .strip_prefix("call ")
        .or_else(|| l.strip_prefix("invoke "))?;
    // The type comes before the target and has no `@` nor `asm`; the target is the first
    // `@` or `%` (a named struct type, `%T`, counts as indirect: the
    // extra entry changes nothing).
    let ini = rest.find(['@', '%'])?;
    if rest[..ini].split_whitespace().any(|t| t == "asm") {
        return None;
    }
    if rest.as_bytes()[ini] == b'%' {
        return Some(None);
    }
    let name = &rest[ini + 1..];
    if let Some(quoted) = name.strip_prefix('"') {
        return Some(Some(&quoted[..quoted.find('"')?]));
    }
    Some(Some(&name[..name.find('(')?]))
}

/// Whether the call to `target` gets a label: the indirect one and the direct one that may throw
/// or call Dart code (the Dart function, the extern the effects table
/// marks); never an intrinsic.
fn labelable(target: Option<&str>) -> bool {
    match target {
        None => true,
        Some(name) if name.starts_with("llvm.") => false,
        Some(name) => {
            let ef = super::externs::effects_of(name);
            ef.throws || ef.calls_dart
        }
    }
}

/// The text of the `asm` of a group of entries (in IR text: `\0A` is the
/// line break, `\22` the quote and `$$` the dollar sign), with the `42:` label before it
/// when `with_label`. In Mach-O, each entry has its `l…` symbol.
fn model(entries: &[Entry], with_label: bool) -> String {
    let (section, macho) = match crate::target::target_os() {
        Os::Windows => (".pushsection \\22.dfpcl$$m\\22,\\22dr\\22", false),
        Os::Linux => (".pushsection dfpcl,\\22aR?\\22,%progbits", false),
        Os::MacOs => (".pushsection __DATA,__dfpcl,regular,no_dead_strip", true),
    };
    let mut s = String::new();
    if with_label {
        s.push_str("42:\\0A\\09");
    }
    s.push_str(section);
    s.push_str("\\0A\\09.p2align 2");
    for (i, e) in entries.iter().enumerate() {
        if macho {
            write!(s, "\\0Aldfpcl${{:uid}}_{i}:").unwrap();
        }
        write!(s, "\\0A\\09.long {}-.", e.label).unwrap();
        match e.record {
            Some(k) if e.kind == 0 => write!(s, "\\0A\\09.long ${{{k}:c}}-.").unwrap(),
            Some(k) => write!(s, "\\0A\\09.long ${{{k}:c}}-.+{}", e.kind).unwrap(),
            None => write!(s, "\\0A\\09.long {}", e.kind).unwrap(),
        }
        write!(s, "\\0A\\09.long {}", e.word).unwrap();
    }
    s.push_str("\\0A\\09.popsection");
    s
}

/// The call of the `asm` of a group, with the operands (`"s"`).
fn group_asm(entries: &[Entry], operands: &[String], with_label: bool) -> String {
    format!(
        "  call void asm sideeffect \"{}\", \"{}\"({}) nounwind memory(inaccessiblemem: readwrite) \"gc-leaf-function\"",
        model(entries, with_label),
        vec!["s"; operands.len()].join(","),
        operands.join(", ")
    )
}

/// The call with `noinline` (before the metadata and the `to label` of an
/// `invoke`).
fn with_noinline(line: &str) -> String {
    // The attribute goes before the `"deopt"` operand (roots by maps), the
    // label of an `invoke` and the metadata.
    match [
        line.find(" [ \"deopt\""),
        line.find(" to label "),
        line.find(", !"),
    ]
    .into_iter()
    .flatten()
    .min()
    {
        Some(i) => format!("{} noinline{}", &line[..i], &line[i..]),
        None => format!("{line} noinline"),
    }
}

/// The linkage of the stack trace constants: in Mach-O, local symbols (the
/// aarch64 writer does not relocate a subtraction with a temporary symbol).
fn linkage() -> &'static str {
    if crate::target::target_os() == Os::MacOs {
        "internal"
    } else {
        "private"
    }
}

impl StackTrace {
    pub(super) fn new() -> StackTrace {
        StackTrace {
            definitions: String::new(),
            urls: HashMap::new(),
            records: HashMap::new(),
        }
    }

    fn url(&mut self, url: &str) -> usize {
        if let Some(&n) = self.urls.get(url) {
            return n;
        }
        let n = self.urls.len();
        let bytes = super::selectors::llvm_bytes(url);
        writeln!(
            self.definitions,
            "@df.pcu.{n} = {} unnamed_addr constant [{} x i8] c\"{bytes}\\00\", align 1",
            linkage(),
            url.len() + 1
        )
        .unwrap();
        self.urls.insert(url.to_string(), n);
        n
    }

    /// The record of `(name, url, marks)`: the offset to the url, the byte
    /// of the marks and the name. One per module for each triple.
    fn record(&mut self, name: &str, url: &str, marks: u8) -> usize {
        if let Some(&k) = self
            .records
            .get(&(name.to_string(), url.to_string(), marks))
        {
            return k;
        }
        let u = self.url(url);
        let k = self.records.len();
        self.records
            .insert((name.to_string(), url.to_string(), marks), k);
        let n = name.len() + 1;
        writeln!(
            self.definitions,
            "@df.pcf.{k} = {} unnamed_addr constant <{{ i32, i8, [{n} x i8] }}> <{{ i32 trunc (i64 sub (i64 ptrtoint (ptr @df.pcu.{u} to i64), i64 ptrtoint (ptr @df.pcf.{k} to i64)) to i32), i8 {marks}, [{n} x i8] c\"{}\\00\" }}>, align 4",
            linkage(),
            super::selectors::llvm_bytes(name)
        )
        .unwrap();
        k
    }

    /// The records and the positions of a call point, from the inner
    /// frame (the deepest copied function) to that of the function itself.
    fn chain(
        &mut self,
        func: &Function,
        d: &FunctionDebugInfo,
        current: (u32, u32),
        context: Option<u32>,
    ) -> Vec<(usize, u32)> {
        let mut v = Vec::new();
        let (mut position, mut ctx) = (current, context);
        let mut steps = 0;
        while let Some(k) = ctx {
            let Some(e) = d.embedded_set.get(k as usize) else {
                break;
            };
            v.push((
                self.record(&e.name, &e.url, e.marks),
                line_and_column(position),
            ));
            position = e.call;
            ctx = e.parent;
            steps += 1;
            if steps > d.embedded_set.len() {
                break;
            }
        }
        v.push((
            self.record(&func.stack_trace_name(), &d.url, d.marks),
            line_and_column(position),
        ));
        v
    }

    /// The group of the function (kinds 1 to 3), through the uniform entry of its
    /// closures: `None` if it is neither a closure body nor an `async` body.
    /// The identity does not carry the hidden mark (an `async` stub that listens to a
    /// `Future` shows up, like the closure in the VM).
    fn function_group(&mut self, func: &Function, d: &FunctionDebugInfo) -> Option<String> {
        let entry = d
            .async_body
            .as_ref()
            .map(|c| c.entry.clone())
            .or_else(|| d.closure_entry.clone())?;
        let k = self.record(
            &func.stack_trace_name(),
            &d.url,
            d.marks & !stack_trace_marks::HIDDEN,
        );
        let operands = vec![format!("ptr @{entry}"), format!("ptr @df.pcf.{k}")];
        let function = "${0:c}".to_string();
        let mut entries = vec![Entry {
            label: function.clone(),
            record: Some(1),
            kind: 1,
            word: line_and_column(d.token),
        }];
        if let Some(c) = &d.async_body {
            for p in &c.waits {
                entries.push(Entry {
                    label: function.clone(),
                    record: Some(1),
                    kind: 2,
                    word: line_and_column(*p),
                });
            }
        }
        if let Some(e) = d.chain_link {
            entries.push(Entry {
                label: function,
                record: None,
                kind: 3,
                word: (e.index as u32) << 2 | u32::from(e.direct) << 1 | u32::from(e.cell),
            });
        }
        Some(group_asm(&entries, &operands, false))
    }

    /// Annotates the text of `func` (from `define` to `}`): the `asm` of the entry before
    /// each labelable call, with the position of the last marker. Without
    /// `keep_markers` (without DWARF), the markers go away. In a
    /// `shared` function (`linkonce_odr`, one copy per image) nothing is
    /// labeled and the calls are emitted `noinline`: the body of another function
    /// copied into it would carry entries into the section of a copy that the linker
    /// discards.
    pub(super) fn label(
        &mut self,
        text: &str,
        func: &Function,
        d: Option<&FunctionDebugInfo>,
        keep_markers: bool,
        shared: bool,
    ) -> String {
        // Without positions, the function has no label (its frames leave the
        // stack trace); the shared one gets the `noinline` anyway.
        let empty;
        let d = match d {
            Some(d) => d,
            None if shared => {
                empty = FunctionDebugInfo::default();
                &empty
            }
            None => return text.to_string(),
        };
        let mut exit = String::with_capacity(text.len() + text.len() / 2);
        let mut current = (d.line, 0);
        // The inlining context of the current position (`df.emb`).
        let mut context: Option<u32> = None;
        // The function group goes before the first instruction.
        let mut pending_group = !shared;
        for line in text.lines() {
            if let Some(pos) = line.strip_prefix(super::debug_info::MARKER) {
                let mut parts = pos.split(' ').filter_map(|x| x.parse::<u32>().ok());
                if let (Some(l), Some(c)) = (parts.next(), parts.next()) {
                    current = (l, c);
                }
                context = None;
                if keep_markers {
                    writeln!(exit, "{line}").unwrap();
                }
                continue;
            }
            if let Some(pos) = line.strip_prefix(super::debug_info::EMBEDDED_MARKER) {
                let mut parts = pos.split(' ').filter_map(|x| x.parse::<u32>().ok());
                if let (Some(l), Some(c), Some(k)) = (parts.next(), parts.next(), parts.next()) {
                    current = (l, c);
                    context = Some(k);
                }
                if keep_markers {
                    writeln!(exit, "{line}").unwrap();
                }
                continue;
            }
            if pending_group
                && line.starts_with("  ")
                && !line.trim_start().starts_with(';')
                && !line.trim().is_empty()
            {
                pending_group = false;
                if let Some(g) = self.function_group(func, d) {
                    writeln!(exit, "{g}").unwrap();
                }
            }
            let target = call_target(line);
            if shared {
                // `noinline` at the end of the call (before any metadata
                // or `invoke` label, which these functions do not have).
                if target.is_some_and(|a| a.is_none_or(|n| !n.starts_with("llvm.")))
                    && !line.contains(", !")
                    && !line.contains(" to label ")
                {
                    writeln!(exit, "{}", with_noinline(line)).unwrap();
                } else {
                    writeln!(exit, "{line}").unwrap();
                }
                continue;
            }
            if let Some(target) = target
                && labelable(target)
            {
                let chain = self.chain(func, d, current, context);
                let operands: Vec<String> = chain
                    .iter()
                    .map(|(k, _)| format!("ptr @df.pcf.{k}"))
                    .collect();
                let entries: Vec<Entry> = chain
                    .iter()
                    .enumerate()
                    .map(|(i, &(_, lc))| Entry {
                        label: "42b".to_string(),
                        record: Some(i),
                        kind: 0,
                        word: lc,
                    })
                    .collect();
                writeln!(exit, "{}", group_asm(&entries, &operands, true)).unwrap();
                // LLVM inlining would erase the frame of the called Dart function.
                if target.is_some_and(|n| n.starts_with("df.")) && !line.contains(" noinline") {
                    writeln!(exit, "{}", with_noinline(line)).unwrap();
                    continue;
                }
            }
            writeln!(exit, "{line}").unwrap();
        }
        exit
    }

    /// The module constants (the urls and the records of the functions).
    pub(super) fn finish(self, out: &mut String) {
        if self.records.is_empty() {
            return;
        }
        out.push_str("\n; The stack trace in the VM's format (llvm/stack_trace.rs)\n");
        out.push_str(&self.definitions);
    }
}

/// The declarations of the registration of the image table and the two operands (the
/// start and the end of the section): the `$a`/`$z` sentinels in COFF (in a `comdat`,
/// one copy per image); in ELF, the linker symbols, with an anchor that
/// guarantees the section; in Mach-O, the linker ones.
pub(super) fn markers() -> (&'static str, &'static str, &'static str) {
    match crate::target::target_os() {
        Os::Windows => (
            "$df.pcl.a = comdat any\n$df.pcl.z = comdat any\n\
             @df.pcl.a = linkonce_odr hidden constant [3 x i32] zeroinitializer, section \".dfpcl$a\", comdat, align 4\n\
             @df.pcl.z = linkonce_odr hidden constant [3 x i32] zeroinitializer, section \".dfpcl$z\", comdat, align 4\n",
            "@df.pcl.a",
            "@df.pcl.z",
        ),
        Os::Linux => (
            "$df.pcl.anchor = comdat any\n\
             @df.pcl.anchor = linkonce_odr hidden constant [3 x i32] zeroinitializer, section \"dfpcl\", comdat, align 4\n\
             @__start_dfpcl = external hidden global i8\n@__stop_dfpcl = external hidden global i8\n",
            "@__start_dfpcl",
            "@__stop_dfpcl",
        ),
        Os::MacOs => (
            "@\"\\01section$start$__DATA$__dfpcl\" = external hidden global i8\n\
             @\"\\01section$end$__DATA$__dfpcl\" = external hidden global i8\n",
            "@\"\\01section$start$__DATA$__dfpcl\"",
            "@\"\\01section$end$__DATA$__dfpcl\"",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::Type;

    fn function(symbol: &str) -> Function {
        Function {
            symbol: symbol.to_string(),
            name: symbol.rsplit('.').next().unwrap_or_default().to_string(),
            params: Vec::new(),
            return_ty: Type::I64,
            blocks: Vec::new(),
            debug_info: None,
        }
    }

    #[test]
    fn finds_the_call_target() {
        assert_eq!(
            call_target("  %v3 = call i64 @df.a.b(i64 %v1)"),
            Some(Some("df.a.b"))
        );
        assert_eq!(
            call_target("  call void @dartforge_exception_throw(i64 %v2, i8 3)"),
            Some(Some("dartforge_exception_throw"))
        );
        assert_eq!(
            call_target("  %r = tail call i64 @\"df.x$tear\"(i64 0)"),
            Some(Some("df.x$tear"))
        );
        assert_eq!(call_target("  %v9 = call i64 %f(i64 %v1)"), Some(None));
        assert_eq!(
            call_target("  call void (...) @llvm.fake.use(ptr addrspace(1) %raiz1)"),
            Some(Some("llvm.fake.use"))
        );
        assert_eq!(
            call_target("  %v4 = call i64 @df.a(i64 %v1) [ \"deopt\"(i64 %v1) ]"),
            Some(Some("df.a"))
        );
        assert_eq!(
            with_noinline("  %v4 = call i64 @df.a(i64 %v1) [ \"deopt\"(i64 %v1) ]"),
            "  %v4 = call i64 @df.a(i64 %v1) noinline [ \"deopt\"(i64 %v1) ]"
        );
        assert_eq!(
            call_target("  %r.ok = invoke i64 @g(i64 1) to label %a unwind label %b"),
            Some(Some("g"))
        );
        assert_eq!(
            call_target("  call void asm sideeffect \"42:\", \"s\"(ptr @df.pcf.0) nounwind"),
            None
        );
        assert_eq!(call_target("  %v4 = add i64 %v1, %v2"), None);
        assert_eq!(call_target("  store i64 %v1, ptr @g"), None);
    }

    #[test]
    fn packs_line_and_column() {
        assert_eq!(line_and_column((3, 5)), 3 << 12 | 5);
        assert_eq!(line_and_column((3, 5000)), 3 << 12);
        assert_eq!(line_and_column((0, 5)), 0);
        assert_eq!(line_and_column((1 << 20, 1)), 0);
    }

    #[test]
    fn labels_calls_with_marker_position() {
        let func = function("df.lib.f");
        let d = FunctionDebugInfo {
            file: "/x/a.dart".to_string(),
            url: "file:///x/a.dart".to_string(),
            line: 2,
            ..Default::default()
        };
        let text = "define i64 @df.lib.f() {\nb0:\n  ; df.pos 3 7\n  %v1 = call i64 @df.lib.g()\n  %v2 = add i64 %v1, 1\n  ret i64 %v2\n}\n";
        let mut r = StackTrace::new();
        let exit = r.label(text, &func, Some(&d), false, false);
        assert!(!exit.contains("df.pos"), "{exit}");
        assert!(exit.contains(&format!(".long {}", 3 << 12 | 7)), "{exit}");
        let asm = exit.find("asm sideeffect").expect("label");
        assert!(asm < exit.find("@df.lib.g()").expect("call"), "{exit}");
        let mut out = String::new();
        r.finish(&mut out);
        assert!(
            out.contains("@df.pcu.0 ") && out.contains("c\"file:\\2F\\2F\\2Fx\\2Fa.dart\\00\""),
            "{out}"
        );
        assert!(
            out.contains("@df.pcf.0 ") && out.contains("i8 0, [2 x i8] c\"f\\00\""),
            "{out}"
        );
        // A direct call to a Dart function does not get LLVM inlining.
        assert!(exit.contains("@df.lib.g() noinline"), "{exit}");
    }

    #[test]
    fn embedded_frames_have_one_entry_per_function() {
        let func = function("df.lib.f");
        let d = FunctionDebugInfo {
            url: "file:///x/a.dart".to_string(),
            line: 2,
            embedded_set: vec![crate::hir::Embedded {
                name: "g".to_string(),
                url: "file:///x/b.dart".to_string(),
                call: (3, 7),
                parent: None,
                marks: 0,
            }],
            ..Default::default()
        };
        let text = "define i64 @df.lib.f() {\nb0:\n  ; df.pos 3 7\n  ; df.emb 9 4 0\n  %v1 = call i64 @df.lib.h()\n  ret i64 %v1\n}\n";
        let mut r = StackTrace::new();
        let exit = r.label(text, &func, Some(&d), false, false);
        assert!(
            exit.contains(&format!(".long {}", 9 << 12 | 4))
                && exit.contains(&format!(".long {}", 3 << 12 | 7)),
            "{exit}"
        );
        assert!(
            exit.contains("\"s,s\"(ptr @df.pcf.0, ptr @df.pcf.1)"),
            "{exit}"
        );
        assert!(!exit.contains("df.emb"), "{exit}");
    }

    #[test]
    fn closure_and_async_body_group() {
        let func = function("df.lib.f$async$q0000abcd");
        let d = FunctionDebugInfo {
            url: "file:///x/a.dart".to_string(),
            line: 2,
            token: (1, 6),
            marks: stack_trace_marks::ASYNC_BODY,
            async_body: Some(crate::hir::TraceAsyncBody {
                entry: "df.lib.f$async$q0000abcd$ent".to_string(),
                waits: vec![(3, 9), (4, 11)],
            }),
            ..Default::default()
        };
        let text = "define i64 @df.lib.f$async$q0000abcd() {\nb0:\n  %v1 = add i64 1, 2\n  ret i64 %v1\n}\n";
        let mut r = StackTrace::new();
        let exit = r.label(text, &func, Some(&d), false, false);
        let g = exit.find("asm sideeffect").expect("grupo");
        assert!(g < exit.find("%v1 = add").expect("instruction"), "{exit}");
        assert!(
            exit.contains("ptr @df.lib.f$async$q0000abcd$ent, ptr @df.pcf.0"),
            "{exit}"
        );
        assert!(
            exit.contains(&format!(".long {}", 3 << 12 | 9)) && exit.contains("-.+2"),
            "{exit}"
        );
        assert_eq!(func.stack_trace_name(), "f");
        assert_eq!(
            function("dart_main$clo0$e1234abcd").stack_trace_name(),
            "main.<anonymous closure>"
        );
        assert_eq!(
            function("df.lib.C.m$inner$e0000ffff").stack_trace_name(),
            "C.m.inner"
        );
    }

    #[test]
    fn shared_function_does_not_label_and_gets_no_inlining() {
        let func = function("df.lib.t$tear");
        let d = FunctionDebugInfo {
            url: "file:///x/a.dart".to_string(),
            line: 1,
            ..Default::default()
        };
        let text = "define linkonce_odr i64 @\"df.lib.t$tear\"() {\nb0:\n  %v1 = call i64 @df.lib.t()\n  ret i64 %v1\n}\n";
        let mut r = StackTrace::new();
        let exit = r.label(text, &func, Some(&d), false, true);
        assert!(!exit.contains("asm"), "{exit}");
        assert!(
            exit.contains("%v1 = call i64 @df.lib.t() noinline"),
            "{exit}"
        );
    }
}
