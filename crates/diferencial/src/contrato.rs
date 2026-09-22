//! Gera `docs/CONTRATO-DDC.md`: cada programa do corpus e o JS que o `dartdevc` emite.

use std::fmt::Write;

use crate::corpus::{Programa, tema};
use crate::oraculos::{Ambiente, compilar_ddc};

/// Remove do JS do `dartdevc` o que se repete em todo módulo: cabeçalho de módulo,
/// import do SDK, `_checkModuleNullSafetyMode`, tabela `CT` vazia, `addRules`,
/// `trackLibraries` e o source map. As linhas `var $x = dartx.x;` ficam: dizem quais
/// membros de tipos nativos o módulo usa.
pub fn cortar_preambulo(js: &str) -> String {
    let linhas: Vec<&str> = js.lines().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < linhas.len() {
        let l = linhas[i];
        let t = l.trim_start();
        if (t.starts_with("var ") && t.contains("= Object.create(dart.library);"))
            || t.starts_with("export {")
            || (t.starts_with("import {") && t.contains("dart_sdk.js"))
            || t.starts_with("dart._checkModuleNullSafetyMode(")
            || t.starts_with("dart_rti._Universe.addRules(")
            || t.starts_with("dart_rti._Universe.addErasedTypes(")
            || t.starts_with("//# sourceMappingURL=")
        {
            i += 1;
            continue;
        }
        if t.starts_with("const CT = Object.create({") && i + 2 < linhas.len() && linhas[i + 1].trim() == "_: () => (C, CT)" && linhas[i + 2].trim() == "});" {
            i += 3;
            continue;
        }
        if t.starts_with("dart.trackLibraries(") {
            while i < linhas.len() && !linhas[i].trim_start().starts_with("}, null);") {
                i += 1;
            }
            i += 1;
            continue;
        }
        out.push_str(l);
        out.push('\n');
        i += 1;
    }
    out.trim_end().to_string() + "\n"
}

/// Compila cada programa e monta o Markdown, agrupado por tema.
pub fn gerar(amb: &Ambiente, programas: &[Programa]) -> String {
    let mut doc = String::new();
    doc.push_str("# Contrato do DDC: o corpus compilado pelo `dartdevc`\n\n");
    doc.push_str("Gerado por `cargo run -p dartforge-diferencial -- contrato`; não editar à mão.\n");
    doc.push_str("Para cada programa de `corpus/js/`, o Dart e o JS (`--modules=es6`) que o\n`dartdevc` 3.6.2 emite, sem o que se repete em todo módulo:\n\n");
    doc.push_str("```js\nvar nome = Object.create(dart.library);      // uma por biblioteca do módulo\nexport { nome };                             // ou `export { nome as alias }`\nimport { dart_rti, core, dart, dartx } from 'dart_sdk.js';\ndart._checkModuleNullSafetyMode(true);\nconst CT = Object.create({ _: () => (C, CT) });   // tabela de constantes (fica quando não é vazia)\n… corpo …\ndart_rti._Universe.addRules(dart.typeUniverse, JSON.parse('{…}'));   // regras de subtipagem (cortadas)\ndart.trackLibraries(\"nome\", { \"org-dartlang-app:/nome.dart\": nome }, { /* parts */ }, null);\n```\n\n");
    doc.push_str("Um `main.mjs` que executa o módulo no Node:\n\n```js\nimport { nome as m } from './nome.js';\nm.main();   // main assíncrono: o Future roda no laço de eventos do Node\n```\n\n");
    let mut tema_atual = "";
    let mut indice = String::new();
    let mut corpo = String::new();
    for p in programas {
        let t = tema(&p.nome);
        if t != tema_atual {
            tema_atual = t;
            let _ = writeln!(corpo, "\n## {t}\n");
            let _ = writeln!(indice, "- {t}");
        }
        let _ = writeln!(indice, "  - {}", p.nome);
        let _ = writeln!(corpo, "### {}\n", p.nome);
        for a in &p.arquivos {
            let fonte = std::fs::read_to_string(a).unwrap_or_default();
            if p.arquivos.len() > 1 {
                let rel = a.strip_prefix(p.diretorio()).unwrap_or(a).to_string_lossy().replace('\\', "/");
                let _ = writeln!(corpo, "`{rel}`:\n");
            }
            let _ = writeln!(corpo, "```dart\n{}\n```\n", fonte.trim_end());
        }
        let dir = amb.dir_saida("contrato", p);
        match compilar_ddc(amb, p, &dir) {
            Ok(js) => {
                let _ = writeln!(corpo, "```js\n{}```\n", cortar_preambulo(&js));
            }
            Err(s) => {
                let _ = writeln!(corpo, "dartdevc falhou (código {}):\n\n```\n{}\n```\n", s.codigo, s.stderr.trim_end());
            }
        }
    }
    doc.push_str("## Índice\n\n");
    doc.push_str(&indice);
    doc.push_str(&corpo);
    doc
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn corta() {
        let js = "var hello = Object.create(dart.library);\nexport { hello };\nimport { core, dart } from 'dart_sdk.js';\nvar $truncate = dartx.truncate;\ndart._checkModuleNullSafetyMode(true);\nconst CT = Object.create({\n  _: () => (C, CT)\n});\nhello.main = function main() {\n  core.print(\"a\");\n};\ndart_rti._Universe.addRules(dart.typeUniverse, JSON.parse('{}'));\ndart.trackLibraries(\"hello\", {\n  \"org-dartlang-app:/hello.dart\": hello\n}, {\n}, null);\n\n//# sourceMappingURL=hello.js.map\n";
        assert_eq!(cortar_preambulo(js), "var $truncate = dartx.truncate;\nhello.main = function main() {\n  core.print(\"a\");\n};\n");
    }
}
