//! O rastro no formato da VM (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
//! §13.14): a tabela endereço → (função, linha, coluna) que o runtime usa
//! para escrever `#0      f (url:linha:coluna)`.
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
//! unidade) e o nome qualificado, terminado em zero.
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
//! do J05 (`llvm/depuracao.rs`): as posições vêm dos marcadores
//! `  ; df.pos <linha> <coluna>`.
//! Escrito sem compilar nem executar (2026-10-05).

use crate::alvo::Sistema;
use crate::hir::{DepuracaoDaFuncao, Function};
use std::collections::HashMap;
use std::fmt::Write as _;

/// A tabela do rastro de um módulo em emissão.
pub(super) struct Rastro {
    /// As urls e os registros das funções, no fim do módulo.
    definicoes: String,
    urls: HashMap<String, usize>,
    registros: usize,
}

/// `linha << 12 | coluna`: linha até 2²⁰ − 1 e coluna até 4095; fora disso,
/// 0 (a linha desconhecida, ou só a linha).
pub(super) fn linha_e_coluna((linha, coluna): (u32, u32)) -> u32 {
    if linha == 0 || linha >= 1 << 20 {
        return 0;
    }
    let coluna = if coluna < 1 << 12 { coluna } else { 0 };
    linha << 12 | coluna
}

/// O alvo da chamada da linha de IR, se ela é uma chamada: `Some(Some(nome))`
/// para a direta e `Some(None)` para a indireta. O `asm` não é alvo.
pub(super) fn alvo_da_chamada(linha: &str) -> Option<Option<&str>> {
    let mut l = linha.trim_start();
    if l.starts_with('%') {
        l = &l[l.find(" = ")? + 3..];
    }
    for prefixo in ["tail ", "musttail ", "notail "] {
        if let Some(r) = l.strip_prefix(prefixo) {
            l = r;
        }
    }
    let resto = l.strip_prefix("call ").or_else(|| l.strip_prefix("invoke "))?;
    // O tipo vem antes do alvo e não tem `@` nem `asm`; o alvo é o primeiro
    // `@` ou `%` (um tipo de struct nomeado, `%T`, conta como indireta: a
    // entrada a mais não muda nada).
    let ini = resto.find(['@', '%'])?;
    if resto[..ini].split_whitespace().any(|t| t == "asm") {
        return None;
    }
    if resto.as_bytes()[ini] == b'%' {
        return Some(None);
    }
    let nome = &resto[ini + 1..];
    if let Some(entre_aspas) = nome.strip_prefix('"') {
        return Some(Some(&entre_aspas[..entre_aspas.find('"')?]));
    }
    Some(Some(&nome[..nome.find('(')?]))
}

/// Se a chamada a `alvo` ganha rótulo: a indireta e a direta que pode lançar
/// ou chamar código Dart (a função Dart, a extern que a tabela de efeitos
/// marca); nunca um intrínseco.
fn rotulavel(alvo: Option<&str>) -> bool {
    match alvo {
        None => true,
        Some(nome) if nome.starts_with("llvm.") => false,
        Some(nome) => {
            let ef = super::externs::efeitos_de(nome);
            ef.lanca || ef.chama_dart
        }
    }
}

/// O texto do `asm` de uma entrada (no texto de IR: `\0A` é a quebra de
/// linha, `\22` a aspa e `$$` o cifrão).
fn modelo(lc: u32) -> String {
    let (secao, ancora) = match crate::alvo::sistema() {
        Sistema::Windows => (".pushsection \\22.dfpcl$$m\\22,\\22dr\\22", ""),
        Sistema::Linux => (".pushsection dfpcl,\\22aR?\\22,%progbits", ""),
        Sistema::MacOs => (".pushsection __DATA,__dfpcl,regular,no_dead_strip", "\\0Aldfpcl${:uid}:"),
    };
    format!(
        "42:\\0A\\09{secao}\\0A\\09.p2align 2{ancora}\\0A\\09.long 42b-.\\0A\\09.long ${{0:c}}-.\\0A\\09.long {lc}\\0A\\09.popsection"
    )
}

/// A ligação das constantes do rastro: no Mach-O, símbolos locais (o
/// escritor do aarch64 não reloca subtração com símbolo temporário).
fn ligacao() -> &'static str {
    if crate::alvo::sistema() == Sistema::MacOs { "internal" } else { "private" }
}

impl Rastro {
    pub(super) fn novo() -> Rastro {
        Rastro { definicoes: String::new(), urls: HashMap::new(), registros: 0 }
    }

    fn url(&mut self, url: &str) -> usize {
        if let Some(&n) = self.urls.get(url) {
            return n;
        }
        let n = self.urls.len();
        let bytes = super::seletores::bytes_llvm(url);
        writeln!(
            self.definicoes,
            "@df.pcu.{n} = {} unnamed_addr constant [{} x i8] c\"{bytes}\\00\", align 1",
            ligacao(),
            url.len() + 1
        )
        .unwrap();
        self.urls.insert(url.to_string(), n);
        n
    }

    /// Um registro novo para a função: o deslocamento até a url e o nome.
    fn registro(&mut self, nome: &str, url: &str) -> usize {
        let u = self.url(url);
        let k = self.registros;
        self.registros += 1;
        let n = nome.len() + 1;
        writeln!(
            self.definicoes,
            "@df.pcf.{k} = {} unnamed_addr constant <{{ i32, [{n} x i8] }}> <{{ i32 trunc (i64 sub (i64 ptrtoint (ptr @df.pcu.{u} to i64), i64 ptrtoint (ptr @df.pcf.{k} to i64)) to i32), [{n} x i8] c\"{}\\00\" }}>, align 4",
            ligacao(),
            super::seletores::bytes_llvm(nome)
        )
        .unwrap();
        k
    }

    /// Anota o texto de `func` (de `define` ao `}`): o `asm` da entrada antes
    /// de cada chamada rotulável, com a posição do último marcador. Sem
    /// `manter_marcadores` (sem o DWARF), os marcadores saem. Numa função
    /// `compartilhada` (`linkonce_odr`, uma cópia por imagem) nada é
    /// rotulado e as chamadas saem `noinline`: o corpo de outra função
    /// copiado nela levaria entradas para a seção de uma cópia que o ligador
    /// descarta.
    pub(super) fn rotular(
        &mut self,
        texto: &str,
        func: &Function,
        d: Option<&DepuracaoDaFuncao>,
        manter_marcadores: bool,
        compartilhada: bool,
    ) -> String {
        // Sem posições, a função não tem rótulo (os quadros dela saem do
        // rastro); a compartilhada recebe o `noinline` mesmo assim.
        let vazio;
        let d = match d {
            Some(d) => d,
            None if compartilhada => {
                vazio = DepuracaoDaFuncao { arquivo: String::new(), url: String::new(), linha: 0, posicoes: HashMap::new(), saidas: HashMap::new() };
                &vazio
            }
            None => return texto.to_string(),
        };
        let mut saida = String::with_capacity(texto.len() + texto.len() / 2);
        let mut registro: Option<usize> = None;
        let mut atual = (d.linha, 0);
        for linha in texto.lines() {
            if let Some(pos) = linha.strip_prefix(super::depuracao::MARCADOR) {
                let mut partes = pos.split(' ').filter_map(|x| x.parse::<u32>().ok());
                if let (Some(l), Some(c)) = (partes.next(), partes.next()) {
                    atual = (l, c);
                }
                if manter_marcadores {
                    writeln!(saida, "{linha}").unwrap();
                }
                continue;
            }
            let alvo = alvo_da_chamada(linha);
            if compartilhada {
                // `noinline` no fim da chamada (antes de qualquer metadado
                // ou rótulo de `invoke`, que estas funções não têm).
                if alvo.is_some_and(|a| a.is_none_or(|n| !n.starts_with("llvm."))) && !linha.contains(", !") && !linha.contains(" to label ") {
                    writeln!(saida, "{linha} noinline").unwrap();
                } else {
                    writeln!(saida, "{linha}").unwrap();
                }
                continue;
            }
            if let Some(alvo) = alvo
                && rotulavel(alvo)
            {
                let k = *registro.get_or_insert_with(|| self.registro(&super::depuracao::nome_dart(func), &d.url));
                writeln!(
                    saida,
                    "  call void asm sideeffect \"{}\", \"s\"(ptr @df.pcf.{k}) nounwind memory(inaccessiblemem: readwrite) \"gc-leaf-function\"",
                    modelo(linha_e_coluna(atual))
                )
                .unwrap();
            }
            writeln!(saida, "{linha}").unwrap();
        }
        saida
    }

    /// As constantes do módulo (as urls e os registros das funções).
    pub(super) fn finalizar(self, out: &mut String) {
        if self.registros == 0 {
            return;
        }
        out.push_str("\n; O rastro no formato da VM (llvm/rastro.rs)\n");
        out.push_str(&self.definicoes);
    }
}

/// As declarações do registro da tabela da imagem e os dois operandos (o
/// começo e o fim da seção): as sentinelas `$a`/`$z` no COFF (em `comdat`,
/// uma cópia por imagem); no ELF, os símbolos do ligador, com uma âncora que
/// garante a seção; no Mach-O, os do ligador.
pub(super) fn marcadores() -> (&'static str, &'static str, &'static str) {
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
mod testes {
    use super::*;
    use crate::hir::Type;

    fn funcao(simbolo: &str) -> Function {
        Function {
            symbol: simbolo.to_string(),
            name: simbolo.rsplit('.').next().unwrap_or_default().to_string(),
            params: Vec::new(),
            return_ty: Type::I64,
            blocks: Vec::new(),
            depuracao: None,
        }
    }

    #[test]
    fn acha_o_alvo_das_chamadas() {
        assert_eq!(alvo_da_chamada("  %v3 = call i64 @df.a.b(i64 %v1)"), Some(Some("df.a.b")));
        assert_eq!(alvo_da_chamada("  call void @dartforge_exception_throw(i64 %v2, i8 3)"), Some(Some("dartforge_exception_throw")));
        assert_eq!(alvo_da_chamada("  %r = tail call i64 @\"df.x$tear\"(i64 0)"), Some(Some("df.x$tear")));
        assert_eq!(alvo_da_chamada("  %v9 = call i64 %f(i64 %v1)"), Some(None));
        assert_eq!(alvo_da_chamada("  call void (...) @llvm.fake.use(ptr addrspace(1) %raiz1)"), Some(Some("llvm.fake.use")));
        assert_eq!(alvo_da_chamada("  %r.ok = invoke i64 @g(i64 1) to label %a unwind label %b"), Some(Some("g")));
        assert_eq!(alvo_da_chamada("  call void asm sideeffect \"42:\", \"s\"(ptr @df.pcf.0) nounwind"), None);
        assert_eq!(alvo_da_chamada("  %v4 = add i64 %v1, %v2"), None);
        assert_eq!(alvo_da_chamada("  store i64 %v1, ptr @g"), None);
    }

    #[test]
    fn empacota_linha_e_coluna() {
        assert_eq!(linha_e_coluna((3, 5)), 3 << 12 | 5);
        assert_eq!(linha_e_coluna((3, 5000)), 3 << 12);
        assert_eq!(linha_e_coluna((0, 5)), 0);
        assert_eq!(linha_e_coluna((1 << 20, 1)), 0);
    }

    #[test]
    fn rotula_as_chamadas_com_a_posicao_do_marcador() {
        let func = funcao("df.lib.f");
        let d = DepuracaoDaFuncao {
            arquivo: "/x/a.dart".to_string(),
            url: "file:///x/a.dart".to_string(),
            linha: 2,
            posicoes: HashMap::new(),
            saidas: HashMap::new(),
        };
        let texto = "define i64 @df.lib.f() {\nb0:\n  ; df.pos 3 7\n  %v1 = call i64 @df.lib.g()\n  %v2 = add i64 %v1, 1\n  ret i64 %v2\n}\n";
        let mut r = Rastro::novo();
        let saida = r.rotular(texto, &func, Some(&d), false, false);
        assert!(!saida.contains("df.pos"), "{saida}");
        assert!(saida.contains(&format!(".long {}", 3 << 12 | 7)), "{saida}");
        let asm = saida.find("asm sideeffect").expect("rótulo");
        assert!(asm < saida.find("@df.lib.g()").expect("chamada"), "{saida}");
        let mut out = String::new();
        r.finalizar(&mut out);
        assert!(out.contains("@df.pcu.0 ") && out.contains("c\"file:\\2F\\2F\\2Fx\\2Fa.dart\\00\""), "{out}");
        assert!(out.contains("@df.pcf.0 ") && out.contains("c\"f\\00\""), "{out}");
    }

    #[test]
    fn funcao_compartilhada_nao_rotula_e_nao_recebe_inlining() {
        let func = funcao("df.lib.t$tear");
        let d = DepuracaoDaFuncao {
            arquivo: String::new(),
            url: "file:///x/a.dart".to_string(),
            linha: 1,
            posicoes: HashMap::new(),
            saidas: HashMap::new(),
        };
        let texto = "define linkonce_odr i64 @\"df.lib.t$tear\"() {\nb0:\n  %v1 = call i64 @df.lib.t()\n  ret i64 %v1\n}\n";
        let mut r = Rastro::novo();
        let saida = r.rotular(texto, &func, Some(&d), false, true);
        assert!(!saida.contains("asm"), "{saida}");
        assert!(saida.contains("%v1 = call i64 @df.lib.t() noinline"), "{saida}");
    }
}
