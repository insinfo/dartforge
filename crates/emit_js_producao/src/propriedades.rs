//! Renomeio de nomes de propriedade (`docs/JS-PRODUCAO-TAMANHO.md` §3.1).
//!
//! O conjunto **renomeável** sai do modelo de elementos, nunca do texto: os
//! nomes que o emissor dá a membros Dart (de instância, estáticos e de topo)
//! e a classes, fora das classes nativas e de interop. Desse conjunto saem os
//! nomes que o JavaScript enxerga: os `external`, os membros de classes
//! `@JS`/`@JSExport`/nativas, as palavras dos *templates* `JS()` vivos, os
//! parâmetros nomeados e os campos nomeados de *records* (a chave é montada
//! em execução), o protocolo do JS e — já no texto — toda *string* literal
//! com forma de identificador (`dsend(o, "foo")`, `js_util.getProperty`).
//! Um nome que chega à execução como *string* fica o de origem: a saída do
//! programa não muda.

use dartforge_elements::model::{ClassId, FunctionKind, Program};
use dartforge_intern::Interner;
use dartforge_types::table::Type;
use std::collections::HashSet;

/// Nomes do protocolo do JavaScript (e do navegador) que o motor chama
/// implicitamente num objeto qualquer.
const PROTOCOLO_JS: &[&str] = &[
    "toString", "valueOf", "toJSON", "then", "catch", "finally", "length", "name", "message", "stack", "constructor", "prototype", "call", "apply", "bind",
    "next", "done", "value", "return", "throw", "get", "set", "handleEvent", "caller", "callee", "arguments", "__proto__", "toLocaleString",
    "hasOwnProperty", "isPrototypeOf", "propertyIsEnumerable", "default", "enumerable", "configurable", "writable",
    // Membros dos objetos embutidos que o emissor, o *bootstrap* e os
    // *templates* chamam direto (`$constCache.get`, `Object.create`,
    // `a.push`): um membro Dart de mesmo nome não pode ser renomeado.
    "create", "defineProperty", "defineProperties", "getOwnPropertyDescriptor", "getOwnPropertyNames", "getOwnPropertySymbols", "getPrototypeOf",
    "setPrototypeOf", "keys", "values", "entries", "assign", "freeze", "isFrozen", "seal", "is", "fromEntries", "has", "delete", "clear", "add", "forEach",
    "size", "push", "pop", "shift", "unshift", "slice", "splice", "concat", "join", "reverse", "sort", "indexOf", "lastIndexOf", "includes", "find",
    "findIndex", "filter", "map", "reduce", "reduceRight", "some", "every", "fill", "copyWithin", "flat", "flatMap", "from", "of", "isArray", "at",
    "charCodeAt", "codePointAt", "charAt", "fromCharCode", "fromCodePoint", "substring", "substr", "toUpperCase", "toLowerCase", "trim", "trimStart",
    "trimEnd", "split", "replace", "replaceAll", "match", "matchAll", "search", "startsWith", "endsWith", "padStart", "padEnd", "repeat",
    "localeCompare", "normalize", "raw", "exec", "test", "source", "flags", "global", "multiline", "ignoreCase", "unicode", "sticky", "lastIndex",
    "index", "input", "groups", "iterator", "asyncIterator", "hasInstance", "toPrimitive", "toStringTag", "for", "keyFor", "description", "resolve",
    "reject", "all", "allSettled", "race", "any", "parse", "stringify", "floor", "ceil", "round", "trunc", "abs", "max", "min", "pow", "sqrt", "random",
    "sign", "log", "exp", "isNaN", "isFinite", "isInteger", "isSafeInteger", "toFixed", "toPrecision", "toExponential", "parseInt", "parseFloat",
    "buffer", "byteLength", "byteOffset", "subarray", "BYTES_PER_ELEMENT", "now", "getTime", "ownKeys", "construct", "deleteProperty", "error",
    "warn", "info", "debug", "cause", "lineNumber", "columnNumber", "fileName", "stackTraceLimit", "captureStackTrace",
];

/// O que o minificador pode renomear e o que ele deve manter.
#[derive(Default)]
pub struct Nomes {
    pub renomeaveis: HashSet<String>,
    pub reservados: HashSet<String>,
}

fn anotacoes_de_interop(p: &Program, i: &Interner, c: ClassId) -> bool {
    let Some(d) = p.class(c).decl else { return false };
    p.unit(d.unit).ast.decl(d.decl).metadata.iter().any(|a| {
        a.name.last().is_some_and(|n| matches!(i.resolve(n.sym), "JS" | "JSExport" | "anonymous" | "staticInterop" | "Native" | "JsPeerInterface"))
    })
}

/// Calcula os conjuntos a partir do modelo. `nomes_js` são as palavras dos
/// *templates* `JS()` vivos (`dartforge_mundo::Mundo::nomes_js`).
pub fn calcular(a: &dartforge_emit_js::Analise<'_>, nomes_js: &HashSet<String>) -> Nomes {
    use dartforge_elements::model::ClassKind;
    use dartforge_emit_js::body::{js_member_name, static_member_name, top_level_name};
    let (p, i, table) = (a.program, a.interner, a.table);
    let ctx = a.ctx();
    let mut n = Nomes::default();
    // Interop: classes `@JS`, tipos de extensão (os do `package:web` são o
    // DOM) e as anotadas como nativas ou exportadas.
    let interop: Vec<bool> = (0..p.classes.len())
        .map(|k| {
            let c = ClassId(k as u32);
            anotacoes_de_interop(p, i, c) || ctx.is_js_class(c) || p.class(c).kind == ClassKind::ExtensionType
        })
        .collect();
    // Os nomes JS dados por `@JS(a.b)` (classe, biblioteca, membro).
    let partes = |s: &str, r: &mut HashSet<String>| {
        for x in s.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '$')) {
            if !x.is_empty() {
                r.insert(x.to_string());
            }
        }
    };
    for jc in ctx.js_classes.values() {
        if let Some(x) = &jc.name {
            partes(x, &mut n.reservados);
        }
    }
    for x in ctx.js_libs.values().chain(ctx.fn_js.values()).chain(ctx.var_js.values()).flatten() {
        partes(x, &mut n.reservados);
    }
    let de_interop = |c: Option<ClassId>| c.is_some_and(|c| interop[c.0 as usize]);
    for (k, c) in p.classes.iter().enumerate() {
        let nome = i.resolve(c.name).to_string();
        if interop[k] {
            n.reservados.insert(nome);
        } else {
            n.renomeaveis.insert(nome);
        }
    }
    for f in p.functions.iter() {
        let nome = i.resolve(f.name);
        if nome.is_empty() || matches!(f.kind, FunctionKind::Constructor | FunctionKind::SyntheticConstructor) {
            continue;
        }
        let js = if f.class.is_some() && !f.static_ {
            js_member_name(nome)
        } else if f.class.is_some() {
            static_member_name(nome)
        } else if f.extension.is_none() {
            top_level_name(nome)
        } else {
            continue;
        };
        if f.external || de_interop(f.class) {
            n.reservados.insert(js);
            n.reservados.insert(nome.to_string());
        } else {
            n.renomeaveis.insert(js);
        }
    }
    for v in p.variables.iter() {
        let nome = i.resolve(v.name);
        if nome.is_empty() || v.extension.is_some() {
            continue;
        }
        let js = if v.class.is_some() && !v.static_ {
            js_member_name(nome)
        } else if v.class.is_some() {
            static_member_name(nome)
        } else {
            top_level_name(nome)
        };
        if v.external || de_interop(v.class) {
            n.reservados.insert(js);
            n.reservados.insert(nome.to_string());
        } else {
            n.renomeaveis.insert(js);
        }
    }
    // Parâmetros nomeados e campos nomeados de records: a chave do objeto de
    // argumentos e o *getter* do record são montados por nome em execução.
    for t in 0..table.len() {
        match table.get(dartforge_types::table::TypeId(t as u32)) {
            Type::Function { named, .. } => {
                for (s, _, _) in named.iter() {
                    n.reservados.insert(i.resolve(*s).to_string());
                }
            }
            Type::Record { named, .. } => {
                for (s, _) in named.iter() {
                    n.reservados.insert(i.resolve(*s).to_string());
                }
            }
            _ => {}
        }
    }
    // Os auxiliares que o *bootstrap* e o emissor penduram em `dart` e em
    // cada biblioteca: nomes do JS gerado, chamados só pelo próprio texto.
    for x in ["privateName", "lateField", "$constCache", "$C"] {
        n.renomeaveis.insert(x.to_string());
    }
    n.reservados.extend(nomes_js.iter().cloned());
    n.reservados.extend(PROTOCOLO_JS.iter().map(|s| s.to_string()));
    n
}
