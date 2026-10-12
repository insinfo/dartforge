//! Assembly of the single file.
//!
//! The development profile delivers N ES modules plus `dart_sdk.js`, and
//! leaves the order to the browser. In production there is **one** file, so
//! the order becomes ours: it is the topological order of the `import` graph
//! between the modules, read from the emitted text itself (each module
//! declares its `import { … } from './…'` in the preamble).
//!
//! Each module becomes an IIFE. This is not aesthetics: a module's preamble
//! declares `var`s with generated names — `var _nome$ = dart.privateName(L$x,
//! "_nome")`, the `dartx` aliases, `_is`/`_as`/`_eval` — which are unique
//! **within** the module but not across modules. Concatenating would make
//! the second declaration of `_nome$` overwrite the first, and two `_nome`
//! from different libraries are **different** symbols: it would be a silent
//! privacy defect. The IIFE's scope solves this without rewriting text. What
//! must be shared — the `var L$… = Object.create(dart.library)` — is hoisted
//! to the top of the file, which is exactly what the development profile's
//! `import`/`export` used to share.

use std::collections::{HashMap, HashSet};

/// A development-profile module, already split into parts.
pub struct Module {
    /// Path of the module (`main.js`, `packages/x/y.js`) — the graph key.
    pub path: String,
    /// `var L$… = Object.create(dart.library);` — hoisted to the top of the
    /// bundle.
    pub namespaces: Vec<String>,
    /// Paths of the modules this one depends on (already normalized).
    pub deps: Vec<String>,
    /// The body, without the `import`, `export` and namespace lines.
    pub body: String,
}

/// Normalizes `base/../rel` into canonical form with `/`.
fn resolve(base: &str, rel: &str) -> String {
    let dir: Vec<&str> = match base.rfind('/') {
        Some(i) => base[..i].split('/').collect(),
        None => Vec::new(),
    };
    let mut parts: Vec<&str> = dir;
    for p in rel.split('/') {
        match p {
            "." | "" => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

/// Splits an emitted ES module into hoistable namespaces, dependencies and
/// body.
///
/// The three preamble line shapes the emitter produces and that disappear
/// here: `var L$x = Object.create(dart.library);` (hoisted), `export { … };`
/// (gone, because there are no more modules) and `import { … } from './…';`
/// (gone: with a single file the names are already in scope — the SDK's at
/// the top, another module's through its hoisted `var L$…`).
pub fn split(path: &str, text: &str) -> Module {
    let mut namespaces = Vec::new();
    let mut deps = Vec::new();
    let mut body = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let t = line.trim_end();
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
                let target = &t[i + 6..t.len() - 2];
                if !target.ends_with("dart_sdk.js") {
                    deps.push(resolve(path, target));
                }
            }
            continue;
        }
        body.push_str(line);
    }
    Module { path: path.to_string(), namespaces, deps, body }
}

/// Topological order of the modules: a dependency comes before whoever
/// imports it.
///
/// The emitter already merges `import` cycles into a single module
/// (`docs/EMISSAO-DDC.md`, "granularidade dos módulos"), so the graph is a
/// DAG. A residual cycle is no reason to abort — the input order is a
/// legitimate tie-breaker, and the DDC contract tolerates the late
/// declaration because classes only touch each other inside functions. The
/// cycle is returned for the caller to record.
pub fn order(modules: Vec<Module>) -> (Vec<Module>, Vec<String>) {
    let index: HashMap<String, usize> =
        modules.iter().enumerate().map(|(i, m)| (m.path.clone(), i)).collect();
    let mut state = vec![0u8; modules.len()]; // 0 = new, 1 = visiting, 2 = done
    let mut order = Vec::with_capacity(modules.len());
    let mut cycles = Vec::new();
    // Explicit stack: a real project has hundreds of modules and long chains.
    for root in 0..modules.len() {
        if state[root] != 0 {
            continue;
        }
        let mut stack = vec![(root, 0usize)];
        state[root] = 1;
        while let Some((n, k)) = stack.pop() {
            if k < modules[n].deps.len() {
                stack.push((n, k + 1));
                let Some(&d) = index.get(&modules[n].deps[k]) else { continue };
                match state[d] {
                    0 => {
                        state[d] = 1;
                        stack.push((d, 0));
                    }
                    1 => cycles.push(format!("{} → {}", modules[n].path, modules[d].path)),
                    _ => {}
                }
            } else {
                state[n] = 2;
                order.push(n);
            }
        }
    }
    let mut by_index: Vec<Option<Module>> = modules.into_iter().map(Some).collect();
    let out = order.into_iter().filter_map(|i| by_index[i].take()).collect();
    cycles.sort();
    cycles.dedup();
    (out, cycles)
}

/// Aliases that DDC exports under library names different from the local
/// names (`html$ as html`, `svg$ as svg`). Without the `export`, the user's
/// modules keep citing `html`/`svg`, so the bundle must recreate those
/// bindings before running any module.
pub fn exported_aliases(sdk: &str) -> Vec<(String, String)> {
    sdk.lines()
        .filter_map(|l| l.trim().strip_prefix("export {").and_then(|r| r.split_once('}').map(|(body, _)| body)))
        .flat_map(|body| body.split(','))
        .filter_map(|entry| {
            let (original, alias) = entry.trim().split_once(" as ")?;
            Some((original.trim().to_string(), alias.trim().to_string()))
        })
        .collect()
}

/// The text of `dart_sdk.js` without `export { … };`. The aliases imported
/// by modules are kept as top-level variables in the single file.
pub fn sdk_without_export(sdk: &str) -> String {
    let mut out = String::with_capacity(sdk.len());
    for line in sdk.split_inclusive('\n') {
        if line.starts_with("export {") {
            continue;
        }
        out.push_str(line);
    }
    for (original, alias) in exported_aliases(sdk) {
        out.push_str(&format!("var {alias} = {original};\n"));
    }
    out
}

/// Name of the entry library's namespace variable and whether `main` is
/// `async`, read from the `main.mjs` the emitter generated (`import { X as m } from './p.js'`).
pub fn entry_from_mjs(mjs: &str, modules: &[Module]) -> Option<String> {
    let line = mjs.lines().find(|l| l.contains(" as m } from './"))?;
    let ident = line.split("import { ").nth(1)?.split(" as m }").next()?.trim();
    let path = line.rsplit("from './").nth(0)?.trim_end_matches("';");
    // The `export { L$foo as foo }` disappeared in `split`; the matching
    // namespace is that module's `var L$foo`. Matched by identifier suffix.
    let m = modules.iter().find(|m| m.path == path)?;
    for ns in &m.namespaces {
        let var = ns.trim_start_matches("var ").split(' ').next()?;
        if var.trim_start_matches("L$") == ident || var == ident {
            return Some(var.to_string());
        }
    }
    m.namespaces.first().and_then(|ns| ns.trim_start_matches("var ").split(' ').next().map(str::to_string))
}

/// The verifier mode's `dart_podado` (`DARTFORGE_JSPROD_VERIFICAR=stub`): a
/// member the closed world said was dead was called. It writes the name to
/// stderr and exits with 97 — never a `throw`, which the program could catch
/// and go on with different behavior.
pub const STUB_PREAMBLE: &str = "function dart_podado(nome) {\n  const m = 'DARTFORGE-PODADO: ' + nome;\n  if (typeof process !== 'undefined') { process.stderr.write(m + '\\n'); process.exit(97); }\n  throw new Error(m);\n}\n";

/// Assembles the single file. `preamble` goes right after the header (empty
/// in normal mode).
pub fn assemble(sdk: &str, modules: &[Module], entry: &str, preamble: &str) -> String {
    let mut out = String::with_capacity(sdk.len() + modules.iter().map(|m| m.body.len() + 64).sum::<usize>());
    out.push_str("// dartforge — perfil de produção (docs/JS-PRODUCAO.md)\n");
    out.push_str(preamble);
    // DDC's `dart_sdk.js` is the browser one: `self` is the global object.
    out.push_str("if (typeof self === 'undefined') globalThis.self = globalThis;\n");
    out.push_str("if (typeof process !== 'undefined') process.on('uncaughtException', (e) => {\n  console.error('Unhandled exception:\\n' + e);\n  process.exit(255);\n});\n");
    out.push_str(sdk);
    if !sdk.ends_with('\n') {
        out.push('\n');
    }
    // `dart:html` declares the native result of querySelectorAll as
    // `List<Node>`. In the browser it is a NodeList, which lacks List's
    // symbolized methods (`every`, `firstWhere` etc.). DDC's bridge to
    // JSArray makes those methods available on the native prototype before
    // running user code; Node/servers have no NodeList.
    out.push_str("if (typeof NodeList !== 'undefined' && !NodeList.prototype[dartx.every]) dart.registerExtension('NodeList', _interceptors.JSArray);\n");
    // Namespaces of every library, at the top: this is what replaces the
    // `import`/`export` between modules.
    let mut seen = HashSet::new();
    for m in modules {
        for ns in &m.namespaces {
            if seen.insert(ns.clone()) {
                out.push_str(ns);
                out.push('\n');
            }
        }
    }
    for m in modules {
        out.push_str("(function () {\n");
        out.push_str(&m.body);
        if !m.body.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("})();\n");
    }
    out.push_str(entry);
    out.push_str(".main();\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_relative_paths() {
        assert_eq!(resolve("main.js", "./x.js"), "x.js");
        assert_eq!(resolve("packages/a/b.js", "../../dart_sdk.js"), "dart_sdk.js");
        assert_eq!(resolve("packages/a/b.js", "../c.js"), "packages/c.js");
    }

    #[test]
    fn renamed_export_becomes_alias_in_bundle() {
        let sdk = "var html$ = 1;\nvar svg$ = 2;\nexport { html$ as html, svg$ as svg };\n";
        let out = sdk_without_export(sdk);
        assert!(!out.contains("export {"));
        assert!(out.contains("var html = html$;\n"));
        assert!(out.contains("var svg = svg$;\n"));
    }

    #[test]
    fn node_list_gets_list_methods_before_user_code() {
        let m = split("main.js", "var L$main = Object.create(dart.library);\nL$main.main = function () {};\n");
        let js = assemble("var dart = {};\n", &[m], "L$main", "");
        let bridge = js.find("dart.registerExtension('NodeList', _interceptors.JSArray)").unwrap();
        let main = js.find("L$main.main = function").unwrap();
        assert!(bridge < main);
    }

    #[test]
    fn splits_preamble() {
        let text = "var L$x = Object.create(dart.library);\nexport { L$x as x };\nimport { core } from './dart_sdk.js';\nimport { y as L$y } from './sub/y.js';\nL$x.f = function () {};\n";
        let m = split("main.js", text);
        assert_eq!(m.namespaces, vec!["var L$x = Object.create(dart.library);"]);
        assert_eq!(m.deps, vec!["sub/y.js"]);
        assert_eq!(m.body, "L$x.f = function () {};\n");
    }

    #[test]
    fn orders_dependency_first() {
        let a = split("a.js", "var L$a = Object.create(dart.library);\nimport { b as L$b } from './b.js';\nL$a.f = 1;\n");
        let b = split("b.js", "var L$b = Object.create(dart.library);\nL$b.g = 2;\n");
        let (ord, cycles) = order(vec![a, b]);
        assert!(cycles.is_empty());
        assert_eq!(ord.iter().map(|m| m.path.as_str()).collect::<Vec<_>>(), vec!["b.js", "a.js"]);
    }

    #[test]
    fn cycle_does_not_abort() {
        let a = split("a.js", "var L$a = Object.create(dart.library);\nimport { b as L$b } from './b.js';\n");
        let b = split("b.js", "var L$b = Object.create(dart.library);\nimport { a as L$a } from './a.js';\n");
        let (ord, cycles) = order(vec![a, b]);
        assert_eq!(ord.len(), 2);
        assert_eq!(cycles.len(), 1);
    }
}
