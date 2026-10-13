//! O rastro no formato da VM (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
//! §13.14): a tabela endereço → (função, linha, coluna) que o runtime usa
//! para escrever `#0      f (url:line:column)`.
//!
//! Antes de cada chamada que pode aparecer num rastro, o emissor põe um
//! `asm sideeffect` que não gera instrução: define um rótulo no código e
//! acrescenta à seção do rastro uma entrada de 12 bytes,
//!
//! ```text
//! i32  rótulo − endereço do campo
//! i32  registro da função − endereço do campo
//! u32  linha << 12 | coluna        (0: desconhecida)
//! ```
//!
//! O endereço de retorno de uma chamada fica depois do rótulo dela, e
//! nenhum outro rótulo cabe entre os dois (o rótulo vem logo antes da
//! chamada, depois dos argumentos); o runtime acha a entrada pelo maior
//! rótulo abaixo do endereço de retorno, dentro da função dele. O rótulo
//! segue a instrução onde o otimizador a puser: num corpo copiado pelo
//! inlining ele vai junto (a entrada continua dizendo a função de origem);
//! numa chamada apagada, sobra uma entrada que nenhum retorno acha.
//!
//! O registro da função é uma constante do módulo: o deslocamento até a url
//! (um texto terminado em zero, compartilhado pelas funções da mesma
//! unidade), um byte de marcas (`hir::stack_trace_marks`) e o nome
//! qualificado, terminado em zero.
//!
//! **Quadros embutidos.** A chamada dentro de um corpo copiado pelo inlining
//! da HIR tem uma entrada por quadro, todas com o mesmo rótulo, da função de
//! dentro para a de fora: a copiada na posição da chamada, e cada função de
//! fora na posição da chamada que a copiou (a VM os mostra pela tabela de
//! inlining). O inlining do LLVM não deixa esse rastro: com o rastro ligado,
//! a chamada direta a uma função Dart sai `noinline`.
//!
//! **O grupo da função.** Os dois bits baixos do campo do registro (o
//! registro é alinhado em 4) dizem a espécie da entrada: 0, um ponto de
//! chamada; 1, a identidade de uma função pela entrada uniforme das closures
//! dela (o rótulo é o endereço da entrada, a palavra é o token); 2, o `await`
//! de cada estado de um corpo `async`, na ordem (a palavra é a posição); 3, o
//! elo de uma closure com quem espera (o campo do registro é só a espécie, a
//! palavra é `posição << 2 | direct << 1 | célula` no ambiente). O runtime
//! os usa para seguir a cadeia de quem espera e escrever
//! `<asynchronous suspension>` (`RT/stack_trace.rs`).
//!
//! A seção, por formato: `.dfpcl$m` no COFF (o ligador junta `$a`, `$m` e
//! `$z` em `.dfpcl`, e as sentinelas `$a`/`$z` dão o começo e o fim); `dfpcl`
//! no ELF, com `SHF_GNU_RETAIN` (`R`) e no grupo da função (`?`), para que a
//! entrada de uma função `comdat` descartada saia com ela; e
//! `__DATA,__dfpcl` no Mach-O, `no_dead_strip`, com um símbolo
//! `l…` por entrada (`${:uid}`, único no objeto): o escritor Mach-O do
//! aarch64 só reloca a subtração cujos dois lados têm um símbolo não
//! temporário antes deles na seção, e o ligador não leva os `l…` ao
//! executável. Uma âncora em `module asm` faria o mesmo com um símbolo só,
//! mas um símbolo local em `module asm` tira da importação do ThinLTO toda
//! função do módulo que tem `asm`.
//!
//! O texto da função é anotado depois de emitido, como as tabelas de linha
//! do J05 (`llvm/debug_info.rs`): as posições vêm dos marcadores
//! `  ; df.pos <line> <column>`.
//! Escrito sem compilar nem executar (2026-10-05).

use crate::alvo::Sistema;
use crate::hir::{FunctionDebugInfo, Function, stack_trace_marks};
use std::collections::HashMap;
use std::fmt::Write as _;

/// A tabela do rastro de um módulo em emissão.
pub(super) struct StackTrace {
    /// As urls e os registros das funções, no fim do módulo.
    definitions: String,
    urls: HashMap<String, usize>,
    /// Os registros já definidos, por `(name, url, marks)`.
    records: HashMap<(String, String, u8), usize>,
}

/// Uma entrada de 12 bytes da seção (veja o começo do arquivo).
struct Entry {
    /// A expressão do rótulo: `42b` (o ponto de chamada) ou `${n:c}` (o
    /// operando `n`, uma função).
    label: String,
    /// O operando do registro, ou nenhum (o campo é só a espécie).
    record: Option<usize>,
    /// A espécie, nos dois bits baixos do campo do registro.
    kind: u32,
    word: u32,
}

/// `line << 12 | column`: linha até 2²⁰ − 1 e coluna até 4095; fora disso,
/// 0 (a linha desconhecida, ou só a linha).
pub(super) fn line_and_column((line, column): (u32, u32)) -> u32 {
    if line == 0 || line >= 1 << 20 {
        return 0;
    }
    let column = if column < 1 << 12 { column } else { 0 };
    line << 12 | column
}

/// O alvo da chamada da linha de IR, se ela é uma chamada: `Some(Some(name))`
/// para a direta e `Some(None)` para a indireta. O `asm` não é alvo.
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
    let rest = l.strip_prefix("call ").or_else(|| l.strip_prefix("invoke "))?;
    // O tipo vem antes do alvo e não tem `@` nem `asm`; o alvo é o primeiro
    // `@` ou `%` (um tipo de struct nomeado, `%T`, conta como indireta: a
    // entrada a mais não muda nada).
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

/// Se a chamada a `alvo` ganha rótulo: a indireta e a direta que pode lançar
/// ou chamar código Dart (a função Dart, a extern que a tabela de efeitos
/// marca); nunca um intrínseco.
fn labelable(alvo: Option<&str>) -> bool {
    match alvo {
        None => true,
        Some(name) if name.starts_with("llvm.") => false,
        Some(name) => {
            let ef = super::externs::effects_of(name);
            ef.throws || ef.calls_dart
        }
    }
}

/// O texto do `asm` de um grupo de entradas (no texto de IR: `\0A` é a
/// quebra de linha, `\22` a aspa e `$$` o cifrão), com o rótulo `42:` antes
/// quando `with_label`. No Mach-O, cada entrada tem o símbolo `l…` dela.
fn model(entries: &[Entry], with_label: bool) -> String {
    let (section, macho) = match crate::alvo::sistema() {
        Sistema::Windows => (".pushsection \\22.dfpcl$$m\\22,\\22dr\\22", false),
        Sistema::Linux => (".pushsection dfpcl,\\22aR?\\22,%progbits", false),
        Sistema::MacOs => (".pushsection __DATA,__dfpcl,regular,no_dead_strip", true),
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

/// A chamada do `asm` de um grupo, com os operandos (`"s"`).
fn group_asm(entries: &[Entry], operands: &[String], with_label: bool) -> String {
    format!(
        "  call void asm sideeffect \"{}\", \"{}\"({}) nounwind memory(inaccessiblemem: readwrite) \"gc-leaf-function\"",
        model(entries, with_label),
        vec!["s"; operands.len()].join(","),
        operands.join(", ")
    )
}

/// A chamada com `noinline` (antes de metadados e do `to label` de um
/// `invoke`).
fn with_noinline(line: &str) -> String {
    // O atributo vai antes do operando `"deopt"` (raízes por mapas), do
    // rótulo de um `invoke` e dos metadados.
    match [line.find(" [ \"deopt\""), line.find(" to label "), line.find(", !")].into_iter().flatten().min() {
        Some(i) => format!("{} noinline{}", &line[..i], &line[i..]),
        None => format!("{line} noinline"),
    }
}

/// A ligação das constantes do rastro: no Mach-O, símbolos locais (o
/// escritor do aarch64 não reloca subtração com símbolo temporário).
fn linkage() -> &'static str {
    if crate::alvo::sistema() == Sistema::MacOs { "internal" } else { "private" }
}

impl StackTrace {
    pub(super) fn new() -> StackTrace {
        StackTrace { definitions: String::new(), urls: HashMap::new(), records: HashMap::new() }
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

    /// O registro de `(name, url, marks)`: o deslocamento até a url, o byte
    /// das marcas e o nome. Um por módulo para cada trio.
    fn record(&mut self, name: &str, url: &str, marks: u8) -> usize {
        if let Some(&k) = self.records.get(&(name.to_string(), url.to_string(), marks)) {
            return k;
        }
        let u = self.url(url);
        let k = self.records.len();
        self.records.insert((name.to_string(), url.to_string(), marks), k);
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

    /// Os registros e as posições de um ponto de chamada, do quadro de
    /// dentro (a função copiada mais funda) ao da própria função.
    fn chain(&mut self, func: &Function, d: &FunctionDebugInfo, current: (u32, u32), context: Option<u32>) -> Vec<(usize, u32)> {
        let mut v = Vec::new();
        let (mut position, mut ctx) = (current, context);
        let mut steps = 0;
        while let Some(k) = ctx {
            let Some(e) = d.embedded_set.get(k as usize) else { break };
            v.push((self.record(&e.name, &e.url, e.marks), line_and_column(position)));
            position = e.call;
            ctx = e.parent;
            steps += 1;
            if steps > d.embedded_set.len() {
                break;
            }
        }
        v.push((self.record(&func.stack_trace_name(), &d.url, d.marks), line_and_column(position)));
        v
    }

    /// O grupo da função (as espécies 1 a 3), pela entrada uniforme das
    /// closures dela: `None` se ela não é corpo de closure nem corpo `async`.
    /// A identidade não leva a marca de oculta (um stub `async` que escuta um
    /// `Future` aparece, como a closure na VM).
    fn function_group(&mut self, func: &Function, d: &FunctionDebugInfo) -> Option<String> {
        let entry = d.async_body.as_ref().map(|c| c.entry.clone()).or_else(|| d.closure_entry.clone())?;
        let k = self.record(&func.stack_trace_name(), &d.url, d.marks & !stack_trace_marks::HIDDEN);
        let operands = vec![format!("ptr @{entry}"), format!("ptr @df.pcf.{k}")];
        let function = "${0:c}".to_string();
        let mut entries = vec![Entry { label: function.clone(), record: Some(1), kind: 1, word: line_and_column(d.token) }];
        if let Some(c) = &d.async_body {
            for p in &c.waits {
                entries.push(Entry { label: function.clone(), record: Some(1), kind: 2, word: line_and_column(*p) });
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

    /// Anota o texto de `func` (de `define` ao `}`): o `asm` da entrada antes
    /// de cada chamada rotulável, com a posição do último marcador. Sem
    /// `keep_markers` (sem o DWARF), os marcadores saem. Numa função
    /// `shared` (`linkonce_odr`, uma cópia por imagem) nada é
    /// rotulado e as chamadas saem `noinline`: o corpo de outra função
    /// copiado nela levaria entradas para a seção de uma cópia que o ligador
    /// descarta.
    pub(super) fn label(
        &mut self,
        text: &str,
        func: &Function,
        d: Option<&FunctionDebugInfo>,
        keep_markers: bool,
        shared: bool,
    ) -> String {
        // Sem posições, a função não tem rótulo (os quadros dela saem do
        // rastro); a compartilhada recebe o `noinline` mesmo assim.
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
        // O contexto de inlining da posição corrente (`df.emb`).
        let mut context: Option<u32> = None;
        // O grupo da função vai antes da primeira instrução.
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
            if pending_group && line.starts_with("  ") && !line.trim_start().starts_with(';') && !line.trim().is_empty() {
                pending_group = false;
                if let Some(g) = self.function_group(func, d) {
                    writeln!(exit, "{g}").unwrap();
                }
            }
            let alvo = call_target(line);
            if shared {
                // `noinline` no fim da chamada (antes de qualquer metadado
                // ou rótulo de `invoke`, que estas funções não têm).
                if alvo.is_some_and(|a| a.is_none_or(|n| !n.starts_with("llvm."))) && !line.contains(", !") && !line.contains(" to label ") {
                    writeln!(exit, "{}", with_noinline(line)).unwrap();
                } else {
                    writeln!(exit, "{line}").unwrap();
                }
                continue;
            }
            if let Some(alvo) = alvo
                && labelable(alvo)
            {
                let chain = self.chain(func, d, current, context);
                let operands: Vec<String> = chain.iter().map(|(k, _)| format!("ptr @df.pcf.{k}")).collect();
                let entries: Vec<Entry> = chain
                    .iter()
                    .enumerate()
                    .map(|(i, &(_, lc))| Entry { label: "42b".to_string(), record: Some(i), kind: 0, word: lc })
                    .collect();
                writeln!(exit, "{}", group_asm(&entries, &operands, true)).unwrap();
                // O inlining do LLVM apagaria o quadro da função Dart chamada.
                if alvo.is_some_and(|n| n.starts_with("df.")) && !line.contains(" noinline") {
                    writeln!(exit, "{}", with_noinline(line)).unwrap();
                    continue;
                }
            }
            writeln!(exit, "{line}").unwrap();
        }
        exit
    }

    /// As constantes do módulo (as urls e os registros das funções).
    pub(super) fn finalizar(self, out: &mut String) {
        if self.records.is_empty() {
            return;
        }
        out.push_str("\n; O rastro no formato da VM (llvm/rastro.rs)\n");
        out.push_str(&self.definitions);
    }
}

/// As declarações do registro da tabela da imagem e os dois operandos (o
/// começo e o fim da seção): as sentinelas `$a`/`$z` no COFF (em `comdat`,
/// uma cópia por imagem); no ELF, os símbolos do ligador, com uma âncora que
/// garante a seção; no Mach-O, os do ligador.
pub(super) fn markers() -> (&'static str, &'static str, &'static str) {
    match crate::alvo::sistema() {
        Sistema::Windows => (
            "$df.pcl.a = comdat any\n$df.pcl.z = comdat any\n\
             @df.pcl.a = linkonce_odr hidden constant [3 x i32] zeroinitializer, section \".dfpcl$a\", comdat, align 4\n\
             @df.pcl.z = linkonce_odr hidden constant [3 x i32] zeroinitializer, section \".dfpcl$z\", comdat, align 4\n",
            "@df.pcl.a",
            "@df.pcl.z",
        ),
        Sistema::Linux => (
            "$df.pcl.ancora = comdat any\n\
             @df.pcl.ancora = linkonce_odr hidden constant [3 x i32] zeroinitializer, section \"dfpcl\", comdat, align 4\n\
             @__start_dfpcl = external hidden global i8\n@__stop_dfpcl = external hidden global i8\n",
            "@__start_dfpcl",
            "@__stop_dfpcl",
        ),
        Sistema::MacOs => (
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
        assert_eq!(call_target("  %v3 = call i64 @df.a.b(i64 %v1)"), Some(Some("df.a.b")));
        assert_eq!(call_target("  call void @dartforge_exception_throw(i64 %v2, i8 3)"), Some(Some("dartforge_exception_throw")));
        assert_eq!(call_target("  %r = tail call i64 @\"df.x$tear\"(i64 0)"), Some(Some("df.x$tear")));
        assert_eq!(call_target("  %v9 = call i64 %f(i64 %v1)"), Some(None));
        assert_eq!(call_target("  call void (...) @llvm.fake.use(ptr addrspace(1) %raiz1)"), Some(Some("llvm.fake.use")));
        assert_eq!(call_target("  %v4 = call i64 @df.a(i64 %v1) [ \"deopt\"(i64 %v1) ]"), Some(Some("df.a")));
        assert_eq!(with_noinline("  %v4 = call i64 @df.a(i64 %v1) [ \"deopt\"(i64 %v1) ]"), "  %v4 = call i64 @df.a(i64 %v1) noinline [ \"deopt\"(i64 %v1) ]");
        assert_eq!(call_target("  %r.ok = invoke i64 @g(i64 1) to label %a unwind label %b"), Some(Some("g")));
        assert_eq!(call_target("  call void asm sideeffect \"42:\", \"s\"(ptr @df.pcf.0) nounwind"), None);
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
        let d = FunctionDebugInfo { file: "/x/a.dart".to_string(), url: "file:///x/a.dart".to_string(), line: 2, ..Default::default() };
        let text = "define i64 @df.lib.f() {\nb0:\n  ; df.pos 3 7\n  %v1 = call i64 @df.lib.g()\n  %v2 = add i64 %v1, 1\n  ret i64 %v2\n}\n";
        let mut r = StackTrace::new();
        let exit = r.label(text, &func, Some(&d), false, false);
        assert!(!exit.contains("df.pos"), "{exit}");
        assert!(exit.contains(&format!(".long {}", 3 << 12 | 7)), "{exit}");
        let asm = exit.find("asm sideeffect").expect("rótulo");
        assert!(asm < exit.find("@df.lib.g()").expect("chamada"), "{exit}");
        let mut out = String::new();
        r.finalizar(&mut out);
        assert!(out.contains("@df.pcu.0 ") && out.contains("c\"file:\\2F\\2F\\2Fx\\2Fa.dart\\00\""), "{out}");
        assert!(out.contains("@df.pcf.0 ") && out.contains("i8 0, [2 x i8] c\"f\\00\""), "{out}");
        // A chamada direta a uma função Dart não recebe o inlining do LLVM.
        assert!(exit.contains("@df.lib.g() noinline"), "{exit}");
    }

    #[test]
    fn embedded_frames_have_one_entry_per_function() {
        let func = function("df.lib.f");
        let d = FunctionDebugInfo {
            url: "file:///x/a.dart".to_string(),
            line: 2,
            embedded_set: vec![crate::hir::Embedded { name: "g".to_string(), url: "file:///x/b.dart".to_string(), call: (3, 7), parent: None, marks: 0 }],
            ..Default::default()
        };
        let text = "define i64 @df.lib.f() {\nb0:\n  ; df.pos 3 7\n  ; df.emb 9 4 0\n  %v1 = call i64 @df.lib.h()\n  ret i64 %v1\n}\n";
        let mut r = StackTrace::new();
        let exit = r.label(text, &func, Some(&d), false, false);
        assert!(exit.contains(&format!(".long {}", 9 << 12 | 4)) && exit.contains(&format!(".long {}", 3 << 12 | 7)), "{exit}");
        assert!(exit.contains("\"s,s\"(ptr @df.pcf.0, ptr @df.pcf.1)"), "{exit}");
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
            async_body: Some(crate::hir::TraceAsyncBody { entry: "df.lib.f$async$q0000abcd$ent".to_string(), waits: vec![(3, 9), (4, 11)] }),
            ..Default::default()
        };
        let text = "define i64 @df.lib.f$async$q0000abcd() {\nb0:\n  %v1 = add i64 1, 2\n  ret i64 %v1\n}\n";
        let mut r = StackTrace::new();
        let exit = r.label(text, &func, Some(&d), false, false);
        let g = exit.find("asm sideeffect").expect("grupo");
        assert!(g < exit.find("%v1 = add").expect("instrução"), "{exit}");
        assert!(exit.contains("ptr @df.lib.f$async$q0000abcd$ent, ptr @df.pcf.0"), "{exit}");
        assert!(exit.contains(&format!(".long {}", 3 << 12 | 9)) && exit.contains("-.+2"), "{exit}");
        assert_eq!(func.stack_trace_name(), "f");
        assert_eq!(function("dart_main$clo0$e1234abcd").stack_trace_name(), "main.<anonymous closure>");
        assert_eq!(function("df.lib.C.m$inner$e0000ffff").stack_trace_name(), "C.m.inner");
    }

    #[test]
    fn shared_function_does_not_label_and_gets_no_inlining() {
        let func = function("df.lib.t$tear");
        let d = FunctionDebugInfo { url: "file:///x/a.dart".to_string(), line: 1, ..Default::default() };
        let text = "define linkonce_odr i64 @\"df.lib.t$tear\"() {\nb0:\n  %v1 = call i64 @df.lib.t()\n  ret i64 %v1\n}\n";
        let mut r = StackTrace::new();
        let exit = r.label(text, &func, Some(&d), false, true);
        assert!(!exit.contains("asm"), "{exit}");
        assert!(exit.contains("%v1 = call i64 @df.lib.t() noinline"), "{exit}");
    }
}
