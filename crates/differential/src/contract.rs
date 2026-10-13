//! Generates `docs/CONTRATO-DDC.md`: each corpus program and the JS that `dartdevc` emits.
//!
//! That document is in Portuguese: the text written by [`generate`] is its
//! content and stays as it is.

use std::fmt::Write;

use crate::corpus::{Program, theme};
use crate::oracles::{Environment, compile_ddc};

/// Removes from `dartdevc`'s JS what repeats in every module: the module
/// header, the SDK import, `_checkModuleNullSafetyMode`, an empty `CT` table,
/// `addRules`, `trackLibraries` and the source map. The `var $x = dartx.x;`
/// lines stay: they say which members of native types the module uses.
pub fn strip_preamble(js: &str) -> String {
    let lines: Vec<&str> = js.lines().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i];
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
        if t.starts_with("const CT = Object.create({") && i + 2 < lines.len() && lines[i + 1].trim() == "_: () => (C, CT)" && lines[i + 2].trim() == "});" {
            i += 3;
            continue;
        }
        if t.starts_with("dart.trackLibraries(") {
            while i < lines.len() && !lines[i].trim_start().starts_with("}, null);") {
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

/// Compiles each program and builds the Markdown, grouped by theme.
pub fn generate(env: &Environment, programs: &[Program]) -> String {
    let mut doc = String::new();
    doc.push_str("# Contrato do DDC: o corpus compilado pelo `dartdevc`\n\n");
    doc.push_str("Gerado por `cargo run -p dartforge-differential -- contract`; não editar à mão.\n");
    doc.push_str("Para cada programa de `corpus/js/`, o Dart e o JS (`--modules=es6`) que o\n`dartdevc` 3.6.2 emite, sem o que se repete em todo módulo:\n\n");
    doc.push_str("```js\nvar nome = Object.create(dart.library);      // uma por biblioteca do módulo\nexport { nome };                             // ou `export { nome as alias }`\nimport { dart_rti, core, dart, dartx } from 'dart_sdk.js';\ndart._checkModuleNullSafetyMode(true);\nconst CT = Object.create({ _: () => (C, CT) });   // tabela de constantes (fica quando não é vazia)\n… corpo …\ndart_rti._Universe.addRules(dart.typeUniverse, JSON.parse('{…}'));   // regras de subtipagem (cortadas)\ndart.trackLibraries(\"nome\", { \"org-dartlang-app:/nome.dart\": nome }, { /* parts */ }, null);\n```\n\n");
    doc.push_str("Um `main.mjs` que executa o módulo no Node:\n\n```js\nimport { nome as m } from './nome.js';\nm.main();   // main assíncrono: o Future roda no laço de eventos do Node\n```\n\n");
    let mut current_theme = "";
    let mut index = String::new();
    let mut body = String::new();
    for p in programs {
        let t = theme(&p.name);
        if t != current_theme {
            current_theme = t;
            let _ = writeln!(body, "\n## {t}\n");
            let _ = writeln!(index, "- {t}");
        }
        let _ = writeln!(index, "  - {}", p.name);
        let _ = writeln!(body, "### {}\n", p.name);
        for a in &p.files {
            let source = std::fs::read_to_string(a).unwrap_or_default();
            if p.files.len() > 1 {
                let rel = a.strip_prefix(p.dir()).unwrap_or(a).to_string_lossy().replace('\\', "/");
                let _ = writeln!(body, "`{rel}`:\n");
            }
            let _ = writeln!(body, "```dart\n{}\n```\n", source.trim_end());
        }
        let dir = env.output_dir("contract", p);
        match compile_ddc(env, p, &dir) {
            Ok(js) => {
                let _ = writeln!(body, "```js\n{}```\n", strip_preamble(&js));
            }
            Err(s) => {
                let _ = writeln!(body, "dartdevc falhou (código {}):\n\n```\n{}\n```\n", s.code, s.stderr.trim_end());
            }
        }
    }
    doc.push_str("## Índice\n\n");
    doc.push_str(&index);
    doc.push_str(&body);
    doc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips() {
        let js = "var hello = Object.create(dart.library);\nexport { hello };\nimport { core, dart } from 'dart_sdk.js';\nvar $truncate = dartx.truncate;\ndart._checkModuleNullSafetyMode(true);\nconst CT = Object.create({\n  _: () => (C, CT)\n});\nhello.main = function main() {\n  core.print(\"a\");\n};\ndart_rti._Universe.addRules(dart.typeUniverse, JSON.parse('{}'));\ndart.trackLibraries(\"hello\", {\n  \"org-dartlang-app:/hello.dart\": hello\n}, {\n}, null);\n\n//# sourceMappingURL=hello.js.map\n";
        assert_eq!(strip_preamble(js), "var $truncate = dartx.truncate;\nhello.main = function main() {\n  core.print(\"a\");\n};\n");
    }
}
