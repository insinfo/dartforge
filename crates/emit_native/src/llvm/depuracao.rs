//! Informação de depuração (J05): as tabelas de linha do DWARF (CodeView no
//! Windows) a partir das posições que o lowering registrou em cada função
//! (`hir::DepuracaoDaFuncao`).
//!
//! Cada função com posições ganha um `DISubprogram` (nome Dart e símbolo) e
//! cada instrução dela um `!dbg` com a `DILocation` do comando que a gerou.
//! O emissor escreve, antes da instrução cuja posição muda, a linha
//! `  ; df.pos <linha> <coluna>`; ao fim da função, [`Depuracao::anotar`]
//! troca esses marcadores pelos `!dbg` de todas as instruções (a que não tem
//! posição herda a anterior; o prólogo, a da declaração). Anotar o texto já
//! emitido cobre também as instruções auxiliares (coerções, quadro de raízes,
//! conferência de exceção) sem tocar em cada ponto de emissão, e o LLVM exige
//! o `!dbg` em toda chamada de uma função com `DISubprogram`.
//!
//! O resultado é o que um depurador nativo precisa: parar em
//! `arquivo.dart:linha` e mostrar a pilha com os nomes e as linhas Dart
//! (`gdb`, `lldb`, o depurador do Visual Studio). Variáveis e tipos Dart não
//! são descritos; a unidade é `FullDebug` porque só nela o LLVM emite o
//! `DW_TAG_subprogram` de cada função (com `LineTablesOnly`, a pilha mostra
//! o símbolo de ligação).

use crate::hir::{DepuracaoDaFuncao, Function};
use std::collections::HashMap;
use std::fmt::Write as _;

/// O prefixo do marcador de posição no texto de uma função.
pub(super) const MARCADOR: &str = "  ; df.pos ";

/// Os metadados de depuração do módulo, numerados na ordem de criação.
pub(super) struct Depuracao {
    /// `!N = …`, na ordem dos números.
    metadados: Vec<String>,
    arquivos: HashMap<String, u32>,
    locais: HashMap<(u32, u32, u32), u32>,
    /// O arquivo da unidade de compilação: o da primeira função anotada.
    arquivo_da_unidade: Option<u32>,
}

/// O número da `DICompileUnit` e do tipo de sub-rotina (vazio), reservados.
const UNIDADE: u32 = 0;
const TIPO: u32 = 1;

impl Depuracao {
    pub(super) fn nova() -> Depuracao {
        Depuracao {
            // Os dois primeiros são preenchidos em `finalizar`.
            metadados: vec![String::new(), "!DISubroutineType(types: !{})".to_string()],
            arquivos: HashMap::new(),
            locais: HashMap::new(),
            arquivo_da_unidade: None,
        }
    }

    fn novo(&mut self, texto: String) -> u32 {
        self.metadados.push(texto);
        (self.metadados.len() - 1) as u32
    }

    fn arquivo(&mut self, caminho: &str) -> u32 {
        if let Some(&n) = self.arquivos.get(caminho) {
            return n;
        }
        let p = std::path::Path::new(caminho);
        let nome = p.file_name().map_or_else(|| caminho.to_string(), |n| n.to_string_lossy().into_owned());
        let dir = p.parent().map(|d| d.display().to_string()).unwrap_or_default();
        let n = self.novo(format!("!DIFile(filename: \"{}\", directory: \"{}\")", texto_ir(&nome), texto_ir(&dir)));
        self.arquivos.insert(caminho.to_string(), n);
        self.arquivo_da_unidade.get_or_insert(n);
        n
    }

    fn local(&mut self, (linha, coluna): (u32, u32), escopo: u32) -> u32 {
        if let Some(&n) = self.locais.get(&(linha, coluna, escopo)) {
            return n;
        }
        let n = self.novo(format!("!DILocation(line: {linha}, column: {coluna}, scope: !{escopo})"));
        self.locais.insert((linha, coluna, escopo), n);
        n
    }

    /// Anota o texto de `func` (de `define` ao `}`): o `DISubprogram` na
    /// definição e o `!dbg` em cada instrução; os marcadores saem.
    pub(super) fn anotar(&mut self, texto: &str, func: &Function, d: &DepuracaoDaFuncao) -> String {
        let arquivo = self.arquivo(&d.arquivo);
        let sp = self.novo(format!(
            "distinct !DISubprogram(name: \"{}\", scope: !{arquivo}, file: !{arquivo}, line: {}, \
             type: !{TIPO}, scopeLine: {}, spFlags: DISPFlagDefinition, unit: !{UNIDADE})",
            texto_ir(&nome_dart(func)),
            d.linha,
            d.linha,
        ));
        let mut atual = self.local((d.linha, 1), sp);
        let mut saida = String::with_capacity(texto.len() + texto.len() / 4);
        for linha in texto.lines() {
            if linha.starts_with("define ") {
                match linha.strip_suffix(" {") {
                    Some(cabeca) => writeln!(saida, "{cabeca} !dbg !{sp} {{").unwrap(),
                    None => writeln!(saida, "{linha}").unwrap(),
                }
            } else if let Some(pos) = linha.strip_prefix(MARCADOR) {
                let mut partes = pos.split(' ').filter_map(|x| x.parse::<u32>().ok());
                if let (Some(l), Some(c)) = (partes.next(), partes.next()) {
                    atual = self.local((l, c), sp);
                }
            } else if linha.starts_with("  ") && !linha.starts_with("  ;") && !linha.trim().is_empty() {
                writeln!(saida, "{linha}, !dbg !{atual}").unwrap();
            } else {
                writeln!(saida, "{linha}").unwrap();
            }
        }
        saida
    }

    /// Os metadados do módulo (nada, se nenhuma função foi anotada).
    pub(super) fn finalizar(mut self, out: &mut String) {
        let Some(arquivo) = self.arquivo_da_unidade else { return };
        self.metadados[UNIDADE as usize] = format!(
            "distinct !DICompileUnit(language: DW_LANG_C99, file: !{arquivo}, producer: \"dartforge\", \
             isOptimized: false, runtimeVersion: 0, emissionKind: FullDebug)"
        );
        let versao = self.novo(match crate::alvo::sistema() {
            crate::alvo::Sistema::Windows => "!{i32 2, !\"CodeView\", i32 1}".to_string(),
            // O `dsymutil` e o `lldb` do Xcode leem o DWARF 4 sem ressalvas.
            crate::alvo::Sistema::MacOs => "!{i32 7, !\"Dwarf Version\", i32 4}".to_string(),
            crate::alvo::Sistema::Linux => "!{i32 7, !\"Dwarf Version\", i32 5}".to_string(),
        });
        let formato = self.novo("!{i32 2, !\"Debug Info Version\", i32 3}".to_string());
        out.push_str("\n; Informação de depuração (J05)\n");
        writeln!(out, "!llvm.dbg.cu = !{{!{UNIDADE}}}").unwrap();
        writeln!(out, "!llvm.module.flags = !{{!{versao}, !{formato}}}").unwrap();
        for (n, m) in self.metadados.iter().enumerate() {
            writeln!(out, "!{n} = {m}").unwrap();
        }
    }
}

/// O nome que o depurador mostra: o qualificado do Dart (`Conta.depositar`,
/// `dobro`), tirado do símbolo estável `df.<biblioteca>.<classe>.<membro>`
/// (a biblioteca vem escapada, sem pontos); sem o prefixo, o da HIR. O
/// símbolo continua na tabela de símbolos do executável.
fn nome_dart(func: &Function) -> String {
    if func.symbol == "dart_main" {
        return "main".to_string();
    }
    func.symbol
        .strip_prefix("df.")
        .and_then(|resto| resto.split_once('.'))
        .map(|(_, nome)| nome.trim_start_matches('.').to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| func.name.clone())
}

/// Texto entre aspas no IR: `"` e `\` e o que não é ASCII visível como `\XX`.
fn texto_ir(s: &str) -> String {
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
