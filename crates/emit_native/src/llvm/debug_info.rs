//! Informação de depuração (J05): as tabelas de linha do DWARF (CodeView no
//! Windows) a partir das posições que o lowering registrou em cada função
//! (`hir::FunctionDebugInfo`).
//!
//! Cada função com posições ganha um `DISubprogram` (nome Dart e símbolo) e
//! cada instrução dela um `!dbg` com a `DILocation` do comando que a gerou.
//! O emissor escreve, antes da instrução cuja posição muda, a linha
//! `  ; df.pos <line> <column>`; ao fim da função, [`DebugInfo::annotate`]
//! troca esses marcadores pelos `!dbg` de todas as instruções (a que não tem
//! posição herda a anterior; o prólogo, a da declaração). Anotar o texto já
//! emitido cobre também as instruções auxiliares (coerções, quadro de raízes,
//! conferência de exceção) sem tocar em cada ponto de emissão, e o LLVM exige
//! o `!dbg` em toda chamada de uma função com `DISubprogram`.
//!
//! O resultado é o que um depurador nativo precisa: parar em
//! `file.dart:line` e mostrar a pilha com os nomes e as linhas Dart
//! (`gdb`, `lldb`, o depurador do Visual Studio). Variáveis e tipos Dart não
//! são descritos; a unidade é `FullDebug` porque só nela o LLVM emite o
//! `DW_TAG_subprogram` de cada função (com `LineTablesOnly`, a pilha mostra
//! o símbolo de ligação).

use crate::hir::{FunctionDebugInfo, Function};
use std::collections::HashMap;
use std::fmt::Write as _;

/// O prefixo do marcador de posição no texto de uma função.
pub(super) const MARKER: &str = "  ; df.pos ";
/// O marcador da posição de uma instrução copiada pelo inlining da HIR:
/// `line column context` (o índice em `FunctionDebugInfo::embedded_set`). Só o
/// rastro o lê (`llvm/stack_trace.rs`); para as tabelas de linha é comentário.
pub(super) const EMBEDDED_MARKER: &str = "  ; df.emb ";

/// Os metadados de depuração do módulo, numerados na ordem de criação.
pub(super) struct DebugInfo {
    /// `!N = …`, na ordem dos números.
    metadata: Vec<String>,
    files: HashMap<String, u32>,
    locals: HashMap<(u32, u32, u32), u32>,
    /// O arquivo da unidade de compilação: o da primeira função anotada.
    unit_file: Option<u32>,
}

/// O número da `DICompileUnit` e do tipo de sub-rotina (vazio), reservados.
const UNIT: u32 = 0;
const TYPE: u32 = 1;

impl DebugInfo {
    pub(super) fn new_one() -> DebugInfo {
        DebugInfo {
            // Os dois primeiros são preenchidos em `finalizar`.
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

    /// Anota o texto de `func` (de `define` ao `}`): o `DISubprogram` na
    /// definição e o `!dbg` em cada instrução; os marcadores saem.
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

    /// Os metadados do módulo (nada, se nenhuma função foi anotada).
    pub(super) fn finalizar(mut self, out: &mut String) {
        let Some(file) = self.unit_file else { return };
        self.metadata[UNIT as usize] = format!(
            "distinct !DICompileUnit(language: DW_LANG_C99, file: !{file}, producer: \"dartforge\", \
             isOptimized: false, runtimeVersion: 0, emissionKind: FullDebug)"
        );
        let version = self.new(match crate::alvo::sistema() {
            crate::alvo::Sistema::Windows => "!{i32 2, !\"CodeView\", i32 1}".to_string(),
            // O `dsymutil` e o `lldb` do Xcode leem o DWARF 4 sem ressalvas.
            crate::alvo::Sistema::MacOs => "!{i32 7, !\"Dwarf Version\", i32 4}".to_string(),
            crate::alvo::Sistema::Linux => "!{i32 7, !\"Dwarf Version\", i32 5}".to_string(),
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

/// O nome que o depurador mostra: o qualificado do Dart (`Conta.depositar`,
/// `double_of`), tirado do símbolo estável `df.<library>.<class>.<member>`
/// (a biblioteca vem escapada, sem pontos); sem o prefixo, o da HIR. O
/// símbolo continua na tabela de símbolos do executável.
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

/// Texto entre aspas no IR: `"` e `\` e o que não é ASCII visível como `\XX`.
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
