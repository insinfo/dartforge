//! O Sass que os projetos reais usam — e só ele.
//!
//! `styleUrls: ['x.css']` aponta para um arquivo que não existe no disco: o
//! `sass_builder` o gera de `x.scss`. Sem isto, nenhum componente com folha
//! de estilo pode ser gerado sem o `build_runner`.
//!
//! Medido nos 155 `.scss` do `new_sali/frontend` (139 em `lib/`, 16 em
//! `web/`): aninhamento com `&`, variáveis, comentários `//`, `@media` e —
//! na folha global `web/style.scss`, que o `index.html` carrega — `@use
//! 'nome' as *`. Nenhum usa `@mixin`, `@include`, `@extend`, `@function`,
//! `@each`, `@for`, `@if`, `map-get` ou placeholder. É esse o subconjunto
//! aqui.
//!
//! O resto é **recusado**. Como a saída ainda passa pelo shim do ngdart, que
//! remove comentários e minifica, o CSS intermediário não precisa sair byte a
//! byte igual ao do `sass_builder` — o que precisa bater é o
//! `.css.shim.dart`, e é contra ele que a verificação roda.
use crate::visao::Motivo;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Compila Sass para CSS, sem resolver módulos (`@use`/`@import`).
pub fn compilar(fonte: &str) -> Result<String, Motivo> {
    compilar_em(fonte, None)
}

/// Compila Sass resolvendo `@use` e `@import` a partir de `dir`.
///
/// `@use 'nome' as *` carrega o módulo e põe os membros dele no escopo — na
/// prática, para gerar CSS: emite o CSS do módulo antes e compartilha as
/// variáveis. Namespace explícito (`nome.$x`) não aparece nos projetos e é
/// recusado.
pub fn compilar_em(fonte: &str, dir: Option<&Path>) -> Result<String, Motivo> {
    let mut variaveis = HashMap::new();
    let mut vistos = Vec::new();
    compilar_com(fonte, dir, &mut variaveis, &mut vistos)
}

fn compilar_com(
    fonte: &str,
    dir: Option<&Path>,
    variaveis: &mut HashMap<String, String>,
    vistos: &mut Vec<PathBuf>,
) -> Result<String, Motivo> {
    let sem_comentario = tirar_comentarios(fonte);
    let (modulos, corpo) = separar_modulos(&sem_comentario)?;
    let mut saida = String::with_capacity(sem_comentario.len());
    for nome in modulos {
        let dir = dir.ok_or(Motivo::Estilos)?;
        let caminho = achar_modulo(dir, &nome).ok_or(Motivo::Estilos)?;
        if vistos.contains(&caminho) {
            continue; // já carregado: `@use` carrega uma vez só
        }
        vistos.push(caminho.clone());
        let texto = std::fs::read_to_string(&caminho).map_err(|_| Motivo::Estilos)?;
        let css = compilar_com(&texto, caminho.parent(), variaveis, vistos)?;
        saida.push_str(&css);
    }
    blocos(&corpo, "", variaveis, &mut saida)?;
    Ok(saida)
}

/// Tira os `@use`/`@import` do começo e devolve os nomes dos módulos.
fn separar_modulos(fonte: &str) -> Result<(Vec<String>, String), Motivo> {
    let mut nomes = Vec::new();
    let mut corpo = String::with_capacity(fonte.len());
    for linha in fonte.lines() {
        let t = linha.trim();
        let regra = t
            .strip_prefix("@use ")
            .or_else(|| t.strip_prefix("@import "));
        let Some(regra) = regra else {
            corpo.push_str(linha);
            corpo.push('\n');
            continue;
        };
        let regra = regra.trim().trim_end_matches(';').trim();
        // `as *` é o único apelido aceito: com namespace, os membros seriam
        // acessados por `nome.$x`, que não sabemos traduzir.
        let (alvos, apelido) = match regra.split_once(" as ") {
            Some((a, b)) => (a, Some(b.trim())),
            None => (regra, None),
        };
        if apelido.is_some_and(|a| a != "*") {
            return Err(Motivo::Estilos);
        }
        for alvo in alvos.split(',') {
            let nome = alvo.trim().trim_matches(['\'', '"']).trim();
            if nome.is_empty() || nome.starts_with("sass:") {
                return Err(Motivo::Estilos);
            }
            nomes.push(nome.to_string());
        }
    }
    Ok((nomes, corpo))
}

/// `nome` -> `nome.scss` ou `_nome.scss` (parcial do Sass), com `.scss`
/// opcional no que foi escrito.
fn achar_modulo(dir: &Path, nome: &str) -> Option<PathBuf> {
    let base = nome.strip_suffix(".scss").unwrap_or(nome);
    let arquivo = Path::new(base).file_name()?.to_string_lossy().to_string();
    let pai = Path::new(base)
        .parent()
        .map(|p| dir.join(p))
        .unwrap_or_else(|| dir.to_path_buf());
    for candidato in [format!("{arquivo}.scss"), format!("_{arquivo}.scss")] {
        let c = pai.join(candidato);
        if c.is_file() {
            return Some(c);
        }
    }
    None
}

/// Normalizações de valor que o Sass faz na saída e que o CSS escrito à mão
/// não teria: sem elas a comparação com o `sass_builder` acusa diferença onde
/// o navegador renderiza igual.
///
/// As quatro foram achadas comparando os 144 `.scss` do new_sali com a saída
/// dele (`--example conferir-sass`):
///
/// | escrito | Sass emite |
/// |---|---|
/// | `0.5rem` | `.5rem` |
/// | `white` | `#fff` |
/// | `transparent` | `rgba(0,0,0,0)` |
/// | `"\e9fe"` | o caractere U+E9FE |
fn normalizar(decls: &str) -> String {
    let mut saida = String::with_capacity(decls.len());
    let mut resto = decls;
    // Percorre declaração a declaração para não mexer no nome da propriedade.
    while !resto.is_empty() {
        let fim = resto.find(';').map(|i| i + 1).unwrap_or(resto.len());
        let (decl, r) = resto.split_at(fim);
        resto = r;
        match decl.split_once(':') {
            // Propriedade customizada guarda o valor como foi escrito.
            Some((prop, valor)) if !prop.trim().starts_with("--") => {
                saida.push_str(prop);
                saida.push(':');
                saida.push_str(&valor_normalizado(valor));
            }
            _ => saida.push_str(decl),
        }
    }
    saida
}

/// Normaliza os tokens de um valor, preservando strings e parênteses.
///
/// Valor que chama função CSS (`var()`, `calc()`, `rgba()`) sai como foi
/// escrito: o Sass não o reescreve, porque não o interpreta.
fn valor_normalizado(valor: &str) -> String {
    if valor.contains("var(") || valor.contains("calc(") || valor.contains('(') {
        return valor.to_string();
    }
    let mut saida = String::with_capacity(valor.len());
    let mut resto = valor;
    while !resto.is_empty() {
        let c = resto.chars().next().unwrap();
        if c == '"' || c == '\'' {
            // String: só os escapes mudam.
            let fim = fim_da_string(resto, c);
            saida.push_str(&decodificar_escapes(&resto[..fim]));
            resto = &resto[fim..];
            continue;
        }
        let fim = resto
            .char_indices()
            .find(|(_, x)| {
                *x == '"'
                    || *x == '\''
                    || x.is_whitespace()
                    || *x == ','
                    || *x == '('
                    || *x == ')'
                    || *x == ';'
                    || *x == '!'
            })
            .map(|(i, _)| i)
            .unwrap_or(resto.len());
        let token = if fim == 0 {
            &resto[..c.len_utf8()]
        } else {
            &resto[..fim]
        };
        saida.push_str(&token_normalizado(token));
        resto = &resto[token.len()..];
    }
    saida
}

/// Fim da string literal aberta em 0 (índice depois da aspa de fechamento).
fn fim_da_string(texto: &str, aspa: char) -> usize {
    let mut escape = false;
    for (i, c) in texto.char_indices().skip(1) {
        if escape {
            escape = false;
            continue;
        }
        if c == '\\' {
            escape = true;
            continue;
        }
        if c == aspa {
            return i + c.len_utf8();
        }
    }
    texto.len()
}

/// `\e9fe` vira o caractere, como o Sass faz ao normalizar a string.
fn decodificar_escapes(s: &str) -> String {
    if !s.contains('\\') {
        return s.to_string();
    }
    let mut saida = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            saida.push(c);
            continue;
        }
        let mut hex = String::new();
        while hex.len() < 6 && chars.peek().is_some_and(|x| x.is_ascii_hexdigit()) {
            hex.push(chars.next().unwrap_or('0'));
        }
        if hex.is_empty() {
            saida.push('\\');
            continue;
        }
        // Um espaço depois do escape é o terminador e não faz parte do valor.
        if chars.peek() == Some(&' ') {
            chars.next();
        }
        match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
            Some(x) => saida.push(x),
            None => {
                saida.push('\\');
                saida.push_str(&hex);
            }
        }
    }
    saida
}

/// Um token isolado: número com zero à esquerda e nome de cor.
fn token_normalizado(t: &str) -> String {
    if t == "transparent" {
        return "rgba(0,0,0,0)".to_string();
    }
    if let Some(hex) = cor_em_hex(t) {
        return hex.to_string();
    }
    // `#ffffff` -> `#fff`, quando os três pares se repetem.
    if let Some(d) = t.strip_prefix('#')
        && d.len() == 6
        && d.chars().all(|c| c.is_ascii_hexdigit())
    {
        let b = d.as_bytes();
        if b[0].eq_ignore_ascii_case(&b[1])
            && b[2].eq_ignore_ascii_case(&b[3])
            && b[4].eq_ignore_ascii_case(&b[5])
        {
            return format!(
                "#{}{}{}",
                d.as_bytes()[0] as char,
                d.as_bytes()[2] as char,
                d.as_bytes()[4] as char
            )
            .to_lowercase();
        }
    }
    // `0.5rem` -> `.5rem`, `-0.5rem` -> `-.5rem`.
    if let Some(r) = t.strip_prefix("0.")
        && r.starts_with(|c: char| c.is_ascii_digit())
    {
        return format!(".{r}");
    }
    if let Some(r) = t.strip_prefix("-0.")
        && r.starts_with(|c: char| c.is_ascii_digit())
    {
        return format!("-.{r}");
    }
    t.to_string()
}

/// Nomes de cor que o Sass troca por hexadecimal por ser mais curto. A lista
/// é só dos casos em que o hexadecimal ganha; `red` continua `red` porque
/// `#f00` é maior.
fn cor_em_hex(nome: &str) -> Option<&'static str> {
    Some(match nome {
        "white" => "#fff",
        "black" => "#000",
        "aqua" => "#0ff",
        "blue" => "#00f",
        "fuchsia" => "#f0f",
        "lime" => "#0f0",
        "yellow" => "#ff0",
        "cyan" => "#0ff",
        "magenta" => "#f0f",
        "darkgray" => "#a9a9a9",
        "darkgrey" => "#a9a9a9",
        "lightgray" => "#d3d3d3",
        "lightgrey" => "#d3d3d3",
        _ => return None,
    })
}

/// Tira `//` até o fim da linha, sem confundir com `://` de URL.
fn tirar_comentarios(fonte: &str) -> String {
    let mut saida = String::with_capacity(fonte.len());
    for linha in fonte.lines() {
        let mut corte = None;
        let b = linha.as_bytes();
        for i in 0..b.len().saturating_sub(1) {
            if b[i] == b'/' && b[i + 1] == b'/' && (i == 0 || b[i - 1] != b':') {
                corte = Some(i);
                break;
            }
        }
        match corte {
            Some(i) => saida.push_str(&linha[..i]),
            None => saida.push_str(linha),
        }
        saida.push('\n');
    }
    saida
}

/// Percorre os blocos de um nível, achatando o aninhamento.
fn blocos(
    fonte: &str,
    pai: &str,
    variaveis: &mut HashMap<String, String>,
    saida: &mut String,
) -> Result<(), Motivo> {
    let mut resto = fonte.trim();
    while !resto.is_empty() {
        // Declaração ou variável soltas neste nível.
        let proximo_abre = resto.find('{');
        let proximo_ponto = resto.find(';');
        if let Some(pv) = proximo_ponto
            && proximo_abre.is_none_or(|a| pv < a)
        {
            let decl = resto[..pv].trim().to_string();
            resto = resto[pv + 1..].trim_start();
            if let Some((nome, valor)) = decl.strip_prefix('$').and_then(|d| d.split_once(':')) {
                if valor.contains("!default") {
                    return Err(Motivo::Estilos);
                }
                let valor = substituir(valor.trim(), variaveis)?;
                variaveis.insert(nome.trim().to_string(), valor);
                continue;
            }
            // Declaração fora de regra não existe em CSS.
            if pai.is_empty() {
                return Err(Motivo::Estilos);
            }
            return Err(Motivo::Estilos);
        }
        let Some(abre) = proximo_abre else {
            if resto.trim().is_empty() {
                break;
            }
            return Err(Motivo::Estilos);
        };
        let cabeca = resto[..abre].trim().to_string();
        let fim = fim_do_bloco(resto, abre + 1).ok_or(Motivo::Estilos)?;
        let corpo = &resto[abre + 1..fim];
        resto = resto[fim + 1..].trim_start();

        if cabeca.starts_with('@') {
            // `@media` aninha regras e é o único que aparece nos projetos.
            if !cabeca.starts_with("@media") {
                return Err(Motivo::Estilos);
            }
            saida.push_str(&cabeca);
            saida.push('{');
            blocos(corpo, pai, variaveis, saida)?;
            saida.push('}');
            continue;
        }
        if cabeca.contains("#{") {
            return Err(Motivo::Estilos); // interpolação de seletor
        }
        let seletor = juntar(pai, &cabeca);
        // Declarações deste nível saem antes das regras aninhadas, como o
        // Sass emite.
        let (decls, aninhados) = separar(corpo)?;
        if tem_funcao_de_cor(&substituir(&decls, variaveis)?) {
            return Err(Motivo::Estilos);
        }
        if !decls.trim().is_empty() {
            saida.push_str(&seletor);
            saida.push('{');
            saida.push_str(&normalizar(&substituir(&decls, variaveis)?));
            saida.push('}');
        }
        blocos(&aninhados, &seletor, variaveis, saida)?;
    }
    Ok(())
}

/// Separa as declarações do nível das regras aninhadas.
fn separar(corpo: &str) -> Result<(String, String), Motivo> {
    let mut decls = String::new();
    let mut aninhados = String::new();
    let mut resto = corpo;
    loop {
        let abre = resto.find('{');
        let Some(abre) = abre else {
            decls.push_str(resto);
            break;
        };
        // Tudo até o `;` anterior ao `{` é declaração; o resto é o cabeçalho
        // da regra aninhada.
        let corte = resto[..abre].rfind(';').map(|i| i + 1).unwrap_or(0);
        decls.push_str(&resto[..corte]);
        let fim = fim_do_bloco(resto, abre + 1).ok_or(Motivo::Estilos)?;
        aninhados.push_str(&resto[corte..=fim]);
        aninhados.push('\n');
        resto = &resto[fim + 1..];
    }
    Ok((decls, aninhados))
}

/// Junta o seletor do pai com o do filho, resolvendo `&`.
fn juntar(pai: &str, filho: &str) -> String {
    if pai.is_empty() {
        return filho.split_whitespace().collect::<Vec<_>>().join(" ");
    }
    let mut partes = Vec::new();
    for f in filho.split(',') {
        let f = f.trim();
        for p in pai.split(',') {
            let p = p.trim();
            partes.push(if f.contains('&') {
                f.replace('&', p)
            } else {
                format!("{p} {f}")
            });
        }
    }
    partes.join(",")
}

/// `rgb(…)`/`hsl(…)` o Sass avalia e escreve como cor (`rgb(47, 88, 141)`
/// sai `#2f588d`, visto no `visualiza_norma_page` do new_sali). Não
/// avaliamos funções: recusa.
fn tem_funcao_de_cor(texto: &str) -> bool {
    let t = texto.to_ascii_lowercase();
    ["rgb(", "hsl(", "hsla("].iter().any(|f| {
        t.match_indices(f).any(|(i, _)| {
            i == 0 || !t.as_bytes()[i - 1].is_ascii_alphanumeric() && t.as_bytes()[i - 1] != b'-'
        })
    })
}

/// Troca `$nome` pelo valor. Variável desconhecida é recusa, não texto vazio.
fn substituir(texto: &str, variaveis: &HashMap<String, String>) -> Result<String, Motivo> {
    if !texto.contains('$') {
        return Ok(texto.to_string());
    }
    let mut saida = String::with_capacity(texto.len());
    let mut resto = texto;
    while let Some(i) = resto.find('$') {
        saida.push_str(&resto[..i]);
        let nome: String = resto[i + 1..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        if nome.is_empty() {
            return Err(Motivo::Estilos);
        }
        let Some(v) = variaveis.get(&nome) else {
            return Err(Motivo::Estilos);
        };
        saida.push_str(v);
        resto = &resto[i + 1 + nome.len()..];
    }
    saida.push_str(resto);
    Ok(saida)
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

#[cfg(test)]
mod testes {
    use super::*;

    /// O espaço do CSS intermediário não importa — quem normaliza é o shim.
    /// O que este teste garante é que `//` some e que `://` de URL fica.
    #[test]
    fn comentario_de_linha_some_e_url_fica() {
        let fonte = "// fora
.a {
  background: url(http://x/y.png); // atrás
}
";
        let shim = crate::css::shim(&compilar(fonte).unwrap()).unwrap();
        assert_eq!(shim, ".a._ngcontent-%ID%{background:url(http://x/y.png)}");
    }

    #[test]
    fn aninhamento_e_e_comercial() {
        let fonte = ".a {
  color: red;
  &:hover { color: blue; }
  .b { color: green; }
}
";
        let shim = crate::css::shim(&compilar(fonte).unwrap()).unwrap();
        assert_eq!(
            shim,
            ".a._ngcontent-%ID%{color:red}.a:hover._ngcontent-%ID%{color:#00f}.a._ngcontent-%ID% .b._ngcontent-%ID%{color:green}"
        );
    }

    #[test]
    fn variavel() {
        let css = compilar("$c: red;\n.a { color: $c; }\n").unwrap();
        assert!(css.contains("color: red"), "{css}");
    }

    /// A folha global do new_sali (`web/style.scss`) começa com quatro
    /// `@use 'x' as *`. O CSS do módulo sai antes, e as variáveis dele valem
    /// no arquivo que o usa.
    #[test]
    fn use_carrega_o_modulo() {
        let dir = tempfile::tempdir().expect("tmp");
        std::fs::write(
            dir.path().join("tema.scss"),
            "$c: red;\n.tema { color: $c; }\n",
        )
        .expect("escreve");
        let css = compilar_em("@use 'tema' as *;\n.a { color: $c; }\n", Some(dir.path()))
            .expect("compila");
        let shim = crate::css::shim(&css).unwrap();
        assert_eq!(
            shim,
            ".tema._ngcontent-%ID%{color:red}.a._ngcontent-%ID%{color:red}"
        );
    }

    #[test]
    fn recusa_o_que_nao_sabe() {
        assert!(compilar("@mixin x { color: red; }").is_err());
        assert!(compilar_em("@use 'sass:math';", None).is_err());
        // Namespace explícito não sabemos traduzir.
        assert!(compilar_em("@use 'tema' as t;", None).is_err());
        assert!(compilar(".a { color: $indefinida; }").is_err());
        assert!(compilar(".#{$x} { color: red; }").is_err());
        // Função de cor o Sass avalia (`rgb(47, 88, 141)` vira `#2f588d`).
        assert!(compilar(":host { background: rgb(47, 88, 141); }").is_err());
        assert!(compilar("$c: hsl(0, 0%, 0%);\n.a { color: $c; }").is_err());
    }

    /// O caso real do `arvore_organograma.scss`, cujo shim oficial é
    /// `._nghost-%ID%{position:relative}.fancytree-container._ngcontent-%ID%{overflow:unset}`.
    #[test]
    fn caso_real_do_new_sali() {
        let fonte = ":host {\n  position: relative;\n}\n\n.fancytree-container {\n  //     --ft-node-padding-y: 0.25rem;\n  overflow: unset;\n}\n";
        let css = compilar(fonte).unwrap();
        let shim = crate::css::shim(&css).unwrap();
        assert_eq!(
            shim,
            "._nghost-%ID%{position:relative}.fancytree-container._ngcontent-%ID%{overflow:unset}"
        );
    }
}
