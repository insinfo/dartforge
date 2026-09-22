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
            let nome = nome.trim_start_matches('-').split('-').next_back().unwrap_or(nome);
            // `@keyframes` sai como está: `from`, `to` e as porcentagens não
            // são seletores e não podem ganhar o atributo do escopo.
            if nome == "keyframes" {
                let corpo_ini = fim_prelúdio + 1;
                let fim = fim_do_bloco(sem_arroba, corpo_ini).ok_or(Motivo::Estilos)?;
                saida.push('@');
                saida.push_str(&comprimir(&sem_arroba[..fim_prelúdio]));
                saida.push('{');
                saida.push_str(&sem_escopo(&sem_arroba[corpo_ini..fim])?);
                saida.push('}');
                resto = sem_arroba[fim + 1..].trim_start();
                continue;
            }
            // `@media` aninha regras; o resto (`@font-face`, `@import`) tem
            // semântica própria e fica de fora.
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

/// Um seletor, com as formas que o compilador oficial trata — cada uma
/// conferida contra a saída dele (caso b16 do corpus):
///
/// | escrito | sai |
/// |---|---|
/// | `.a::before` | `.a._ngcontent-%ID%::before` |
/// | `:host(.x) .b` | `._nghost-%ID%.x .b._ngcontent-%ID%` |
/// | `:host-context(.p) .c` | dois seletores, um por posição do hospedeiro |
/// | `::ng-deep .d` | ` .d` — o que vem depois não é escopado |
/// | `.e > .f` | `.e._ngcontent-%ID% > .f._ngcontent-%ID%` |
fn um_seletor(s: &str) -> Result<String, Motivo> {
    // `::ng-deep`, `>>>` e `/deep/` soltam o escopo daí para a frente.
    for fundo in ["::ng-deep", ">>>", "/deep/"] {
        if let Some((antes, depois)) = s.split_once(fundo) {
            let antes = antes.trim();
            let escopado = if antes.is_empty() { String::new() } else { um_seletor(antes)? };
            return Ok(format!("{escopado} {}", depois.trim()));
        }
    }
    if let Some(resto) = s.strip_prefix(":host-context(") {
        // O hospedeiro pode ser o próprio elemento ou um ancestral dele.
        let (dentro, depois) = ate_fechar(resto).ok_or(Motivo::Estilos)?;
        let cauda = compostos(depois.trim())?;
        let junta = |a: String| if cauda.is_empty() { a } else { format!("{a} {cauda}") };
        return Ok(format!(
            "{},{}",
            junta(format!("{HOSPEDEIRO}{dentro}")),
            junta(format!("{dentro} {HOSPEDEIRO}"))
        ));
    }
    if let Some(resto) = s.strip_prefix(":host(") {
        let (dentro, depois) = ate_fechar(resto).ok_or(Motivo::Estilos)?;
        let cauda = compostos(depois.trim())?;
        let cabeca = format!("{HOSPEDEIRO}{dentro}");
        return Ok(if cauda.is_empty() { cabeca } else { format!("{cabeca} {cauda}") });
    }
    compostos(s)
}

/// O conteúdo até o parêntese que fecha, e o que vem depois.
fn ate_fechar(s: &str) -> Option<(&str, &str)> {
    let mut nivel = 1usize;
    for (i, c) in s.char_indices() {
        match c {
            '(' => nivel += 1,
            ')' => {
                nivel -= 1;
                if nivel == 0 {
                    return Some((&s[..i], &s[i + 1..]));
                }
            }
            _ => {}
        }
    }
    None
}

/// Cada composto da cadeia recebe o atributo — antes do pseudo-elemento,
/// depois da pseudo-classe, como o oficial faz.
fn compostos(s: &str) -> Result<String, Motivo> {
    if s.is_empty() {
        return Ok(String::new());
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
            return Err(Motivo::Estilos);
        }
        match composto.split_once("::") {
            Some((antes, pseudo)) => {
                saida.push_str(antes);
                saida.push_str(CONTEUDO);
                saida.push_str("::");
                saida.push_str(pseudo);
            }
            None => {
                saida.push_str(composto);
                saida.push_str(CONTEUDO);
            }
        }
    }
    Ok(saida)
}

/// Regras sem escopo nenhum, para dentro de `@keyframes`.
fn sem_escopo(css: &str) -> Result<String, Motivo> {
    let mut saida = String::new();
    let mut resto = css.trim();
    while !resto.is_empty() {
        let Some(abre) = resto.find('{') else { return Err(Motivo::Estilos) };
        let fim = fim_do_bloco(resto, abre + 1).ok_or(Motivo::Estilos)?;
        saida.push_str(&comprimir(&resto[..abre]));
        saida.push('{');
        saida.push_str(&declaracoes(&resto[abre + 1..fim])?);
        saida.push('}');
        resto = resto[fim + 1..].trim_start();
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

/// Espaço interno do valor é preservado (`0 auto`), o das pontas não — e o
/// que encosta em parêntese some, como na saída do compilador oficial
/// (`linear-gradient(45deg, …)`).
fn comprimir_valor(v: &str) -> String {
    let mut saida = String::with_capacity(v.len());
    let mut espaco = false;
    for c in v.trim().chars() {
        if c.is_whitespace() {
            espaco = true;
            continue;
        }
        if espaco && !saida.is_empty() && !saida.ends_with('(') && c != ')' {
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

    /// As formas do caso b16, uma a uma, contra a saída do oficial.
    #[test]
    fn formas_do_b16_iguais_ao_oficial() {
        let css = ".a::before {
  content: \"x\";
}

:host(.tema-escuro) .b {
  color: white;
}

:host-context(.pai) .c {
  color: red;
}

::ng-deep .d {
  color: blue;
}

.e > .f {
  margin: 0;
}

input[type=\"text\"] {
  border: 0;
}

@keyframes girar {
  from { opacity: 0; }
  to { opacity: 1; }
}

.g {
  animation: girar 1s;
}
";
        let esperado = ".a._ngcontent-%ID%::before{content:\"x\"}._nghost-%ID%.tema-escuro .b._ngcontent-%ID%{color:white}._nghost-%ID%.pai .c._ngcontent-%ID%,.pai ._nghost-%ID% .c._ngcontent-%ID%{color:red} .d{color:blue}.e._ngcontent-%ID% > .f._ngcontent-%ID%{margin:0}input[type=\"text\"]._ngcontent-%ID%{border:0}@keyframes girar{from{opacity:0}to{opacity:1}}.g._ngcontent-%ID%{animation:girar 1s}";
        assert_eq!(shim(css).unwrap(), esperado);
    }

    #[test]
    fn recusa_o_que_nao_sabe() {
        // Aninhamento é Sass, não CSS.
        assert!(shim(".a { .b { color: red; } }").is_err());
        assert!(shim("@font-face { font-family: x; }").is_err());
    }
}
