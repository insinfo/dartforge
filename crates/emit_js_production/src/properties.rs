//! Renaming of property names (`docs/JS-PRODUCAO-TAMANHO.md` §3.1).
//!
//! The **renamable** set comes from the element model, never from the text:
//! the names the emitter gives to Dart members (instance, static and
//! top-level) and to classes, outside native and interop classes. Removed
//! from that set are the names JavaScript sees: the `external` ones, the
//! members of `@JS`/`@JSExport`/native classes, the words of the live `JS()`
//! *templates*, named parameters and named *record* fields (the key is built
//! at run time), the JS protocol and — already in the text — every string
//! literal shaped like an identifier (`dsend(o, "foo")`,
//! `js_util.getProperty`). A name that reaches run time as a *string* keeps
//! its source name: the program's output does not change.

use dartforge_elements::model::{ClassId, FunctionKind, Program};
use dartforge_intern::Interner;
use dartforge_types::table::Type;
use std::collections::HashSet;

/// Names of the JavaScript (and browser) protocol that the engine calls
/// implicitly on any object.
const JS_PROTOCOL: &[&str] = &[
    "toString", "valueOf", "toJSON", "then", "catch", "finally", "length", "name", "message", "stack", "constructor", "prototype", "call", "apply", "bind",
    "next", "done", "value", "return", "throw", "get", "set", "handleEvent", "caller", "callee", "arguments", "__proto__", "toLocaleString",
    "hasOwnProperty", "isPrototypeOf", "propertyIsEnumerable", "default", "enumerable", "configurable", "writable",
    // Members of built-in objects that the emitter, the *bootstrap* and the
    // *templates* call directly (`$constCache.get`, `Object.create`,
    // `a.push`): a Dart member with the same name cannot be renamed.
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

/// What the minifier may rename and what it must keep.
#[derive(Default)]
pub struct Names {
    pub renamable: HashSet<String>,
    pub reserved: HashSet<String>,
    /// Labels of the program's libraries (`dartforge_emit_js::recipe_tag`):
    /// the properties of private names end with them.
    pub tags: HashSet<String>,
}

fn has_interop_annotation(p: &Program, i: &Interner, c: ClassId) -> bool {
    let Some(d) = p.class(c).decl else { return false };
    p.unit(d.unit).ast.decl(d.decl).metadata.iter().any(|a| {
        a.name.last().is_some_and(|n| matches!(i.resolve(n.sym), "JS" | "JSExport" | "anonymous" | "staticInterop" | "Native" | "JsPeerInterface"))
    })
}

/// Computes the sets from the model. `js_names` are the words of the live
/// `JS()` *templates* (`dartforge_world::World::js_names`).
pub fn compute(a: &dartforge_emit_js::Analysis<'_>, js_names: &HashSet<String>, named_args: Option<&std::collections::HashMap<String, String>>) -> Names {
    use dartforge_elements::model::ClassKind;
    use dartforge_emit_js::body::{js_member_name, static_member_name, top_level_name};
    let (p, i, table) = (a.program, a.interner, a.table);
    let ctx = a.ctx();
    let mut n = Names::default();
    for (k, l) in p.libraries.iter().enumerate() {
        if !l.is_sdk {
            n.tags.insert(dartforge_emit_js::recipe_tag(dartforge_elements::model::LibraryId(k as u32)));
        }
    }
    // Interop: `@JS` classes, extension types (those of `package:web` are the
    // DOM) and the ones annotated as native or exported.
    let interop: Vec<bool> = (0..p.classes.len())
        .map(|k| {
            let c = ClassId(k as u32);
            has_interop_annotation(p, i, c) || ctx.is_js_class(c) || p.class(c).kind == ClassKind::ExtensionType
        })
        .collect();
    // The JS names given by `@JS(a.b)` (class, library, member).
    let parts = |s: &str, r: &mut HashSet<String>| {
        for x in s.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '$')) {
            if !x.is_empty() {
                r.insert(x.to_string());
            }
        }
    };
    for jc in ctx.js_classes.values() {
        if let Some(x) = &jc.name {
            parts(x, &mut n.reserved);
        }
    }
    for x in ctx.js_libs.values().chain(ctx.fn_js.values()).chain(ctx.var_js.values()).flatten() {
        parts(x, &mut n.reserved);
    }
    let from_interop = |c: Option<ClassId>| c.is_some_and(|c| interop[c.0 as usize]);
    for (k, c) in p.classes.iter().enumerate() {
        let name = i.resolve(c.name).to_string();
        if interop[k] {
            n.reserved.insert(name);
        } else {
            n.renamable.insert(name);
        }
    }
    for f in p.functions.iter() {
        let name = i.resolve(f.name);
        if matches!(f.kind, FunctionKind::Constructor | FunctionKind::SyntheticConstructor) {
            // `C.new`, `C.name`: a class property that only the text cites
            // (`new C.new(…)`, `C.name.call(this, …)`).
            if f.class.is_some_and(|c| !interop[c.0 as usize]) {
                n.renamable.insert(if name.is_empty() { "new".to_string() } else { static_member_name(name) });
            }
            continue;
        }
        if name.is_empty() {
            continue;
        }
        let js = if f.class.is_some() && !f.static_ {
            js_member_name(name)
        } else if f.class.is_some() {
            static_member_name(name)
        } else if f.extension.is_none() {
            top_level_name(name)
        } else {
            continue;
        };
        // An SDK `external` with a *patch* (`unsafeCast`, `identical`) is
        // ordinary Dart: the body comes from the patch, nothing in JS calls
        // it by name.
        let external_js = f.external && !(p.library(f.library).is_sdk && f.patched_by.is_some());
        if external_js || from_interop(f.class) {
            n.reserved.insert(js);
            n.reserved.insert(name.to_string());
        } else {
            n.renamable.insert(js);
        }
    }
    for v in p.variables.iter() {
        let name = i.resolve(v.name);
        if name.is_empty() || v.extension.is_some() {
            continue;
        }
        let js = if v.class.is_some() && !v.static_ {
            js_member_name(name)
        } else if v.class.is_some() {
            static_member_name(name)
        } else {
            top_level_name(name)
        };
        if v.external || from_interop(v.class) {
            n.reserved.insert(js);
            n.reserved.insert(name.to_string());
        } else {
            n.renamable.insert(js);
        }
    }
    // Named parameters and named record fields: the key of the arguments
    // object and the record's *getter* are built by name at run time.
    for t in 0..table.len() {
        match table.get(dartforge_types::table::TypeId(t as u32)) {
            Type::Function { named, .. } => {
                for (s, _, _) in named.iter() {
                    // With the named-argument map (`named_args.rs`) the key is
                    // the short one, and the source name stays free for the
                    // members of the same name.
                    let name = i.resolve(*s);
                    match named_args.and_then(|m| m.get(name)) {
                        Some(short) => n.reserved.insert(short.clone()),
                        None => n.reserved.insert(name.to_string()),
                    };
                }
            }
            Type::Record { named, .. } => {
                for (s, _) in named.iter() {
                    n.reserved.insert(i.resolve(*s).to_string());
                }
            }
            _ => {}
        }
    }
    // The helpers that the *bootstrap* and the emitter hang on `dart` and on
    // each library: names of the generated JS, called only by the text
    // itself.
    for x in ["privateName", "lateFields", "$constCache", "$C"] {
        n.renamable.insert(x.to_string());
    }
    // `new` in the *templates* (`#.new = function`,
    // `Object.getPrototypeOf(#).new`) is the Dart constructor, never a
    // JavaScript property: renamed along with it.
    n.reserved.extend(js_names.iter().filter(|w| *w != "new").cloned());
    n.reserved.extend(JS_PROTOCOL.iter().map(|s| s.to_string()));
    n
}
