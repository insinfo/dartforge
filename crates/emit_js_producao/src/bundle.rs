//! Montagem do arquivo único.
//!
//! O perfil de desenvolvimento entrega N módulos ES mais o `dart_sdk.js`, e
//! deixa o navegador resolver a ordem. Em produção existe **um** arquivo, então
//! a ordem passa a ser nossa: é a ordem topológica do grafo de `import` entre
//! os módulos, que se lê do próprio texto emitido (cada módulo declara os seus
//! `import { … } from './…'` no preâmbulo).
//!
//! Cada módulo vira uma IIFE. Isso não é estética: o preâmbulo de um módulo
//! declara `var`s com nomes gerados — `var _nome$ = dart.privateName(L$x,
//! "_nome")`, os aliases de `dartx`, `_is`/`_as`/`_eval` — que são únicos
//! **dentro** do módulo e não entre módulos. Concatenar faria a segunda
//! declaração de `_nome$` sobrescrever a primeira, e dois `_nome` de
//! bibliotecas diferentes são símbolos **diferentes**: seria um defeito
//! silencioso de privacidade. O escopo da IIFE resolve isso sem reescrever
//! texto. O que precisa ser compartilhado — os `var L$… = Object.create(
//! dart.library)` — é içado para o topo do arquivo, que é exatamente o que os
//! `import`/`export` do perfil de desenvolvimento compartilhavam.

use std::collections::{HashMap, HashSet};

/// Um módulo do perfil de desenvolvimento, já separado em partes.
pub struct Modulo {
    /// Caminho do módulo (`main.js`, `packages/x/y.js`) — é a chave do grafo.
    pub caminho: String,
    /// `var L$… = Object.create(dart.library);` — içadas para o topo do bundle.
    pub namespaces: Vec<String>,
    /// Caminhos dos módulos de que este depende (já normalizados).
    pub depende: Vec<String>,
    /// O corpo, sem as linhas de `import`, `export` e de namespace.
    pub corpo: String,
}

/// Normaliza `base/../rel` em forma canônica com `/`.
fn resolver(base: &str, rel: &str) -> String {
    let dir: Vec<&str> = match base.rfind('/') {
        Some(i) => base[..i].split('/').collect(),
        None => Vec::new(),
    };
    let mut partes: Vec<&str> = dir;
    for p in rel.split('/') {
        match p {
            "." | "" => {}
            ".." => {
                partes.pop();
            }
            outro => partes.push(outro),
        }
    }
    partes.join("/")
}

/// Separa um módulo ES emitido em namespaces içáveis, dependências e corpo.
///
/// As três formas de linha de preâmbulo que o emissor produz e que somem aqui:
/// `var L$x = Object.create(dart.library);` (içada), `export { … };` (some,
/// porque não há mais módulos) e `import { … } from './…';` (some: com um só
/// arquivo os nomes já estão em escopo — os do SDK no topo, os de outro módulo
/// pelo `var L$…` içado dele).
pub fn separar(caminho: &str, texto: &str) -> Modulo {
    let mut namespaces = Vec::new();
    let mut depende = Vec::new();
    let mut corpo = String::with_capacity(texto.len());
    for linha in texto.split_inclusive('\n') {
        let t = linha.trim_end();
        if t.starts_with("var L$") && t.ends_with("= Object.create(dart.library);") {
            namespaces.push(t.to_string());
            continue;
        }
        if t.starts_with("export {") && t.ends_with("};") {
            continue;
        }
        if t.starts_with("import ") && t.ends_with("';") {
            // `import { a as b } from './x/y.js';`
            if let Some(i) = t.rfind("from '") {
                let alvo = &t[i + 6..t.len() - 2];
                if !alvo.ends_with("dart_sdk.js") {
                    depende.push(resolver(caminho, alvo));
                }
            }
            continue;
        }
        corpo.push_str(linha);
    }
    Modulo { caminho: caminho.to_string(), namespaces, depende, corpo }
}

/// Ordem topológica dos módulos: uma dependência vem antes de quem a importa.
///
/// O emissor já funde ciclos de `import` num módulo só (`docs/EMISSAO-DDC.md`,
/// "granularidade dos módulos"), então o grafo é um DAG. Um ciclo residual não
/// é motivo para abortar — a ordem de entrada é um desempate legítimo, e o
/// contrato do DDC tolera a declaração tardia porque as classes só se tocam
/// dentro de funções. O ciclo é devolvido para quem chamar registrar.
pub fn ordenar(modulos: Vec<Modulo>) -> (Vec<Modulo>, Vec<String>) {
    let indice: HashMap<String, usize> =
        modulos.iter().enumerate().map(|(i, m)| (m.caminho.clone(), i)).collect();
    let mut estado = vec![0u8; modulos.len()]; // 0 = novo, 1 = visitando, 2 = pronto
    let mut ordem = Vec::with_capacity(modulos.len());
    let mut ciclos = Vec::new();
    // Pilha explícita: um projeto real tem centenas de módulos e cadeias longas.
    for raiz in 0..modulos.len() {
        if estado[raiz] != 0 {
            continue;
        }
        let mut pilha = vec![(raiz, 0usize)];
        estado[raiz] = 1;
        while let Some((n, k)) = pilha.pop() {
            if k < modulos[n].depende.len() {
                pilha.push((n, k + 1));
                let Some(&d) = indice.get(&modulos[n].depende[k]) else { continue };
                match estado[d] {
                    0 => {
                        estado[d] = 1;
                        pilha.push((d, 0));
                    }
                    1 => ciclos.push(format!("{} → {}", modulos[n].caminho, modulos[d].caminho)),
                    _ => {}
                }
            } else {
                estado[n] = 2;
                ordem.push(n);
            }
        }
    }
    let mut por_indice: Vec<Option<Modulo>> = modulos.into_iter().map(Some).collect();
    let saida = ordem.into_iter().filter_map(|i| por_indice[i].take()).collect();
    ciclos.sort();
    ciclos.dedup();
    (saida, ciclos)
}

/// O texto do `dart_sdk.js` sem a linha `export { … };`, que não faz sentido
/// num arquivo que não é módulo.
pub fn sdk_sem_export(sdk: &str) -> String {
    let mut out = String::with_capacity(sdk.len());
    for linha in sdk.split_inclusive('\n') {
        if linha.starts_with("export {") {
            continue;
        }
        out.push_str(linha);
    }
    out
}

/// Nome da variável de namespace da biblioteca de entrada e se `main` é `async`,
/// lidos do `main.mjs` que o emissor gerou (`import { X as m } from './p.js'`).
pub fn entrada_do_mjs(mjs: &str, modulos: &[Modulo]) -> Option<String> {
    let linha = mjs.lines().find(|l| l.contains(" as m } from './"))?;
    let ident = linha.split("import { ").nth(1)?.split(" as m }").next()?.trim();
    let caminho = linha.rsplit("from './").nth(0)?.trim_end_matches("';");
    // O `export { L$foo as foo }` sumiu no `separar`; o namespace correspondente
    // é o `var L$foo` daquele módulo. Casa por sufixo do identificador.
    let m = modulos.iter().find(|m| m.caminho == caminho)?;
    for ns in &m.namespaces {
        let var = ns.trim_start_matches("var ").split(' ').next()?;
        if var.trim_start_matches("L$") == ident || var == ident {
            return Some(var.to_string());
        }
    }
    m.namespaces.first().and_then(|ns| ns.trim_start_matches("var ").split(' ').next().map(str::to_string))
}

/// Monta o arquivo único.
pub fn montar(sdk: &str, modulos: &[Modulo], entrada: &str) -> String {
    let mut out = String::with_capacity(sdk.len() + modulos.iter().map(|m| m.corpo.len() + 64).sum::<usize>());
    out.push_str("// dartforge — perfil de produção (docs/JS-PRODUCAO.md)\n");
    // O `dart_sdk.js` do DDC é o do navegador: `self` é o objeto global.
    out.push_str("if (typeof self === 'undefined') globalThis.self = globalThis;\n");
    out.push_str("if (typeof process !== 'undefined') process.on('uncaughtException', (e) => {\n  console.error('Unhandled exception:\\n' + e);\n  process.exit(255);\n});\n");
    out.push_str(sdk);
    if !sdk.ends_with('\n') {
        out.push('\n');
    }
    // Namespaces de todas as bibliotecas, no topo: é o que substitui os
    // `import`/`export` entre módulos.
    let mut vistos = HashSet::new();
    for m in modulos {
        for ns in &m.namespaces {
            if vistos.insert(ns.clone()) {
                out.push_str(ns);
                out.push('\n');
            }
        }
    }
    for m in modulos {
        out.push_str("(function () {\n");
        out.push_str(&m.corpo);
        if !m.corpo.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("})();\n");
    }
    out.push_str(entrada);
    out.push_str(".main();\n");
    out
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn resolve_caminhos_relativos() {
        assert_eq!(resolver("main.js", "./x.js"), "x.js");
        assert_eq!(resolver("packages/a/b.js", "../../dart_sdk.js"), "dart_sdk.js");
        assert_eq!(resolver("packages/a/b.js", "../c.js"), "packages/c.js");
    }

    #[test]
    fn separa_preambulo() {
        let texto = "var L$x = Object.create(dart.library);\nexport { L$x as x };\nimport { core } from './dart_sdk.js';\nimport { y as L$y } from './sub/y.js';\nL$x.f = function () {};\n";
        let m = separar("main.js", texto);
        assert_eq!(m.namespaces, vec!["var L$x = Object.create(dart.library);"]);
        assert_eq!(m.depende, vec!["sub/y.js"]);
        assert_eq!(m.corpo, "L$x.f = function () {};\n");
    }

    #[test]
    fn ordena_dependencia_antes() {
        let a = separar("a.js", "var L$a = Object.create(dart.library);\nimport { b as L$b } from './b.js';\nL$a.f = 1;\n");
        let b = separar("b.js", "var L$b = Object.create(dart.library);\nL$b.g = 2;\n");
        let (ord, ciclos) = ordenar(vec![a, b]);
        assert!(ciclos.is_empty());
        assert_eq!(ord.iter().map(|m| m.caminho.as_str()).collect::<Vec<_>>(), vec!["b.js", "a.js"]);
    }

    #[test]
    fn ciclo_nao_aborta() {
        let a = separar("a.js", "var L$a = Object.create(dart.library);\nimport { b as L$b } from './b.js';\n");
        let b = separar("b.js", "var L$b = Object.create(dart.library);\nimport { a as L$a } from './a.js';\n");
        let (ord, ciclos) = ordenar(vec![a, b]);
        assert_eq!(ord.len(), 2);
        assert_eq!(ciclos.len(), 1);
    }
}
