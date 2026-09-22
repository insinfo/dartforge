//! O Sass que os projetos reais usam — e só ele.
//!
//! `styleUrls: ['x.css']` aponta para um arquivo que não existe no disco: o
//! `sass_builder` o gera de `x.scss`. Sem isto, nenhum componente com folha
//! de estilo pode ser gerado sem o `build_runner`.
//!
//! Medido nos 139 `.scss` do `new_sali/frontend`: **nenhum** usa `@use`,
//! `@import`, `@mixin`, `@include`, `@extend`, `@function`, `@each`, `@for`,
//! `@if`, `map-get` ou placeholder. O que usam é aninhamento com `&`,
//! variáveis, comentários `//` e `@media`. É esse o subconjunto aqui.
//!
//! O resto é **recusado**. Como a saída ainda passa pelo shim do ngdart, que
//! remove comentários e minifica, o CSS intermediário não precisa sair byte a
//! byte igual ao do `sass_builder` — o que precisa bater é o
//! `.css.shim.dart`, e é contra ele que a verificação roda.
use crate::visao::Motivo;
use std::collections::HashMap;

/// Compila Sass para CSS.
pub fn compilar(fonte: &str) -> Result<String, Motivo> {
    let sem_comentario = tirar_comentarios(fonte);
    let mut variaveis = HashMap::new();
    let mut saida = String::with_capacity(sem_comentario.len());
    blocos(&sem_comentario, "", &mut variaveis, &mut saida)?;
    Ok(saida)
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
        if let Some(pv) = proximo_ponto {
            if proximo_abre.is_none_or(|a| pv < a) {
                let decl = resto[..pv].trim().to_string();
                resto = resto[pv + 1..].trim_start();
                if let Some((nome, valor)) = decl.strip_prefix('$').and_then(|d| d.split_once(':'))
                {
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
        if !decls.trim().is_empty() {
            saida.push_str(&seletor);
            saida.push('{');
            saida.push_str(&substituir(&decls, variaveis)?);
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
        let Some(v) = variaveis.get(&nome) else { return Err(Motivo::Estilos) };
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
            ".a._ngcontent-%ID%{color:red}.a:hover._ngcontent-%ID%{color:blue}.a._ngcontent-%ID% .b._ngcontent-%ID%{color:green}"
        );
    }


    #[test]
    fn variavel() {
        let css = compilar("$c: red;\n.a { color: $c; }\n").unwrap();
        assert!(css.contains("color: red"), "{css}");
    }

    #[test]
    fn recusa_o_que_nao_sabe() {
        assert!(compilar("@mixin x { color: red; }").is_err());
        assert!(compilar("@use 'sass:math';").is_err());
        assert!(compilar(".a { color: $indefinida; }").is_err());
        assert!(compilar(".#{$x} { color: red; }").is_err());
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
