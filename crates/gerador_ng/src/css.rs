//! O shim de estilo do ngdart: o que o `shadow_css.dart` do ngcompiler faz.
//!
//! Sem Shadow DOM, o isolamento de estilo é por atributo: cada elemento da
//! visão recebe `_ngcontent-<id>` e cada seletor da folha ganha
//! `._ngcontent-%ID%` — o `%ID%` é trocado pelo id do componente em tempo de
//! execução. O seletor `:host` vira `._nghost-%ID%`.
//!
//! As regras foram lidas da saída do compilador oficial para um CSS que cobre
//! as formas reais (caso b15 do corpus):
//!
//! ```text
//! :host { display: block; }   ._nghost-%ID%{display:block}
//! .a .b { color: red; }       .a._ngcontent-%ID% .b._ngcontent-%ID%{color:red}
//! .c, .d { margin: 0 auto; }  .c._ngcontent-%ID%,.d._ngcontent-%ID%{margin:0 auto}
//! a:hover { … }               a:hover._ngcontent-%ID%{…}
//! @media (max-width: 600px)   @media (max-width:600px){…}
//! ```
//!
//! O que não estiver aqui é **recusado**, não traduzido por aproximação: uma
//! folha traduzida errado quebra a aparência sem quebrar a compilação, que é
//! o pior tipo de defeito.
use crate::visao::Motivo;

const CONTEUDO: &str = "._ngcontent-%ID%";
const HOSPEDEIRO: &str = "._nghost-%ID%";

/// Transforma a folha no texto que vai dentro de `styles` no
/// `<nome>.css.shim.dart`.
pub fn shim(css: &str) -> Result<String, Motivo> {
    let limpo = sem_comentarios(css);
    let mut saida = String::with_capacity(limpo.len());
    regras(&limpo, &mut saida, true)?;
    Ok(saida)
}

/// Remove `/* … */`.
fn sem_comentarios(css: &str) -> String {
    let mut saida = String::with_capacity(css.len());
    let b = css.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'*' {
            match css[i + 2..].find("*/") {
                Some(f) => i += 2 + f + 2,
                None => break,
            }
            continue;
        }
        saida.push(css[i..].chars().next().unwrap_or('\0'));
        i += css[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
    }
    saida
}

/// Percorre regras e regras-arroba no nível dado.
fn regras(css: &str, saida: &mut String, topo: bool) -> Result<(), Motivo> {
    let mut resto = css.trim();
    while !resto.is_empty() {
        if let Some(sem_arroba) = resto.strip_prefix('@') {
            let Some(fim_prelúdio) = sem_arroba.find(['{', ';']) else {
                return Err(Motivo::Estilos);
            };
            let nome = sem_arroba.split_whitespace().next().unwrap_or("");
            // `@media` aninha regras; o resto (`@keyframes`, `@font-face`,
            // `@import`) tem semântica própria — `from`/`to` não podem ganhar
            // atributo — e fica de fora.
            if nome != "media" || !topo {
                return Err(Motivo::Estilos);
            }
            let prelúdio = &sem_arroba[..fim_prelúdio];
            let corpo_ini = fim_prelúdio + 1;
            let fim = fim_do_bloco(sem_arroba, corpo_ini).ok_or(Motivo::Estilos)?;
            saida.push('@');
            saida.push_str(&comprimir(prelúdio));
            saida.push('{');
            regras(&sem_arroba[corpo_ini..fim], saida, false)?;
            saida.push('}');
            resto = sem_arroba[fim + 1..].trim_start();
            continue;
        }
        let Some(abre) = resto.find('{') else { return Err(Motivo::Estilos) };
        let seletor = &resto[..abre];
        let fim = fim_do_bloco(resto, abre + 1).ok_or(Motivo::Estilos)?;
        let corpo = &resto[abre + 1..fim];
        if corpo.contains('{') {
            return Err(Motivo::Estilos); // aninhamento: é Sass, não CSS
        }
        saida.push_str(&seletores(seletor)?);
        saida.push('{');
        saida.push_str(&declaracoes(corpo)?);
        saida.push('}');
        resto = resto[fim + 1..].trim_start();
    }
    Ok(())
}

/// Índice da chave que fecha o bloco aberto em `ini`.
fn fim_do_bloco(texto: &str, ini: usize) -> Option<usize> {
    let mut nivel = 1usize;
    for (i, c) in texto[ini..].char_indices() {
        match c {
            '{' => nivel += 1,
            '}' => {
                nivel -= 1;
                if nivel == 0 {
                    return Some(ini + i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Lista de seletores separada por vírgula, cada um com o atributo do escopo.
fn seletores(lista: &str) -> Result<String, Motivo> {
    let mut partes = Vec::new();
    for s in lista.split(',') {
        let s = s.trim();
        if s.is_empty() {
            return Err(Motivo::Estilos);
        }
        partes.push(um_seletor(s)?);
    }
    Ok(partes.join(","))
}

/// Um seletor: cada composto da cadeia recebe o atributo, no fim — inclusive
/// depois de pseudo-classe, como o oficial faz em `a:hover._ngcontent-%ID%`.
fn um_seletor(s: &str) -> Result<String, Motivo> {
    if s.contains("::") || s.contains("/deep/") || s.contains(">>>") {
        // Pseudo-elemento e travessia de sombra têm regra própria no
        // `shadow_css.dart`; ficam de fora até serem lidos de lá.
        return Err(Motivo::Estilos);
    }
    let mut saida = String::with_capacity(s.len() + CONTEUDO.len());
    for (i, composto) in s.split_whitespace().enumerate() {
        if i > 0 {
            saida.push(' ');
        }
        // Combinadores soltos (`>`, `+`, `~`) não são compostos.
        if matches!(composto, ">" | "+" | "~") {
            saida.push_str(composto);
            continue;
        }
        if composto == ":host" {
            saida.push_str(HOSPEDEIRO);
            continue;
        }
        if composto.starts_with(":host") {
            return Err(Motivo::Estilos); // `:host(...)`, `:host-context`
        }
        saida.push_str(composto);
        saida.push_str(CONTEUDO);
    }
    Ok(saida)
}

/// Declarações `prop:valor`, sem ponto e vírgula final.
fn declaracoes(corpo: &str) -> Result<String, Motivo> {
    let mut partes = Vec::new();
    for d in corpo.split(';') {
        let d = d.trim();
        if d.is_empty() {
            continue;
        }
        let Some((prop, valor)) = d.split_once(':') else { return Err(Motivo::Estilos) };
        partes.push(format!("{}:{}", prop.trim(), comprimir_valor(valor)));
    }
    Ok(partes.join(";"))
}

/// Espaço interno do valor é preservado (`0 auto`), o das pontas não.
fn comprimir_valor(v: &str) -> String {
    let mut saida = String::with_capacity(v.len());
    let mut espaco = false;
    for c in v.trim().chars() {
        if c.is_whitespace() {
            espaco = true;
            continue;
        }
        if espaco && !saida.is_empty() {
            saida.push(' ');
        }
        espaco = false;
        saida.push(c);
    }
    saida
}

/// Prelúdio de regra-arroba: espaço depois de `:` some (`max-width:600px`).
fn comprimir(texto: &str) -> String {
    let mut saida = String::with_capacity(texto.len());
    let mut espaco = false;
    for c in texto.trim().chars() {
        if c.is_whitespace() {
            espaco = true;
            continue;
        }
        if espaco && !saida.is_empty() && !saida.ends_with([':', '(']) && c != ')' {
            saida.push(' ');
        }
        espaco = false;
        saida.push(c);
    }
    saida
}

#[cfg(test)]
mod testes {
    use super::*;

    /// A saída exata do compilador oficial para o CSS do caso b15.
    #[test]
    fn folha_rica_igual_ao_oficial() {
        let css = "/* um comentário */\n:host {\n  display: block;\n}\n\n.a .b {\n  color: red;\n}\n\n.c, .d {\n  margin: 0 auto;\n}\n\na:hover {\n  text-decoration: underline;\n}\n\n@media (max-width: 600px) {\n  .a {\n    display: none;\n  }\n}\n";
        let esperado = "._nghost-%ID%{display:block}.a._ngcontent-%ID% .b._ngcontent-%ID%{color:red}.c._ngcontent-%ID%,.d._ngcontent-%ID%{margin:0 auto}a:hover._ngcontent-%ID%{text-decoration:underline}@media (max-width:600px){.a._ngcontent-%ID%{display:none}}";
        assert_eq!(shim(css).unwrap(), esperado);
    }

    /// A do caso b07.
    #[test]
    fn folha_simples_igual_ao_oficial() {
        let css = ".c { color: red; }\nspan { font-weight: bold; }\n";
        assert_eq!(
            shim(css).unwrap(),
            ".c._ngcontent-%ID%{color:red}span._ngcontent-%ID%{font-weight:bold}"
        );
    }

    #[test]
    fn recusa_o_que_nao_sabe() {
        assert!(shim("@keyframes girar { from { opacity: 0; } }").is_err());
        assert!(shim(".a::before { content: ''; }").is_err());
        assert!(shim(":host(.x) { color: red; }").is_err());
        // Aninhamento é Sass, não CSS.
        assert!(shim(".a { .b { color: red; } }").is_err());
    }
}
