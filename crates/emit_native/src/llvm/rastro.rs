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
//! unidade), um byte de marcas (`hir::marcas_do_rastro`) e o nome
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
//! palavra é `posição << 2 | direto << 1 | célula` no ambiente). O runtime
//! os usa para seguir a cadeia de quem espera e escrever
//! `<asynchronous suspension>` (`RT/rastro.rs`).
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
use crate::hir::{DepuracaoDaFuncao, Function, marcas_do_rastro};
use std::collections::HashMap;
use std::fmt::Write as _;

/// A tabela do rastro de um módulo em emissão.
pub(super) struct Rastro {
    /// As urls e os registros das funções, no fim do módulo.
    definicoes: String,
    urls: HashMap<String, usize>,
    /// Os registros já definidos, por `(nome, url, marcas)`.
    registros: HashMap<(String, String, u8), usize>,
}

/// Uma entrada de 12 bytes da seção (veja o começo do arquivo).
struct Entrada {
    /// A expressão do rótulo: `42b` (o ponto de chamada) ou `${n:c}` (o
    /// operando `n`, uma função).
    rotulo: String,
    /// O operando do registro, ou nenhum (o campo é só a espécie).
    registro: Option<usize>,
    /// A espécie, nos dois bits baixos do campo do registro.
    especie: u32,
    palavra: u32,
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

/// O texto do `asm` de um grupo de entradas (no texto de IR: `\0A` é a
/// quebra de linha, `\22` a aspa e `$$` o cifrão), com o rótulo `42:` antes
/// quando `com_rotulo`. No Mach-O, cada entrada tem o símbolo `l…` dela.
fn modelo(entradas: &[Entrada], com_rotulo: bool) -> String {
    let (secao, macho) = match crate::alvo::sistema() {
        Sistema::Windows => (".pushsection \\22.dfpcl$$m\\22,\\22dr\\22", false),
        Sistema::Linux => (".pushsection dfpcl,\\22aR?\\22,%progbits", false),
        Sistema::MacOs => (".pushsection __DATA,__dfpcl,regular,no_dead_strip", true),
    };
    let mut s = String::new();
    if com_rotulo {
        s.push_str("42:\\0A\\09");
    }
    s.push_str(secao);
    s.push_str("\\0A\\09.p2align 2");
    for (i, e) in entradas.iter().enumerate() {
        if macho {
            write!(s, "\\0Aldfpcl${{:uid}}_{i}:").unwrap();
        }
        write!(s, "\\0A\\09.long {}-.", e.rotulo).unwrap();
        match e.registro {
            Some(k) if e.especie == 0 => write!(s, "\\0A\\09.long ${{{k}:c}}-.").unwrap(),
            Some(k) => write!(s, "\\0A\\09.long ${{{k}:c}}-.+{}", e.especie).unwrap(),
            None => write!(s, "\\0A\\09.long {}", e.especie).unwrap(),
        }
        write!(s, "\\0A\\09.long {}", e.palavra).unwrap();
    }
    s.push_str("\\0A\\09.popsection");
    s
}

/// A chamada do `asm` de um grupo, com os operandos (`"s"`).
fn asm_do_grupo(entradas: &[Entrada], operandos: &[String], com_rotulo: bool) -> String {
    format!(
        "  call void asm sideeffect \"{}\", \"{}\"({}) nounwind memory(inaccessiblemem: readwrite) \"gc-leaf-function\"",
        modelo(entradas, com_rotulo),
        vec!["s"; operandos.len()].join(","),
        operandos.join(", ")
    )
}

/// A chamada com `noinline` (antes de metadados e do `to label` de um
/// `invoke`).
fn com_noinline(linha: &str) -> String {
    if let Some(i) = linha.find(" to label ") {
        return format!("{} noinline{}", &linha[..i], &linha[i..]);
    }
    if let Some(i) = linha.find(", !") {
        return format!("{} noinline{}", &linha[..i], &linha[i..]);
    }
    format!("{linha} noinline")
}

/// A ligação das constantes do rastro: no Mach-O, símbolos locais (o
/// escritor do aarch64 não reloca subtração com símbolo temporário).
fn ligacao() -> &'static str {
    if crate::alvo::sistema() == Sistema::MacOs { "internal" } else { "private" }
}

impl Rastro {
    pub(super) fn novo() -> Rastro {
        Rastro { definicoes: String::new(), urls: HashMap::new(), registros: HashMap::new() }
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

    /// O registro de `(nome, url, marcas)`: o deslocamento até a url, o byte
    /// das marcas e o nome. Um por módulo para cada trio.
    fn registro(&mut self, nome: &str, url: &str, marcas: u8) -> usize {
        if let Some(&k) = self.registros.get(&(nome.to_string(), url.to_string(), marcas)) {
            return k;
        }
        let u = self.url(url);
        let k = self.registros.len();
        self.registros.insert((nome.to_string(), url.to_string(), marcas), k);
        let n = nome.len() + 1;
        writeln!(
            self.definicoes,
            "@df.pcf.{k} = {} unnamed_addr constant <{{ i32, i8, [{n} x i8] }}> <{{ i32 trunc (i64 sub (i64 ptrtoint (ptr @df.pcu.{u} to i64), i64 ptrtoint (ptr @df.pcf.{k} to i64)) to i32), i8 {marcas}, [{n} x i8] c\"{}\\00\" }}>, align 4",
            ligacao(),
            super::seletores::bytes_llvm(nome)
        )
        .unwrap();
        k
    }

    /// Os registros e as posições de um ponto de chamada, do quadro de
    /// dentro (a função copiada mais funda) ao da própria função.
    fn cadeia(&mut self, func: &Function, d: &DepuracaoDaFuncao, atual: (u32, u32), contexto: Option<u32>) -> Vec<(usize, u32)> {
        let mut v = Vec::new();
        let (mut posicao, mut ctx) = (atual, contexto);
        let mut passos = 0;
        while let Some(k) = ctx {
            let Some(e) = d.embutidas.get(k as usize) else { break };
            v.push((self.registro(&e.nome, &e.url, e.marcas), linha_e_coluna(posicao)));
            posicao = e.chamada;
            ctx = e.pai;
            passos += 1;
            if passos > d.embutidas.len() {
                break;
            }
        }
        v.push((self.registro(&func.nome_do_rastro(), &d.url, d.marcas), linha_e_coluna(posicao)));
        v
    }

    /// O grupo da função (as espécies 1 a 3), pela entrada uniforme das
    /// closures dela: `None` se ela não é corpo de closure nem corpo `async`.
    /// A identidade não leva a marca de oculta (um stub `async` que escuta um
    /// `Future` aparece, como a closure na VM).
    fn grupo_da_funcao(&mut self, func: &Function, d: &DepuracaoDaFuncao) -> Option<String> {
        let entrada = d.corpo_async.as_ref().map(|c| c.entrada.clone()).or_else(|| d.entrada_de_closure.clone())?;
        let k = self.registro(&func.nome_do_rastro(), &d.url, d.marcas & !marcas_do_rastro::OCULTA);
        let operandos = vec![format!("ptr @{entrada}"), format!("ptr @df.pcf.{k}")];
        let funcao = "${0:c}".to_string();
        let mut entradas = vec![Entrada { rotulo: funcao.clone(), registro: Some(1), especie: 1, palavra: linha_e_coluna(d.token) }];
        if let Some(c) = &d.corpo_async {
            for p in &c.esperas {
                entradas.push(Entrada { rotulo: funcao.clone(), registro: Some(1), especie: 2, palavra: linha_e_coluna(*p) });
            }
        }
        if let Some(e) = d.elo {
            entradas.push(Entrada {
                rotulo: funcao,
                registro: None,
                especie: 3,
                palavra: (e.indice as u32) << 2 | u32::from(e.direto) << 1 | u32::from(e.celula),
            });
        }
        Some(asm_do_grupo(&entradas, &operandos, false))
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
                vazio = DepuracaoDaFuncao::default();
                &vazio
            }
            None => return texto.to_string(),
        };
        let mut saida = String::with_capacity(texto.len() + texto.len() / 2);
        let mut atual = (d.linha, 0);
        // O contexto de inlining da posição corrente (`df.emb`).
        let mut contexto: Option<u32> = None;
        // O grupo da função vai antes da primeira instrução.
        let mut grupo_pendente = !compartilhada;
        for linha in texto.lines() {
            if let Some(pos) = linha.strip_prefix(super::depuracao::MARCADOR) {
                let mut partes = pos.split(' ').filter_map(|x| x.parse::<u32>().ok());
                if let (Some(l), Some(c)) = (partes.next(), partes.next()) {
                    atual = (l, c);
                }
                contexto = None;
                if manter_marcadores {
                    writeln!(saida, "{linha}").unwrap();
                }
                continue;
            }
            if let Some(pos) = linha.strip_prefix(super::depuracao::MARCADOR_EMBUTIDO) {
                let mut partes = pos.split(' ').filter_map(|x| x.parse::<u32>().ok());
                if let (Some(l), Some(c), Some(k)) = (partes.next(), partes.next(), partes.next()) {
                    atual = (l, c);
                    contexto = Some(k);
                }
                if manter_marcadores {
                    writeln!(saida, "{linha}").unwrap();
                }
                continue;
            }
            if grupo_pendente && linha.starts_with("  ") && !linha.trim_start().starts_with(';') && !linha.trim().is_empty() {
                grupo_pendente = false;
                if let Some(g) = self.grupo_da_funcao(func, d) {
                    writeln!(saida, "{g}").unwrap();
                }
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
                let cadeia = self.cadeia(func, d, atual, contexto);
                let operandos: Vec<String> = cadeia.iter().map(|(k, _)| format!("ptr @df.pcf.{k}")).collect();
                let entradas: Vec<Entrada> = cadeia
                    .iter()
                    .enumerate()
                    .map(|(i, &(_, lc))| Entrada { rotulo: "42b".to_string(), registro: Some(i), especie: 0, palavra: lc })
                    .collect();
                writeln!(saida, "{}", asm_do_grupo(&entradas, &operandos, true)).unwrap();
                // O inlining do LLVM apagaria o quadro da função Dart chamada.
                if alvo.is_some_and(|n| n.starts_with("df.")) && !linha.contains(" noinline") {
                    writeln!(saida, "{}", com_noinline(linha)).unwrap();
                    continue;
                }
            }
            writeln!(saida, "{linha}").unwrap();
        }
        saida
    }

    /// As constantes do módulo (as urls e os registros das funções).
    pub(super) fn finalizar(self, out: &mut String) {
        if self.registros.is_empty() {
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
        let d = DepuracaoDaFuncao { arquivo: "/x/a.dart".to_string(), url: "file:///x/a.dart".to_string(), linha: 2, ..Default::default() };
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
        assert!(out.contains("@df.pcf.0 ") && out.contains("i8 0, [2 x i8] c\"f\\00\""), "{out}");
        // A chamada direta a uma função Dart não recebe o inlining do LLVM.
        assert!(saida.contains("@df.lib.g() noinline"), "{saida}");
    }

    #[test]
    fn quadros_embutidos_tem_uma_entrada_por_funcao() {
        let func = funcao("df.lib.f");
        let d = DepuracaoDaFuncao {
            url: "file:///x/a.dart".to_string(),
            linha: 2,
            embutidas: vec![crate::hir::Embutida { nome: "g".to_string(), url: "file:///x/b.dart".to_string(), chamada: (3, 7), pai: None, marcas: 0 }],
            ..Default::default()
        };
        let texto = "define i64 @df.lib.f() {\nb0:\n  ; df.pos 3 7\n  ; df.emb 9 4 0\n  %v1 = call i64 @df.lib.h()\n  ret i64 %v1\n}\n";
        let mut r = Rastro::novo();
        let saida = r.rotular(texto, &func, Some(&d), false, false);
        assert!(saida.contains(&format!(".long {}", 9 << 12 | 4)) && saida.contains(&format!(".long {}", 3 << 12 | 7)), "{saida}");
        assert!(saida.contains("\"s,s\"(ptr @df.pcf.0, ptr @df.pcf.1)"), "{saida}");
        assert!(!saida.contains("df.emb"), "{saida}");
    }

    #[test]
    fn grupo_da_closure_e_do_corpo_async() {
        let func = funcao("df.lib.f$async$q0000abcd");
        let d = DepuracaoDaFuncao {
            url: "file:///x/a.dart".to_string(),
            linha: 2,
            token: (1, 6),
            marcas: marcas_do_rastro::CORPO_ASYNC,
            corpo_async: Some(crate::hir::CorpoAsyncDoRastro { entrada: "df.lib.f$async$q0000abcd$ent".to_string(), esperas: vec![(3, 9), (4, 11)] }),
            ..Default::default()
        };
        let texto = "define i64 @df.lib.f$async$q0000abcd() {\nb0:\n  %v1 = add i64 1, 2\n  ret i64 %v1\n}\n";
        let mut r = Rastro::novo();
        let saida = r.rotular(texto, &func, Some(&d), false, false);
        let g = saida.find("asm sideeffect").expect("grupo");
        assert!(g < saida.find("%v1 = add").expect("instrução"), "{saida}");
        assert!(saida.contains("ptr @df.lib.f$async$q0000abcd$ent, ptr @df.pcf.0"), "{saida}");
        assert!(saida.contains(&format!(".long {}", 3 << 12 | 9)) && saida.contains("-.+2"), "{saida}");
        assert_eq!(func.nome_do_rastro(), "f");
        assert_eq!(funcao("dart_main$clo0$e1234abcd").nome_do_rastro(), "main.<anonymous closure>");
        assert_eq!(funcao("df.lib.C.m$inner$e0000ffff").nome_do_rastro(), "C.m.inner");
    }

    #[test]
    fn funcao_compartilhada_nao_rotula_e_nao_recebe_inlining() {
        let func = funcao("df.lib.t$tear");
        let d = DepuracaoDaFuncao { url: "file:///x/a.dart".to_string(), linha: 1, ..Default::default() };
        let texto = "define linkonce_odr i64 @\"df.lib.t$tear\"() {\nb0:\n  %v1 = call i64 @df.lib.t()\n  ret i64 %v1\n}\n";
        let mut r = Rastro::novo();
        let saida = r.rotular(texto, &func, Some(&d), false, true);
        assert!(!saida.contains("asm"), "{saida}");
        assert!(saida.contains("%v1 = call i64 @df.lib.t() noinline"), "{saida}");
    }
}
