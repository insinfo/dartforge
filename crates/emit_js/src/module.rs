//! Emissão de módulos: um módulo ES6 por biblioteca, classes, funções e
//! variáveis de topo, regras rti e o `main.mjs`.

use crate::body::{js_member_name, static_member_name, AsyncKind, FnEmitter};
use crate::ctx::Ctx;
use crate::js::{self, Js, Writer, P_ASSIGN};
use crate::ty::Ty;
use crate::Emitido;
use dartforge_diagnostics::Diagnostic;
use dartforge_elements::model::{ClassId, ClassKind, Element, FunctionElementId, FunctionKind, FunctionRef, LibraryId, UnitId, VariableId, VariableRef};
use dartforge_frontend::ast::{self, DeclKind, FunctionBody, MemberKind};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

/// Estado por módulo: usos de bibliotecas, símbolos `dartx` e privados.
pub struct ModState {
    pub lib: LibraryId,
    /// Bibliotecas emitidas neste módulo (um componente fortemente conexo de imports).
    pub group: Vec<LibraryId>,
    pub sdk_used: RefCell<BTreeSet<String>>,
    pub user_imports: RefCell<BTreeSet<u32>>,
    pub dartx_used: RefCell<BTreeMap<String, String>>,
    pub private_syms: RefCell<BTreeMap<String, (u32, String)>>,
    pub noted_classes: RefCell<BTreeSet<u32>>,
}

impl ModState {
    pub fn new(lib: LibraryId) -> Self {
        ModState {
            lib,
            group: vec![lib],
            sdk_used: RefCell::new(BTreeSet::new()),
            user_imports: RefCell::new(BTreeSet::new()),
            dartx_used: RefCell::new(BTreeMap::new()),
            private_syms: RefCell::new(BTreeMap::new()),
            noted_classes: RefCell::new(BTreeSet::new()),
        }
    }
    pub fn use_sdk(&self, name: &str) {
        self.sdk_used.borrow_mut().insert(name.to_string());
    }
    pub fn lib_var(&self, ctx: &Ctx, lib: LibraryId) -> String {
        let info = &ctx.libs[lib.0 as usize];
        if info.is_sdk {
            self.use_sdk(&info.js_var);
        } else if !self.group.contains(&lib) {
            self.user_imports.borrow_mut().insert(lib.0);
        }
        info.js_var.clone()
    }
    pub fn dartx(&self, name: &str) -> String {
        let var = dartx_var(name);
        self.dartx_used.borrow_mut().insert(var.clone(), name.to_string());
        var
    }
    pub fn private_sym(&self, ctx: &Ctx, lib: LibraryId, name: &str) -> String {
        let _ = self.lib_var(ctx, lib);
        let ident = ctx.lib_ident(lib).to_string();
        let var = format!("$P_{ident}_{}", js_safe(name));
        self.private_syms.borrow_mut().insert(var.clone(), (lib.0, name.to_string()));
        var
    }
    pub fn note_class(&self, c: ClassId) {
        self.noted_classes.borrow_mut().insert(c.0);
    }
}

fn js_safe(name: &str) -> String {
    name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '$' }).collect()
}

/// Nome da variável que guarda um símbolo `dartx`.
pub fn dartx_var(name: &str) -> String {
    let op = match name {
        "+" => "plus",
        "-" => "minus",
        "*" => "times",
        "/" => "divide",
        "~/" => "floorDivide",
        "%" => "modulo",
        "<<" => "leftShift",
        ">>" => "rightShift",
        ">>>" => "tripleShift",
        "&" => "bitAnd",
        "|" => "bitOr",
        "^" => "bitXor",
        "~" => "bitNot",
        "<" => "lessThan",
        ">" => "greaterThan",
        "<=" => "lessOrEquals",
        ">=" => "greaterOrEquals",
        _ => return format!("${name}"),
    };
    format!("${op}")
}

/// Emite todos os módulos do programa.
pub fn emitir(ctx: &Ctx) -> Result<Emitido, Vec<Diagnostic>> {
    let mut modulos = Vec::new();
    let mut entry_ident = String::from("main");
    let mut entry_path = String::from("main.js");
    let mut main_async = false;
    let mut done: HashSet<u32> = HashSet::new();
    for (i, lib) in ctx.program.libraries.iter().enumerate() {
        let lid = LibraryId(i as u32);
        let info = &ctx.libs[i];
        if info.is_sdk || lib.units.is_empty() || done.contains(&lid.0) {
            continue;
        }
        let group: Vec<LibraryId> = ctx.groups.get(&lid.0).cloned().unwrap_or_else(|| vec![lid]);
        for g in &group {
            done.insert(g.0);
        }
        let text = emit_group(ctx, &group);
        modulos.push((info.module_path.clone(), text));
        for &g in &group {
            if Some(g) == ctx.program.entry {
                entry_ident = ctx.libs[g.0 as usize].ident.clone();
                entry_path = info.module_path.clone();
                main_async = lib_main_is_async(ctx, g);
            }
        }
    }
    let _ = main_async;
    // O `dart_sdk.js` do DDC é o do navegador: `self` é o global (Node só tem `globalThis`).
    modulos.push(("preambulo.js".to_string(), "if (typeof self === 'undefined') globalThis.self = globalThis;\n".to_string()));
    let entrada = format!(
        "if (typeof process !== 'undefined') process.on('uncaughtException', (e) => {{\n  console.error('Unhandled exception:\\n' + e);\n  process.exit(255);\n}});\nimport './preambulo.js';\nimport {{ dart }} from './dart_sdk.js';\nimport {{ {entry_ident} as m }} from './{entry_path}';\nm.main();\n"
    );
    Ok(Emitido { modulos, entrada })
}

fn lib_main_is_async(ctx: &Ctx, lib: LibraryId) -> bool {
    let Some(sym) = ctx.sym("main") else { return false };
    let Some(b) = ctx.program.library(lib).declared.get(&sym) else { return false };
    let Some(Element::Function(fid)) = b.getter else { return false };
    let f = ctx.program.function(fid);
    if let FunctionRef::Function { unit, function } = f.node {
        let af = ctx.program.unit(unit).ast.function(function);
        return af.modifier == ast::AsyncModifier::Async;
    }
    false
}

/// Ordena classes: superclasses e mixins antes das subclasses.
fn order_classes(ctx: &Ctx, classes: &[ClassId]) -> Vec<ClassId> {
    let set: HashSet<ClassId> = classes.iter().copied().collect();
    let mut out = Vec::new();
    let mut done: HashSet<ClassId> = HashSet::new();
    fn visit(ctx: &Ctx, c: ClassId, set: &HashSet<ClassId>, done: &mut HashSet<ClassId>, out: &mut Vec<ClassId>) {
        if done.contains(&c) {
            return;
        }
        done.insert(c);
        let class = ctx.program.class(c);
        if let Some(s) = class.supertype_class {
            if set.contains(&s) {
                visit(ctx, s, set, done, out);
            }
        }
        for &m in &ctx.mixins_of(c) {
            if set.contains(&m) {
                visit(ctx, m, set, done, out);
            }
        }
        out.push(c);
    }
    for &c in classes {
        visit(ctx, c, &set, &mut done, &mut out);
    }
    out
}

fn emit_group(ctx: &Ctx, group: &[LibraryId]) -> String {
    let lib = group[0];
    let mut m = ModState::new(lib);
    m.group = group.to_vec();
    let info = &ctx.libs[lib.0 as usize];
    let mut body = Writer::default();

    // Classes de todas as bibliotecas do grupo, ordenadas por hierarquia.
    let in_group = |l: LibraryId| group.contains(&l);
    let mut classes: Vec<ClassId> = Vec::new();
    for (i, c) in ctx.program.classes.iter().enumerate() {
        if in_group(c.library) && c.decl.is_some() {
            classes.push(ClassId(i as u32));
        }
    }
    let ordered = order_classes(ctx, &classes);
    for c in ordered {
        m.note_class(c);
        emit_class(ctx, &m, c, &mut body);
    }
    for &glib in group {
        emit_library_rest(ctx, &m, glib, &mut body);
    }

    // Regras rti das classes do módulo (e referenciadas).
    let rules = emit_rules(ctx, &m);

    // Prelúdio.
    let mut out = String::new();
    let mut exports: Vec<String> = Vec::new();
    for &glib in group {
        let gi = &ctx.libs[glib.0 as usize];
        out.push_str(&format!("var {} = Object.create(dart.library);\n", gi.js_var));
        exports.push(format!("{} as {}", gi.js_var, gi.ident));
    }
    out.push_str(&format!("export {{ {} }};\n", exports.join(", ")));
    let up = "../".repeat(info.module_path.matches('/').count());
    let mut sdk: BTreeSet<String> = m.sdk_used.borrow().clone();
    for s in ["dart", "dart_rti", "core", "dartx"] {
        sdk.insert(s.to_string());
    }
    sdk.insert("async".to_string());
    let sdk_list: Vec<String> = sdk.iter().cloned().collect();
    out.push_str(&format!("import {{ {} }} from './{up}dart_sdk.js';\n", sdk_list.join(", ")));
    for &ul in m.user_imports.borrow().iter() {
        let uinfo = &ctx.libs[ul as usize];
        out.push_str(&format!("import {{ {} as {} }} from './{up}{}';\n", uinfo.ident, uinfo.js_var, uinfo.module_path));
    }
    for (var, name) in m.dartx_used.borrow().iter() {
        if js::is_js_ident(name) {
            out.push_str(&format!("var {var} = dartx.{name};\n"));
        } else {
            out.push_str(&format!("var {var} = dartx[{}];\n", js::string_literal(name)));
        }
    }
    out.push_str("var _is = dart.privateName(dart_rti, \"_is\");\nvar _as = dart.privateName(dart_rti, \"_as\");\nvar _eval = dart.privateName(dart_rti, \"_eval\");\nvar _bind = dart.privateName(dart_rti, \"_bind\");\n");
    out.push_str("var _current = dart.privateName(async, \"_current\");\nvar _datum = dart.privateName(async, \"_datum\");\nvar _yieldStar = dart.privateName(async, \"_yieldStar\");\n");
    for (var, (l, name)) in m.private_syms.borrow().iter() {
        let lv = &ctx.libs[*l as usize].js_var;
        out.push_str(&format!("var {var} = dart.privateName({lv}, {});\n", js::string_literal(name)));
    }
    out.push_str("dart._checkModuleNullSafetyMode(true);\n");
    for &glib in group {
        let lvar = &ctx.libs[glib.0 as usize].js_var;
        out.push_str(&format!("{lvar}.$constCache = new Map();\n{lvar}.$C = function(k, f) {{ let v = {lvar}.$constCache.get(k); if (v === void 0) {{ v = f(); {lvar}.$constCache.set(k, v); }} return v; }};\n"));
    }
    out.push_str(&body.out);
    out.push_str(&rules);
    let tracked: Vec<String> = group
        .iter()
        .map(|&g| format!("  {}: {}", js::string_literal(&ctx.program.library(g).uri), ctx.libs[g.0 as usize].js_var))
        .collect();
    out.push_str(&format!(
        "dart.trackLibraries({}, {{\n{}\n}}, {{\n}}, null);\n",
        js::string_literal(&info.ident),
        tracked.join(",\n")
    ));
    out
}

/// Funções, variáveis de topo e estáticos de extensão de uma biblioteca.
fn emit_library_rest(ctx: &Ctx, m: &ModState, lib: LibraryId, body: &mut Writer) {
    let lvar = ctx.libs[lib.0 as usize].js_var.clone();
    let mut functions: Vec<FunctionElementId> = Vec::new();
    let mut variables: Vec<VariableId> = Vec::new();
    for (i, f) in ctx.program.functions.iter().enumerate() {
        if f.library == lib && f.class.is_none() && f.kind != FunctionKind::ImplicitAccessor {
            functions.push(FunctionElementId(i as u32));
        }
    }
    let mut ext_variables: Vec<VariableId> = Vec::new();
    for (i, v) in ctx.program.variables.iter().enumerate() {
        if v.library == lib && v.class.is_none() {
            if v.extension.is_none() {
                variables.push(VariableId(i as u32));
            } else {
                ext_variables.push(VariableId(i as u32));
            }
        }
    }
    // Funções de topo e de extensão; acessores de topo agrupados por nome.
    let mut accessors: Vec<(String, String)> = Vec::new();
    for fid in functions {
        emit_top_function(ctx, m, fid, body, &mut accessors);
    }
    if !accessors.is_empty() {
        body.line(&format!("dart.copyProperties({lvar}, {{"));
        let texts: Vec<String> = accessors.iter().map(|(_, t)| indent(t)).collect();
        body.push_raw(&texts.join(",\n"));
        body.push_raw("\n});\n");
    }
    // Variáveis de topo.
    emit_top_variables(ctx, m, lib, &variables, body);
    // Estáticos de extensão: `L['Ext|nome']`.
    if !ext_variables.is_empty() {
        let mut lazy: Vec<String> = Vec::new();
        for vid in ext_variables {
            let v = ctx.program.variable(vid);
            let Some(ext) = v.extension else { continue };
            let e = ctx.program.extension(ext);
            let tmp = FnEmitter::new(ctx, m, e.decl.unit, None, true);
            let ext_name = tmp.extension_js_name(ext);
            let name = ctx.name(v.name);
            let key = format!("{ext_name}|{name}");
            let VariableRef::Field { unit: fu, member, index } = v.node else { continue };
            let mem = ctx.program.unit(fu).ast.member(member);
            let MemberKind::Field(list) = &mem.kind else { continue };
            let var = &list.variables[index];
            let ty = ctx.var_ty(vid);
            let mut e2 = FnEmitter::new(ctx, m, fu, None, true);
            e2.in_const = v.const_;
            let init = match var.initializer {
                Some(i) => {
                    let (js, _) = e2.emit_expr(i, Some(&ty));
                    let pre = if e2.temps.is_empty() { String::new() } else { format!("let {};\n", e2.temps.join(", ")) };
                    format!("{pre}{}return {};", e2.w.out, js.code)
                }
                None => "return null;".to_string(),
            };
            let mut entry = format!("get [{}]() {{\n{}\n}}", js::string_literal(&key), indent(&init));
            if !v.final_ && !v.const_ {
                entry.push_str(&format!(",\nset [{}](value) {{}}", js::string_literal(&key)));
            }
            lazy.push(entry);
        }
        if !lazy.is_empty() {
            body.line(&format!("dart.defineLazy({lvar}, {{"));
            body.push_raw(&indent(&lazy.join(",\n")));
            body.push_raw("\n});\n");
        }
    }
}

// ---------------------------------------------------------------------------
// Regras rti
// ---------------------------------------------------------------------------

fn emit_rules(ctx: &Ctx, m: &ModState) -> String {
    let mut entries: Vec<String> = Vec::new();
    let noted: Vec<u32> = m.noted_classes.borrow().iter().copied().collect();
    let mut seen: HashSet<u32> = HashSet::new();
    let mut queue: Vec<ClassId> = noted.into_iter().map(ClassId).collect();
    while let Some(c) = queue.pop() {
        if !seen.insert(c.0) {
            continue;
        }
        let lib = ctx.lib_of_class(c);
        if ctx.libs[lib.0 as usize].is_sdk {
            continue;
        }
        if ctx.program.class(c).decl.is_none() {
            continue;
        }
        let class = ctx.program.class(c);
        let params = &ctx.class_params[c.0 as usize];
        let mut items: Vec<String> = Vec::new();
        for (i, p) in params.iter().enumerate() {
            items.push(format!("\"{}.{}\":\"{}\"", ctx.class_name(c), p.name, i + 1));
        }
        // Supertipos transitivos.
        let this_ty = ctx.this_ty(c);
        let mut sseen: HashSet<u32> = HashSet::new();
        let mut squeue: Vec<Ty> = ctx.direct_supers(&this_ty);
        let mut supers: Vec<Ty> = Vec::new();
        while let Some(s) = squeue.pop() {
            let Some(sc) = s.class() else { continue };
            if Some(sc) == ctx.object || !sseen.insert(sc.0) {
                continue;
            }
            supers.push(s.clone());
            squeue.extend(ctx.direct_supers(&s));
        }
        let _ = class;
        for s in supers {
            let Ty::Iface { class: sc, args, .. } = &s else { continue };
            let sparams = &ctx.class_params[sc.0 as usize];
            for (p, a) in sparams.iter().zip(args.iter()) {
                items.push(format!("\"{}.{}\":{}", ctx.class_name(*sc), p.name, json_str(&rule_recipe(ctx, a, c))));
            }
            let arg_recipes: Vec<String> = args.iter().map(|a| json_str(&rule_recipe(ctx, a, c))).collect();
            items.push(format!("{}:[{}]", json_str(&ctx.class_recipe(*sc)), arg_recipes.join(",")));
            if !ctx.libs[ctx.lib_of_class(*sc).0 as usize].is_sdk {
                queue.push(*sc);
            }
        }
        if !items.is_empty() {
            entries.push(format!("{}:{{{}}}", json_str(&ctx.class_recipe(c)), items.join(",")));
        }
    }
    if entries.is_empty() {
        return String::new();
    }
    let json = format!("{{{}}}", entries.join(","));
    format!("dart_rti._Universe.addRules(dart.typeUniverse, JSON.parse({}));\n", js::string_literal(&json))
}

fn json_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Receita em ambiente de classe (parâmetros próprios por índice 1..n).
fn rule_recipe(ctx: &Ctx, t: &Ty, class: ClassId) -> String {
    match t {
        Ty::Dynamic => "@".into(),
        Ty::Void => "~".into(),
        Ty::Never => "0&".into(),
        Ty::Null => ctx.null_.map(|n| ctx.class_recipe(n)).unwrap_or("core|Null".into()),
        Ty::Iface { class: c, args, nullable } => {
            let mut s = ctx.class_recipe(*c);
            if !args.is_empty() {
                s.push('<');
                let parts: Vec<String> = args.iter().map(|a| rule_recipe(ctx, a, class)).collect();
                s.push_str(&parts.join(","));
                s.push('>');
            }
            if *nullable {
                s.push('?');
            }
            s
        }
        Ty::Param { id, nullable, .. } => {
            let params = &ctx.class_params[class.0 as usize];
            match params.iter().position(|p| p.id == *id) {
                Some(i) => format!("{}{}", i + 1, if *nullable { "?" } else { "" }),
                None => "@".into(),
            }
        }
        Ty::FutureOr { arg, nullable } => format!("{}/{}", rule_recipe(ctx, arg, class), if *nullable { "?" } else { "" }),
        Ty::Fn { ret, pos, opt, named, type_params, nullable } => {
            let mut s = rule_recipe(ctx, ret, class);
            s.push('(');
            let mut parts: Vec<String> = pos.iter().map(|a| rule_recipe(ctx, a, class)).collect();
            if !opt.is_empty() {
                parts.push(format!("[{}]", opt.iter().map(|a| rule_recipe(ctx, a, class)).collect::<Vec<_>>().join(",")));
            }
            if !named.is_empty() {
                parts.push(format!(
                    "{{{}}}",
                    named.iter().map(|(n, t, r)| format!("{n}{}{}", if *r { "!" } else { ":" }, rule_recipe(ctx, t, class))).collect::<Vec<_>>().join(",")
                ));
            }
            s.push_str(&parts.join(","));
            s.push(')');
            if !type_params.is_empty() {
                s.push('<');
                s.push_str(&type_params.iter().map(|p| rule_recipe(ctx, &p.bound, class)).collect::<Vec<_>>().join(","));
                s.push('>');
            }
            if *nullable {
                s.push('?');
            }
            s
        }
        Ty::Record { pos, named, nullable } => {
            let mut s = String::from("+");
            s.push_str(&named.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>().join(","));
            s.push('(');
            let mut parts: Vec<String> = pos.iter().map(|a| rule_recipe(ctx, a, class)).collect();
            parts.extend(named.iter().map(|(_, t)| rule_recipe(ctx, t, class)));
            s.push_str(&parts.join(","));
            s.push(')');
            if *nullable {
                s.push('?');
            }
            s
        }
    }
}

// ---------------------------------------------------------------------------
// Funções e variáveis de topo
// ---------------------------------------------------------------------------

/// Emite `function nome(params) { corpo }` (texto completo) para um elemento função.
fn function_text(ctx: &Ctx, m: &ModState, fid: FunctionElementId, head_name: Option<&str>, class: Option<ClassId>, is_static: bool, extension_this: Option<&Ty>) -> (String, AsyncKind) {
    let f = ctx.program.function(fid);
    let FunctionRef::Function { unit, function } = f.node else {
        return (String::new(), AsyncKind::None);
    };
    let af = ctx.program.unit(unit).ast.function(function);
    let mut e = FnEmitter::new(ctx, m, unit, class, is_static);
    let data = &ctx.outline.functions[fid.0 as usize];
    // Parâmetros de tipo da função.
    let mut tp_js: Vec<String> = Vec::new();
    for &pid in data.type_params.iter() {
        let p = ctx.ty_param_of(pid);
        let jsn = js::ident(&p.name);
        e.fn_type_params.push((p.id, jsn.clone()));
        tp_js.push(jsn);
    }
    let kind = match af.modifier {
        ast::AsyncModifier::None => AsyncKind::None,
        ast::AsyncModifier::Async => AsyncKind::Async,
        ast::AsyncModifier::AsyncStar => AsyncKind::AsyncStar,
        ast::AsyncModifier::SyncStar => AsyncKind::SyncStar,
    };
    let ret_ty = ctx.ty_of(data.return_type);
    let sig_ty = ctx.fn_ty(fid);
    e.async_kind = kind;
    e.ret_ty = ret_ty.clone();
    let mut extra_prologue = String::new();
    let mut params_js: Vec<String> = tp_js;
    if let Some(t) = extension_this {
        // `$this` é o receptor da extensão.
        params_js.push("$this".into());
        if let Some(sym) = ctx.sym("this") {
            e.declare_js(sym, "$this".into(), t.clone());
        }
        e.extension_this = Some(t.clone());
    }
    let ps: &[ast::Parameter] = af.parameters.as_deref().unwrap_or(&[]);
    let (pjs, prologue) = e.declare_params(ps, Some(&sig_ty));
    if !pjs.is_empty() {
        params_js.push(pjs);
    }
    extra_prologue.push_str(&prologue);
    e.emit_body(&af.body);
    let body = finish_body(&mut e);
    let head = match head_name {
        Some(n) => format!("{n}({}) {{", params_js.join(", ")),
        None => format!("function({}) {{", params_js.join(", ")),
    };
    (e.wrap_async_head(kind, &head, &extra_prologue, &body, &ret_ty), kind)
}

/// Fecha o corpo: declara temps.
fn finish_body(e: &mut FnEmitter) -> String {
    let mut body = String::new();
    if !e.temps.is_empty() {
        body.push_str(&format!("let {};\n", e.temps.join(", ")));
    }
    body.push_str(&e.w.out);
    body
}

fn emit_top_function(ctx: &Ctx, m: &ModState, fid: FunctionElementId, w: &mut Writer, accessors: &mut Vec<(String, String)>) {
    let f = ctx.program.function(fid);
    let lvar = &ctx.libs[f.library.0 as usize].js_var;
    let name = ctx.name(f.name);
    if let Some(ext) = f.extension {
        emit_extension_function(ctx, m, fid, ext, w);
        return;
    }
    match f.kind {
        FunctionKind::Getter | FunctionKind::Setter => {
            // Acessores de topo: `dart.copyProperties(L, { get x() {...}, set x(v) {...} })`.
            let (text, _) = function_text(ctx, m, fid, Some(&format!("{} {}", if f.kind == FunctionKind::Getter { "get" } else { "set" }, js::prop_key(name))), None, true, None);
            accessors.push((name.to_string(), text));
        }
        _ => {
            let (text, _) = function_text(ctx, m, fid, Some(&format!("function {}", js::ident(name))), None, true, None);
            w.line(&format!("{lvar}{} = {text};", js::prop_access(name)));
        }
    }
}

fn emit_extension_function(ctx: &Ctx, m: &ModState, fid: FunctionElementId, ext: dartforge_elements::model::ExtensionId, w: &mut Writer) {
    let f = ctx.program.function(fid);
    let lvar = &ctx.libs[f.library.0 as usize].js_var;
    let name = ctx.name(f.name);
    let e = ctx.program.extension(ext);
    let tmp = FnEmitter::new(ctx, m, e.decl.unit, None, true);
    let ext_name = tmp.extension_js_name(ext);
    let key = match f.kind {
        FunctionKind::Getter => format!("{ext_name}|get#{name}"),
        FunctionKind::Setter => format!("{ext_name}|set#{name}"),
        _ => format!("{ext_name}|{name}"),
    };
    let data = &ctx.outline.extensions[ext.0 as usize];
    let on_ty = ctx.ty_of(data.on);
    // Parâmetros de tipo da extensão vêm primeiro.
    let ext_tps: Vec<(u32, String)> = data.type_params.iter().map(|p| {
        let tp = ctx.ty_param_of(*p);
        (tp.id, js::ident(&tp.name))
    }).collect();
    let (text, _) = if f.static_ {
        function_text_ext(ctx, m, fid, &ext_tps, None, ext)
    } else {
        function_text_ext(ctx, m, fid, &ext_tps, Some(&on_ty), ext)
    };
    w.line(&format!("{lvar}[{}] = {text};", js::string_literal(&key)));
}

fn function_text_ext(ctx: &Ctx, m: &ModState, fid: FunctionElementId, ext_tps: &[(u32, String)], this_ty: Option<&Ty>, ext: dartforge_elements::model::ExtensionId) -> (String, AsyncKind) {
    let f = ctx.program.function(fid);
    let FunctionRef::Function { unit, function } = f.node else {
        return (String::new(), AsyncKind::None);
    };
    let af = ctx.program.unit(unit).ast.function(function);
    let mut e = FnEmitter::new(ctx, m, unit, None, true);
    e.current_extension = Some(ext);
    let data = &ctx.outline.functions[fid.0 as usize];
    let mut params_js: Vec<String> = Vec::new();
    for (id, jsn) in ext_tps {
        e.fn_type_params.push((*id, jsn.clone()));
        params_js.push(jsn.clone());
    }
    for &pid in data.type_params.iter() {
        let p = ctx.ty_param_of(pid);
        let jsn = js::ident(&p.name);
        e.fn_type_params.insert(0, (p.id, jsn.clone()));
        params_js.push(jsn);
    }
    let kind = match af.modifier {
        ast::AsyncModifier::None => AsyncKind::None,
        ast::AsyncModifier::Async => AsyncKind::Async,
        ast::AsyncModifier::AsyncStar => AsyncKind::AsyncStar,
        ast::AsyncModifier::SyncStar => AsyncKind::SyncStar,
    };
    let ret_ty = ctx.ty_of(data.return_type);
    let sig_ty = ctx.fn_ty(fid);
    e.async_kind = kind;
    e.ret_ty = ret_ty.clone();
    if let Some(t) = this_ty {
        params_js.push("$this".into());
        e.extension_this = Some(t.clone());
        if let Some(sym) = ctx.sym("this") {
            e.declare_js(sym, "$this".into(), t.clone());
        }
    }
    let ps: &[ast::Parameter] = af.parameters.as_deref().unwrap_or(&[]);
    let (pjs, prologue) = e.declare_params(ps, Some(&sig_ty));
    if !pjs.is_empty() {
        params_js.push(pjs);
    }
    e.emit_body(&af.body);
    let body = finish_body(&mut e);
    let head = format!("function({}) {{", params_js.join(", "));
    (e.wrap_async_head(kind, &head, &prologue, &body, &ret_ty), kind)
}

fn emit_top_variables(ctx: &Ctx, m: &ModState, lib: LibraryId, vars: &[VariableId], w: &mut Writer) {
    if vars.is_empty() {
        return;
    }
    let lvar = &ctx.libs[lib.0 as usize].js_var;
    let mut lazy: Vec<String> = Vec::new();
    let mut props: Vec<String> = Vec::new();
    for &vid in vars {
        let v = ctx.program.variable(vid);
        let name = ctx.name(v.name);
        let VariableRef::TopLevel { unit, decl, index } = v.node else { continue };
        let d = ctx.program.unit(unit).ast.decl(decl);
        let DeclKind::Variables(list) = &d.kind else { continue };
        let var = &list.variables[index];
        let ty = ctx.var_ty(vid);
        let mut e = FnEmitter::new(ctx, m, unit, None, true);
        e.in_const = v.const_;
        let init = match var.initializer {
            Some(i) => {
                let (js, _) = e.emit_expr(i, Some(&ty));
                let mut pre = String::new();
                if !e.temps.is_empty() {
                    pre = format!("let {};\n", e.temps.join(", "));
                }
                format!("{pre}{}return {};", e.w.out, js.code)
            }
            None => "return null;".to_string(),
        };
        if v.late {
            m.use_sdk("_internal");
            let storage = format!("_#{name}");
            lazy.push(format!("get [{}]() {{\n{}\n}},\nset [{}](value) {{}}", js::string_literal(&storage), indent(&init), js::string_literal(&storage)));
            let getter = if var.initializer.is_some() {
                format!(
                    "get {}() {{\n  let t = {lvar}[{}];\n  if (t == null) {{ t = (() => {{\n{}\n  }})(); {lvar}[{}] = t; }}\n  return t;\n}}",
                    js::prop_key(name),
                    js::string_literal(&storage),
                    indent(&indent(&init)),
                    js::string_literal(&storage)
                )
            } else {
                format!(
                    "get {}() {{\n  let t = {lvar}[{}];\n  return t == null ? dart.throw(new _internal.LateError.fieldNI({})) : t;\n}}",
                    js::prop_key(name),
                    js::string_literal(&storage),
                    js::string_literal(name)
                )
            };
            props.push(getter);
            if !v.final_ {
                props.push(format!("set {}(v) {{\n  {lvar}[{}] = v;\n}}", js::prop_key(name), js::string_literal(&storage)));
            } else {
                props.push(format!(
                    "set {}(v) {{\n  if ({lvar}[{}] != null) dart.throw(new _internal.LateError.fieldAI({}));\n  {lvar}[{}] = v;\n}}",
                    js::prop_key(name),
                    js::string_literal(&storage),
                    js::string_literal(name),
                    js::string_literal(&storage)
                ));
            }
        } else {
            let mut entry = format!("get {}() {{\n{}\n}}", js::prop_key(name), indent(&init));
            if !v.final_ && !v.const_ {
                entry.push_str(&format!(",\nset {}(value) {{}}", js::prop_key(name)));
            }
            lazy.push(entry);
        }
    }
    if !props.is_empty() {
        w.line(&format!("dart.copyProperties({lvar}, {{"));
        w.push_raw(&indent(&props.join(",\n")));
        w.push_raw("\n});\n");
    }
    if !lazy.is_empty() {
        w.line(&format!("dart.defineLazy({lvar}, {{"));
        w.push_raw(&indent(&lazy.join(",\n")));
        w.push_raw("\n});\n");
    }
}

fn indent(s: &str) -> String {
    s.lines().map(|l| format!("  {l}")).collect::<Vec<_>>().join("\n")
}

// ---------------------------------------------------------------------------
// Classes
// ---------------------------------------------------------------------------

/// Campo de instância e como é armazenado.
struct FieldInfo {
    #[allow(dead_code)]
    vid: VariableId,
    /// Emite par getter/setter (campo virtual).
    virtual_: bool,
    name: String,
    /// Símbolo de armazenamento (`this[sym]`); `None` → propriedade direta.
    storage: Option<String>,
    late: bool,
    final_: bool,
    init: Option<ast::ExprId>,
    unit: UnitId,
    ty: Ty,
}

fn emit_class(ctx: &Ctx, m: &ModState, c: ClassId, w: &mut Writer) {
    let class = ctx.program.class(c);
    let Some(decl) = class.decl else { return };
    let unit = decl.unit;
    let lvar = ctx.libs[class.library.0 as usize].js_var.clone();
    let cname = ctx.class_name(c).to_string();
    let cref = format!("{lvar}.{cname}");
    let is_enum = class.kind == ClassKind::Enum;
    let is_mixin = class.kind == ClassKind::Mixin;
    if class.kind == ClassKind::ExtensionType {
        // Membros viram funções estáticas… (não suportado além do básico).
        return;
    }
    let generic = ctx.requires_rti(c);
    let d = ctx.program.unit(unit).ast.decl(decl.decl);
    let (members, enum_constants): (Vec<ast::MemberId>, Vec<&ast::EnumConstant>) = match &d.kind {
        DeclKind::Class(cd) => (cd.members.clone(), vec![]),
        DeclKind::Mixin(md) => (md.members.clone(), vec![]),
        DeclKind::Enum(ed) => (ed.members.clone(), ed.constants.iter().collect()),
        _ => (vec![], vec![]),
    };
    let ast = &ctx.program.unit(unit).ast;

    // Campos de instância.
    let mut fields: Vec<FieldInfo> = Vec::new();
    for &vid in &class.fields {
        let v = ctx.program.variable(vid);
        if v.static_ {
            continue;
        }
        let name = ctx.name(v.name).to_string();
        let VariableRef::Field { unit: fu, member, index } = v.node else { continue };
        let mem = ctx.program.unit(fu).ast.member(member);
        let MemberKind::Field(list) = &mem.kind else { continue };
        let var = &list.variables[index];
        let private = name.starts_with('_');
        let storage = if v.late {
            Some(m.private_sym(ctx, class.library, &format!("_#{cname}#{name}")))
        } else if private {
            Some(m.private_sym(ctx, class.library, &name))
        } else if is_enum {
            None
        } else {
            Some(m.private_sym(ctx, class.library, &format!("{cname}.{name}")))
        };
        let virtual_ = v.late || (!private && !is_enum);
        fields.push(FieldInfo { vid, name, virtual_, storage, late: v.late, final_: v.final_, init: var.initializer, unit: fu, ty: ctx.var_ty(vid) });
    }

    // Superclasse JS.
    let super_ref = superclass_js(ctx, m, c, w);

    // Corpo da classe.
    let mut cw = Writer::default();
    cw.indent = 1;
    let mut ext_methods: Vec<String> = Vec::new();
    let mut ext_accessors: Vec<String> = Vec::new();
    let natives = native_member_names(ctx, c);

    // Getters/setters de campos virtuais.
    for f in &fields {
        if !f.virtual_ {
            continue;
        }
        if let Some(sym) = &f.storage {
            let key = if f.name.starts_with('_') { format!("[{}]", m.private_sym(ctx, class.library, &f.name)) } else { js::prop_key(&f.name) };
            if f.late {
                m.use_sdk("_internal");
                let mut e = FnEmitter::new(ctx, m, f.unit, Some(c), false);
                match f.init {
                    Some(i) => {
                        let (js, _) = e.emit_expr(i, Some(&f.ty));
                        let pre = if e.temps.is_empty() { String::new() } else { format!("let {};\n", e.temps.join(", ")) };
                        cw.line(&format!("get {key}() {{"));
                        cw.line(&format!("  let t = this[{sym}];"));
                        cw.line(&format!("  if (t == null) {{ t = (() => {{ {pre}{}return {}; }})(); this[{sym}] = t; }}", e.w.out.replace('\n', " "), js.code));
                        cw.line("  return t;");
                        cw.line("}");
                    }
                    None => {
                        cw.line(&format!("get {key}() {{"));
                        cw.line(&format!("  let t = this[{sym}];"));
                        cw.line(&format!("  return t == null ? dart.throw(new _internal.LateError.fieldNI({})) : t;", js::string_literal(&f.name)));
                        cw.line("}");
                    }
                }
                if f.final_ {
                    cw.line(&format!("set {key}(v) {{"));
                    cw.line(&format!("  if (this[{sym}] != null) dart.throw(new _internal.LateError.fieldAI({}));", js::string_literal(&f.name)));
                    cw.line(&format!("  this[{sym}] = v;"));
                    cw.line("}");
                } else {
                    cw.line(&format!("set {key}(v) {{ this[{sym}] = v; }}"));
                }
            } else {
                cw.line(&format!("get {key}() {{ return this[{sym}]; }}"));
                if f.final_ {
                    cw.line(&format!("set {key}(value) {{ super{} = value; }}", if f.name.starts_with('_') { format!("[{sym}]") } else { js::prop_access(&f.name) }));
                } else {
                    cw.line(&format!("set {key}(value) {{ this[{sym}] = value; }}"));
                }
            }
            if !f.name.starts_with('_') && natives.contains(&f.name) {
                ext_accessors.push(f.name.clone());
            }
        }
    }

    // Enum: `_enumToString`.
    if is_enum {
        m.use_sdk("core");
        let name_sym = m.private_sym(ctx, ctx.program.core.expect("core"), "_name");
        let ets = m.private_sym(ctx, ctx.program.core.expect("core"), "_enumToString");
        cw.line(&format!("[{ets}]() {{ return {} + this[{name_sym}]; }}", js::string_literal(&format!("{cname}."))));
    }

    // Métodos, getters, setters, operadores, estáticos, factories.
    let mut method_sigs: Vec<(String, Ty)> = Vec::new();
    let mut getter_sigs: Vec<(String, Ty)> = Vec::new();
    let mut setter_sigs: Vec<(String, Ty)> = Vec::new();
    let mut static_methods: Vec<String> = Vec::new();
    let mut generic_methods: Vec<(String, Vec<Ty>)> = Vec::new();
    let mut ctors: Vec<(ast::MemberId, ast::Constructor)> = Vec::new();
    let _ = &mut ctors;
    let mut ctor_members: Vec<ast::MemberId> = Vec::new();
    let mut has_equals = false;
    for &mid in &members {
        let mem = ast.member(mid);
        match &mem.kind {
            MemberKind::Method(fid) => {
                let af = ast.function(*fid);
                let Some(fname) = af.name else { continue };
                let mut name = ctx.name(fname.sym).to_string();
                if af.kind == ast::FunctionKind::Operator && name == "-" && af.parameters.as_ref().is_some_and(|p| p.is_empty()) {
                    name = "unary-".to_string();
                }
                let key = if af.kind == ast::FunctionKind::Setter { format!("{name}_=") } else { name.clone() };
                let ksym = ctx.sym(&key);
                let fe = ksym.and_then(|s| if af.static_ { class.static_members.get(&s) } else { class.instance_members.get(&s) }.copied());
                let Some(feid) = fe else { continue };
                if matches!(af.body, FunctionBody::Empty) && !af.external {
                    continue; // abstrato
                }
                let jsname = js_member_name(&name);
                let mut e_tmp = FnEmitter::new(ctx, m, unit, Some(c), af.static_);
                let key_js = e_tmp.decl_member_key(c, &name);
                let data = &ctx.outline.functions[feid.0 as usize];
                let fty = ctx.fn_ty(feid);
                let is_generic = !data.type_params.is_empty();
                let head = if af.static_ {
                    let sk = js::prop_key(&static_member_name(&name));
                    match af.kind {
                        ast::FunctionKind::Getter => format!("static get {sk}"),
                        ast::FunctionKind::Setter => format!("static set {sk}"),
                        _ => format!("static {sk}"),
                    }
                } else {
                    match af.kind {
                        ast::FunctionKind::Getter => format!("get {key_js}"),
                        ast::FunctionKind::Setter => format!("set {key_js}"),
                        _ => if name.starts_with('_') { key_js.clone() } else { js::prop_key(&jsname) },
                    }
                };
                let (text, _) = function_text(ctx, m, feid, Some(&head), Some(c), af.static_, None);
                let text = if name == "==" && !af.static_ {
                    has_equals = true;
                    // `_equals(other)`: verifica nulo antes.
                    text.replacen("{\n", "{\n  if (other == null) return false;\n", 1).replace("_equals(", "_equals(")
                } else {
                    text
                };
                let text = if name == "==" { fix_equals_param(&text) } else { text };
                for line in text.lines() {
                    cw.line(line);
                }
                if af.static_ {
                    if af.kind == ast::FunctionKind::Function {
                        static_methods.push(name.clone());
                    }
                } else {
                    match af.kind {
                        ast::FunctionKind::Getter => {
                            let rt = ctx.ty_of(data.return_type);
                            getter_sigs.push((name.clone(), rt));
                            if natives.contains(&name) {
                                ext_accessors.push(name.clone());
                            }
                        }
                        ast::FunctionKind::Setter => {
                            let pt = data.parameters.first().map(|p| ctx.ty_of(p.ty)).unwrap_or(Ty::Dynamic);
                            setter_sigs.push((name.clone(), pt));
                            if natives.contains(&name) {
                                ext_accessors.push(name.clone());
                            }
                        }
                        _ => {
                            method_sigs.push((jsname.clone(), fty.clone()));
                            if is_generic {
                                let defaults: Vec<Ty> = data.type_params.iter().map(|p| {
                                    let tp = ctx.ty_param_of(*p);
                                    if tp.bound.mentions_params() { Ty::Dynamic } else { (*tp.bound).clone() }
                                }).collect();
                                generic_methods.push((jsname.clone(), defaults));
                            }
                            if natives.contains(&name) || name == "==" {
                                ext_methods.push(jsname.clone());
                            }
                        }
                    }
                }
            }
            MemberKind::Constructor(ctor) => {
                if ctor.factory {
                    emit_factory(ctx, m, c, unit, ctor, mid, &mut cw);
                    static_methods.push(ctor.name.map(|n| ctx.name(n.sym).to_string()).unwrap_or("new".into()));
                } else {
                    ctor_members.push(mid);
                }
            }
            MemberKind::Field(_) => {}
        }
    }
    let _ = has_equals;

    // `[Symbol.iterator]` para classes Iterable cuja superclasse não é Iterable.
    if !is_mixin {
        if let Some(it) = ctx.iterable_ {
            let declares_iterator = ctx.sym("iterator").is_some_and(|s| class.instance_members.contains_key(&s));
            let super_is_iterable = ctx.superclass_of(c).is_some_and(|sc| ctx.is_subclass(sc, it) && Some(sc) != ctx.object);
            if declares_iterator && ctx.is_subclass(c, it) && !super_is_iterable {
                cw.line(&format!("[Symbol.iterator]() {{ return new dart.JsIterator(this[{}]); }}", m.dartx("iterator")));
            }
        }
    }

    // Encaminhadores para `noSuchMethod` em classes concretas com membros abstratos.
    if !class.modifiers.abstract_ && !is_mixin && ctx.has_user_nsm(c) {
        m.use_sdk("_internal");
        m.use_sdk("core");
        let mut se = FnEmitter::new(ctx, m, unit, Some(c), false);
        for (name, mk) in ctx.unimplemented_abstract(c) {
            if name == "noSuchMethod" || name.starts_with('_') {
                continue;
            }
            let sym = format!("dart.const(new _internal.Symbol.new({}))", js::string_literal(&name));
            match mk {
                crate::ctx::MemberKind::Method(fid) => {
                    let ret = ctx.ty_of(ctx.outline.functions[fid.0 as usize].return_type);
                    let cast = if matches!(ret, Ty::Dynamic | Ty::Void) || ret.mentions_params() { String::new() } else { format!("{}[_as]", se.rti(&ret)) };
                    let jsname = js_member_name(&name);
                    cw.line(&format!("{}(...args) {{ return {cast}(dart.noSuchMethod(this, new core._Invocation.method({sym}, null, args, null))); }}", js::prop_key(&jsname)));
                    method_sigs.push((jsname.clone(), ctx.fn_ty(fid)));
                    if natives.contains(&name) {
                        ext_methods.push(jsname);
                    }
                }
                crate::ctx::MemberKind::Getter(fid) => {
                    let ret = ctx.ty_of(ctx.outline.functions[fid.0 as usize].return_type);
                    let cast = if matches!(ret, Ty::Dynamic | Ty::Void) || ret.mentions_params() { String::new() } else { format!("{}[_as]", se.rti(&ret)) };
                    cw.line(&format!("get {}() {{ return {cast}(dart.noSuchMethod(this, new core._Invocation.getter({sym}))); }}", js::prop_key(&name)));
                    getter_sigs.push((name.clone(), ret));
                    if natives.contains(&name) {
                        ext_accessors.push(name.clone());
                    }
                }
                crate::ctx::MemberKind::Setter(_) => {
                    let ssym = format!("dart.const(new _internal.Symbol.new({}))", js::string_literal(&format!("{name}=")));
                    cw.line(&format!("set {}(v) {{ dart.noSuchMethod(this, new core._Invocation.setter({ssym}, v)); }}", js::prop_key(&name)));
                }
                crate::ctx::MemberKind::Field(_) => {}
            }
        }
        let _ = &mut se;
    }

    // Tearoffs de construtores (`C.new == C.new`).
    if !is_mixin {
        for (&csym, &cfid) in &class.constructors {
            let cf = ctx.program.function(cfid);
            let cn = ctx.name(csym);
            let jsname = if cn.is_empty() { "new".to_string() } else { static_member_name(cn) };
            let generic_c = !ctx.class_params[c.0 as usize].is_empty();
            if generic_c {
                continue;
            }
            let call = if cf.factory { format!("{cref}.{jsname}(...args)") } else { format!("new {cref}.{jsname}(...args)") };
            cw.line(&format!("static [{}](...args) {{ return {call}; }}", js::string_literal(&format!("_#{jsname}#tearOff"))));
        }
    }

    // Getter só (ou setter só) sobrepondo campo/acessor herdado: emite o par que falta.
    {
        let this_ty = ctx.this_ty(c);
        let mut extra: Vec<String> = Vec::new();
        for (n, _) in &getter_sigs {
            if setter_sigs.iter().any(|(m, _)| m == n) || fields.iter().any(|f| f.name == *n) {
                continue;
            }
            let has_super_setter = class.supertype_class.and_then(|sc| ctx.lookup_member(&ctx.this_ty(sc), n, true)).is_some()
                || ctx.mixins_of(c).iter().any(|mx| ctx.lookup_member(&ctx.this_ty(*mx), n, true).is_some());
            if has_super_setter {
                let key = if n.starts_with('_') { format!("[{}]", m.private_sym(ctx, class.library, n)) } else { js::prop_key(n) };
                let acc = if n.starts_with('_') { format!("[{}]", m.private_sym(ctx, class.library, n)) } else { js::prop_access(n) };
                extra.push(format!("set {key}(value) {{ super{acc} = value; }}"));
            }
        }
        for (n, _) in &setter_sigs {
            if getter_sigs.iter().any(|(m, _)| m == n) || fields.iter().any(|f| f.name == *n) {
                continue;
            }
            let has_super_getter = class.supertype_class.and_then(|sc| ctx.lookup_member(&ctx.this_ty(sc), n, false)).is_some()
                || ctx.mixins_of(c).iter().any(|mx| ctx.lookup_member(&ctx.this_ty(*mx), n, false).is_some());
            if has_super_getter {
                let key = if n.starts_with('_') { format!("[{}]", m.private_sym(ctx, class.library, n)) } else { js::prop_key(n) };
                let acc = if n.starts_with('_') { format!("[{}]", m.private_sym(ctx, class.library, n)) } else { js::prop_access(n) };
                extra.push(format!("get {key}() {{ return super{acc}; }}"));
            }
        }
        let _ = this_ty;
        for x in extra {
            cw.line(&x);
        }
    }

    // Declaração da classe.
    let head = if is_mixin {
        format!("{cref} = class {cname} extends core.Object {{}};\n{cref}[dart.mixinOn] = {}$mixin_super => class {cname} extends {}$mixin_super {{", cname, cname)
    } else {
        format!("{cref} = class {cname} extends {super_ref} {{")
    };
    m.use_sdk("core");
    w.line(&head);
    w.push_raw(&cw.out);
    w.line("};");

    // Construtores generativos.
    let mut ctor_names: Vec<String> = Vec::new();
    let has_synthetic = class.constructors.values().any(|f| ctx.program.function(*f).kind == FunctionKind::SyntheticConstructor);
    if ctor_members.is_empty() && !is_mixin && (has_synthetic || class.constructors.is_empty() || is_enum) {
        // Construtor sintético.
        let jsname = "new";
        ctor_names.push(jsname.into());
        let mut body = Writer::default();
        body.indent = 1;
        if generic {
            body.line("this.$ti = this.$ti || _ti || dart.getReifiedType(this);");
        }
        emit_field_inits(ctx, m, c, &fields, &HashSet::new(), &mut body);
        if is_enum {
            let base = mixin_base_ref(ctx, c, "core._Enum");
            body.line(&format!("{base}.new.call(this, t$index, t$name);"));
        } else {
            emit_super_call_default(ctx, m, c, &mut body);
        }
        let params = if is_enum && generic { "t$index, t$name, _ti".to_string() } else if is_enum { "t$index, t$name".to_string() } else if generic { "_ti".to_string() } else { String::new() };
        w.line(&format!("({cref}.{jsname} = function({params}) {{"));
        w.push_raw(&body.out);
        w.line(&format!("}}).prototype = {cref}.prototype;"));
    }
    for mid in ctor_members {
        let mem = ast.member(mid);
        let MemberKind::Constructor(ctor) = &mem.kind else { continue };
        let name = ctor.name.map(|n| ctx.name(n.sym).to_string());
        let jsname = name.clone().map(|n| static_member_name(&n)).unwrap_or("new".into());
        ctor_names.push(jsname.clone());
        let text = emit_constructor(ctx, m, c, unit, ctor, &fields, generic, is_enum);
        w.line(&format!("({cref}.{jsname} = {text}).prototype = {cref}.prototype;"));
    }
    if is_mixin || class.modifiers.mixin {
        let mut body = Writer::default();
        body.indent = 1;
        emit_field_inits(ctx, m, c, &fields, &HashSet::new(), &mut body);
        w.line(&format!("({cref}[dart.mixinNew] = function() {{"));
        w.push_raw(&body.out);
        w.line(&format!("}}).prototype = {cref}.prototype;"));
    }

    // Recursos rti: a própria classe e as interfaces implementadas (transitivas).
    let mut recipes: Vec<String> = vec![ctx.class_recipe(c)];
    let mut iseen: HashSet<u32> = HashSet::new();
    let mut iqueue: Vec<ClassId> = ctx.interfaces_of(c);
    if is_mixin {
        iqueue.extend(class.on_classes.iter().copied());
    }
    while let Some(i) = iqueue.pop() {
        if Some(i) == ctx.object || !iseen.insert(i.0) {
            continue;
        }
        recipes.push(ctx.class_recipe(i));
        let ic = ctx.program.class(i);
        iqueue.extend(ic.interface_classes.iter().copied());
        if let Some(s) = ic.supertype_class {
            iqueue.push(s);
        }
        iqueue.extend(ic.mixin_classes.iter().copied());
    }
    let recipes_js: Vec<String> = recipes.iter().map(|r| js::string_literal(r)).collect();
    w.line(&format!("dart.addRtiResources({cref}, [{}]);", recipes_js.join(", ")));

    // Assinaturas.
    let mut se = FnEmitter::new(ctx, m, unit, Some(c), false);
    se.sig_mode = true;
    if !method_sigs.is_empty() {
        let items: Vec<String> = method_sigs.iter().map(|(n, t)| format!("{}: _ti => {}", sig_key(&n, &se, c), se.rti(t))).collect();
        w.line(&format!("dart.setMethodSignature({cref}, () => Object.setPrototypeOf({{{}}}, dart.getMethods(Object.getPrototypeOf({cref}))));", items.join(", ")));
    }
    if !generic_methods.is_empty() {
        let items: Vec<String> = generic_methods
            .iter()
            .map(|(n, ds)| format!("{}: _ti => [{}]", sig_key(n, &se, c), ds.iter().map(|d| se.rti(d)).collect::<Vec<_>>().join(", ")))
            .collect();
        w.line(&format!("dart.setMethodsDefaultTypeArgSignature({cref}, () => Object.setPrototypeOf({{{}}}, dart.getMethodsDefaultTypeArgs(Object.getPrototypeOf({cref}))));", items.join(", ")));
    }
    if !getter_sigs.is_empty() {
        let items: Vec<String> = getter_sigs.iter().map(|(n, t)| format!("{}: _ti => {}", sig_key(n, &se, c), se.rti(t))).collect();
        w.line(&format!("dart.setGetterSignature({cref}, () => Object.setPrototypeOf({{{}}}, dart.getGetters(Object.getPrototypeOf({cref}))));", items.join(", ")));
    }
    if !setter_sigs.is_empty() {
        let items: Vec<String> = setter_sigs.iter().map(|(n, t)| format!("{}: _ti => {}", sig_key(n, &se, c), se.rti(t))).collect();
        w.line(&format!("dart.setSetterSignature({cref}, () => Object.setPrototypeOf({{{}}}, dart.getSetters(Object.getPrototypeOf({cref}))));", items.join(", ")));
    }
    if !static_methods.is_empty() {
        let items: Vec<String> = static_methods.iter().map(|n| js::string_literal(&static_member_name(n))).collect();
        w.line(&format!("dart.setStaticMethodSignature({cref}, () => [{}]);", items.join(", ")));
    }
    let lib_uri = js::string_literal(&ctx.program.library(class.library).uri);
    w.line(&format!("dart.setLibraryUri({cref}, {lib_uri});"));
    if !fields.is_empty() {
        let items: Vec<String> = fields
            .iter()
            .map(|f| {
                let key = match &f.storage {
                    Some(s) if f.name.starts_with('_') || f.late => format!("[{s}]"),
                    _ => js::prop_key(&f.name),
                };
                let ty = if f.late { f.ty.with_nullable(true) } else { f.ty.clone() };
                format!("{key}: {{type: _ti => {}, isConst: false, isFinal: {}}}", se.rti(&ty), f.final_ && !f.late)
            })
            .collect();
        w.line(&format!("dart.setFieldSignature({cref}, () => Object.setPrototypeOf({{{}}}, dart.getFields(Object.getPrototypeOf({cref}))));", items.join(", ")));
    }
    // Estáticos: campos.
    let static_fields: Vec<VariableId> = class.fields.iter().copied().filter(|v| ctx.program.variable(*v).static_).collect();
    let mut static_names: Vec<String> = static_fields.iter().map(|v| ctx.name(ctx.program.variable(*v).name).to_string()).collect();
    if is_enum {
        static_names.insert(0, "values".into());
        for ec in &enum_constants {
            static_names.push(ctx.name(ec.name.sym).to_string());
        }
    }
    if !static_names.is_empty() {
        let items: Vec<String> = static_names.iter().map(|n| js::string_literal(&static_member_name(n))).collect();
        w.line(&format!("dart.setStaticFieldSignature({cref}, () => [{}]);", items.join(", ")));
    }
    // Extensões (membros de interfaces nativas).
    ext_methods.sort();
    ext_methods.dedup();
    ext_accessors.sort();
    ext_accessors.dedup();
    if !ext_methods.is_empty() {
        let items: Vec<String> = ext_methods.iter().map(|n| js::string_literal(n)).collect();
        w.line(&format!("dart.defineExtensionMethods({cref}, [{}]);", items.join(", ")));
    }
    if !ext_accessors.is_empty() {
        let items: Vec<String> = ext_accessors.iter().map(|n| js::string_literal(n)).collect();
        w.line(&format!("dart.defineExtensionAccessors({cref}, [{}]);", items.join(", ")));
    }
    // Campos estáticos e constantes de enum (lazy).
    let mut lazy: Vec<String> = Vec::new();
    if is_enum {
        let mut e = FnEmitter::new(ctx, m, unit, Some(c), true);
        e.in_const = true;
        let mut names = Vec::new();
        for (i, ec) in enum_constants.iter().enumerate() {
            let n = ctx.name(ec.name.sym).to_string();
            names.push(format!("{cref}{}", js::prop_access(&static_member_name(&n))));
            let ctor_name = ec.constructor.map(|c| ctx.name(c.sym).to_string()).unwrap_or_default();
            let mut args: Vec<String> = vec![i.to_string(), js::string_literal(&n)];
            if let Some(a) = &ec.arguments {
                let (pos, named, _) = e.emit_args_plain(a);
                args.extend(pos);
                if let Some(nm) = named {
                    args.push(nm);
                }
            }
            let jsname = if ctor_name.is_empty() { "new".to_string() } else { static_member_name(&ctor_name) };
            if generic {
                let mut tys: Vec<Ty> = ec.type_args.iter().map(|t| e.resolve_type(*t)).collect();
                while tys.len() < ctx.class_params[c.0 as usize].len() {
                    tys.push(Ty::Dynamic);
                }
                let t = Ty::Iface { class: c, args: tys, nullable: false };
                args.insert(2, e.rti(&t));
            }
            lazy.push(format!("get {}() {{ return dart.const(new {cref}.{jsname}({})); }}", js::prop_key(&static_member_name(&n)), args.join(", ")));
        }
        let rti = e.rti(&Ty::iface(c));
        lazy.insert(0, format!("get values() {{ return dart.constList({rti}, [{}]); }}", names.join(", ")));
    }
    for vid in static_fields {
        let v = ctx.program.variable(vid);
        let name = static_member_name(ctx.name(v.name));
        let VariableRef::Field { unit: fu, member, index } = v.node else { continue };
        let mem = ctx.program.unit(fu).ast.member(member);
        let MemberKind::Field(list) = &mem.kind else { continue };
        let var = &list.variables[index];
        let ty = ctx.var_ty(vid);
        let mut e = FnEmitter::new(ctx, m, fu, Some(c), true);
        e.in_const = v.const_;
        let init = match var.initializer {
            Some(i) => {
                let (js, _) = e.emit_expr(i, Some(&ty));
                let pre = if e.temps.is_empty() { String::new() } else { format!("let {};\n", e.temps.join(", ")) };
                format!("{pre}{}return {};", e.w.out, js.code)
            }
            None => "return null;".to_string(),
        };
        if v.late {
            m.use_sdk("_internal");
            let storage = format!("_#{name}");
            lazy.push(format!("get [{}]() {{\n{}\n}},\nset [{}](value) {{}}", js::string_literal(&storage), indent(&init), js::string_literal(&storage)));
            let getter = if var.initializer.is_some() {
                format!(
                    "get {}() {{\n  let t = {cref}[{}];\n  if (t == null) {{ t = (() => {{\n{}\n  }})(); {cref}[{}] = t; }}\n  return t;\n}}",
                    js::prop_key(&name),
                    js::string_literal(&storage),
                    indent(&indent(&init)),
                    js::string_literal(&storage)
                )
            } else {
                format!(
                    "get {}() {{\n  let t = {cref}[{}];\n  return t == null ? dart.throw(new _internal.LateError.fieldNI({})) : t;\n}}",
                    js::prop_key(&name),
                    js::string_literal(&storage),
                    js::string_literal(&name)
                )
            };
            let setter = format!("set {}(v) {{\n  {cref}[{}] = v;\n}}", js::prop_key(&name), js::string_literal(&storage));
            w.line(&format!("dart.copyProperties({cref}, {{\n{},\n{}\n}});", indent(&getter), indent(&setter)));
        } else {
            let mut entry = format!("get {}() {{\n{}\n}}", js::prop_key(&name), indent(&init));
            if !v.final_ && !v.const_ {
                entry.push_str(&format!(",\nset {}(value) {{}}", js::prop_key(&name)));
            }
            lazy.push(entry);
        }
    }
    if !lazy.is_empty() {
        w.line(&format!("dart.defineLazy({cref}, {{"));
        w.push_raw(&indent(&lazy.join(",\n")));
        w.push_raw("\n});\n");
    }
}

fn fix_equals_param(text: &str) -> String {
    // Garante que o parâmetro de `_equals` se chama `other` no teste de nulo.
    // O texto começa com `_equals(<param>) {` — renomeia o teste para o nome real.
    if let Some(rest) = text.strip_prefix("_equals(") {
        if let Some(end) = rest.find(')') {
            let pname = rest[..end].trim().to_string();
            if !pname.is_empty() && pname != "other" {
                return text.replacen("if (other == null) return false;", &format!("if ({pname} == null) return false;"), 1);
            }
        }
    }
    text.to_string()
}

fn sig_key(name: &str, e: &FnEmitter, c: ClassId) -> String {
    if name.starts_with('_') && !matches!(name, "_equals" | "_get" | "_set" | "_negate") {
        e.decl_member_key(c, name)
    } else {
        js::prop_key(name)
    }
}

/// Nomes públicos de membros de instância de supertipos que são interfaces nativas.
fn native_member_names(ctx: &Ctx, c: ClassId) -> HashSet<String> {
    let mut names = HashSet::new();
    let mut seen: HashSet<u32> = HashSet::new();
    let mut queue = vec![c];
    if let Some(o) = ctx.object {
        queue.push(o);
    }
    while let Some(k) = queue.pop() {
        if !seen.insert(k.0) {
            continue;
        }
        let class = ctx.program.class(k);
        if ctx.ext_set.contains(&k) {
            for (&sym, &fid) in &class.instance_members {
                let f = ctx.program.function(fid);
                let n = ctx.name(f.name);
                if !n.starts_with('_') {
                    let _ = sym;
                    names.insert(n.to_string());
                }
            }
        }
        for s in &ctx.class_supers[k.0 as usize] {
            if let Some(sc) = s.class() {
                queue.push(sc);
            }
        }
    }
    names
}

/// Referência JS à superclasse (com aplicação de mixins emitida antes).
fn superclass_js(ctx: &Ctx, m: &ModState, c: ClassId, w: &mut Writer) -> String {
    let class = ctx.program.class(c);
    let (mut base, base_class) = if class.kind == ClassKind::Enum {
        m.use_sdk("core");
        ("core._Enum".to_string(), ctx.underscore_enum)
    } else {
        match ctx.superclass_of(c) {
            Some(s) if Some(s) != ctx.object => {
                let tmp = FnEmitter::new(ctx, m, class.decl.map(|d| d.unit).unwrap_or(UnitId(0)), None, true);
                (tmp.class_ref(s), Some(s))
            }
            _ => {
                m.use_sdk("core");
                ("core.Object".to_string(), None)
            }
        }
    };
    for (i, &mx) in ctx.mixins_of(c).iter().enumerate() {
        let tmp = FnEmitter::new(ctx, m, class.decl.map(|d| d.unit).unwrap_or(UnitId(0)), None, true);
        let mref = tmp.class_ref(mx);
        let app = format!("{}$mixin{}", ctx.class_name(c), i);
        w.line(&format!("const {app} = class {app} extends {base} {{}};"));
        // Construtores encaminhadores para a superclasse.
        let mut ctor_names: Vec<String> = match base_class {
            Some(b) if Some(b) != ctx.object => ctx
                .program
                .class(b)
                .constructors
                .iter()
                .filter(|(_, f)| !ctx.program.function(**f).factory)
                .map(|(s, _)| {
                    let n = ctx.name(*s);
                    if n.is_empty() { "new".to_string() } else { n.to_string() }
                })
                .collect(),
            _ => vec!["new".into()],
        };
        if ctor_names.is_empty() {
            ctor_names.push("new".into());
        }
        for n in ctor_names {
            let n = if n == "new" { n } else { static_member_name(&n) };
            if base == "core.Object" {
                w.line(&format!("({app}.{n} = function() {{ if ({mref}[dart.mixinNew]) {mref}[dart.mixinNew].call(this); }}).prototype = {app}.prototype;"));
            } else {
                w.line(&format!("({app}.{n} = function(...args) {{ if ({mref}[dart.mixinNew]) {mref}[dart.mixinNew].call(this); {base}.{n}.apply(this, args); }}).prototype = {app}.prototype;"));
            }
        }
        w.line(&format!("dart.applyMixin({app}, {mref});"));
        base = app;
    }
    base
}

/// Inicializadores de campos declarados (`int x = 0;` / `null`), exceto os já
/// inicializados por `this.x` ou pela lista de inicialização.
fn emit_field_inits(ctx: &Ctx, m: &ModState, c: ClassId, fields: &[FieldInfo], skip: &HashSet<String>, body: &mut Writer) {
    for f in fields {
        if skip.contains(&f.name) {
            continue;
        }
        let target = match &f.storage {
            Some(s) => format!("this[{s}]"),
            None => format!("this{}", js::prop_access(&f.name)),
        };
        match f.init {
            Some(i) if !f.late => {
                let mut e = FnEmitter::new(ctx, m, f.unit, Some(c), false);
                let (js, _) = e.emit_expr(i, Some(&f.ty));
                if !e.temps.is_empty() {
                    body.line(&format!("let {};", e.temps.join(", ")));
                }
                for line in e.w.out.lines() {
                    body.line(line);
                }
                body.line(&format!("{target} = {};", js.code));
            }
            _ => body.line(&format!("{target} = null;")),
        }
    }
}

fn emit_super_call_default(ctx: &Ctx, m: &ModState, c: ClassId, body: &mut Writer) {
    let class = ctx.program.class(c);
    let sup = match ctx.superclass_of(c) {
        Some(s) if Some(s) != ctx.object => Some(s),
        _ => None,
    };
    if sup.is_none() && ctx.mixins_of(c).is_empty() {
        return;
    }
    let tmp = FnEmitter::new(ctx, m, class.decl.map(|d| d.unit).unwrap_or(UnitId(0)), None, true);
    let sref = sup.map(|s| tmp.class_ref(s)).unwrap_or_else(|| "core.Object".to_string());
    let sgeneric = sup.is_some_and(|s| ctx.requires_rti(s));
    let base = mixin_base_ref(ctx, c, &sref);
    if sgeneric {
        body.line(&format!("{base}.new.call(this, null);"));
    } else {
        body.line(&format!("{base}.new.call(this);"));
    }
}

/// Se a classe tem mixins, o construtor da superclasse imediata é o da última aplicação.
fn mixin_base_ref(ctx: &Ctx, c: ClassId, sref: &str) -> String {
    let class = ctx.program.class(c);
    if ctx.mixins_of(c).is_empty() {
        sref.to_string()
    } else {
        format!("{}$mixin{}", ctx.class_name(c), ctx.mixins_of(c).len() - 1)
    }
}

/// Texto `function(params) { ... }` de um construtor generativo.
fn emit_constructor(ctx: &Ctx, m: &ModState, c: ClassId, unit: UnitId, ctor: &ast::Constructor, fields: &[FieldInfo], generic: bool, is_enum: bool) -> String {
    let class = ctx.program.class(c);
    let mut e = FnEmitter::new(ctx, m, unit, Some(c), false);
    e.in_constructor = true;
    let ctor_sym = ctor.name.map(|n| n.sym).or(ctx.empty_sym);
    let fid = ctor_sym.and_then(|s| class.constructors.get(&s).copied());
    let sig = fid.map(|f| ctx.fn_ty(f));
    let (pjs, prologue) = e.declare_params(&ctor.parameters, sig.as_ref());
    let mut params: Vec<String> = Vec::new();
    if is_enum {
        params.push("t$index".into());
        params.push("t$name".into());
    }
    if generic {
        params.push("_ti".into());
    }
    if !pjs.is_empty() {
        params.push(pjs);
    }
    let mut body = Writer::default();
    body.indent = 1;
    for line in prologue.lines() {
        body.line(line);
    }
    if generic {
        body.line("this.$ti = this.$ti || _ti || dart.getReifiedType(this);");
    }
    // Redirecionamento `: this(...)`.
    let redirect = ctor.initializers.iter().find_map(|i| match i {
        ast::Initializer::Redirect { constructor, arguments, .. } => Some((constructor, arguments)),
        _ => None,
    });
    let cref = e.class_ref(c);
    if let Some((target, args)) = redirect {
        let tname = target.map(|n| static_member_name(ctx.name(n.sym))).unwrap_or("new".into());
        let tsym = target.map(|n| n.sym).or(ctx.empty_sym);
        let tfid = tsym.and_then(|s| class.constructors.get(&s).copied());
        let tsig = tfid.map(|f| ctx.fn_ty(f)).unwrap_or(Ty::Dynamic);
        let (arg_js, _, _) = e.emit_args_for(&tsig, args, None);
        let mut all: Vec<String> = vec!["this".into()];
        if is_enum {
            all.push("t$index".into());
            all.push("t$name".into());
        }
        if generic {
            all.push("null".into());
        }
        all.extend(arg_js);
        for line in e.w.out.lines() {
            body.line(line);
        }
        body.line(&format!("{cref}.{tname}.call({});", all.join(", ")));
        let mut text = String::new();
        text.push_str(&format!("function({}) {{\n", params.join(", ")));
        if !e.temps.is_empty() {
            text.push_str(&format!("  let {};\n", e.temps.join(", ")));
        }
        text.push_str(&body.out);
        text.push('}');
        return text;
    }
    // Inicializadores de campo correm primeiro (mesmo quando `this.x` ou a
    // lista os sobrescrevem); campos sem inicializador que serão atribuídos
    // não precisam do `null` inicial.
    let mut skip: HashSet<String> = HashSet::new();
    for p in ctor.parameters.iter() {
        if p.this_ {
            if let Some(n) = p.name {
                let name = ctx.name(n.sym).to_string();
                if fields.iter().any(|f| f.name == name && f.init.is_none()) {
                    skip.insert(name);
                }
            }
        }
    }
    for i in ctor.initializers.iter() {
        if let ast::Initializer::Field { name, .. } = i {
            let name = ctx.name(name.sym).to_string();
            if fields.iter().any(|f| f.name == name && f.init.is_none()) {
                skip.insert(name);
            }
        }
    }
    emit_field_inits(ctx, m, c, fields, &skip, &mut body);
    // `this.x` params.
    for p in ctor.parameters.iter() {
        if p.this_ {
            if let Some(n) = p.name {
                let name = ctx.name(n.sym).to_string();
                let target = field_target(fields, &name);
                let jsn = e.lookup_local(n.sym).map(|l| l.js.clone()).unwrap_or(js::ident(&name));
                body.line(&format!("{target} = {jsn};"));
            }
        }
    }
    // Lista de inicialização.
    let mut super_call: Option<String> = None;
    for i in ctor.initializers.iter() {
        match i {
            ast::Initializer::Field { name, value, .. } => {
                let n = ctx.name(name.sym).to_string();
                let fty = fields.iter().find(|f| f.name == n).map(|f| f.ty.clone());
                let (js, _) = e.emit_expr(*value, fty.as_ref());
                flush_stmts(&mut e, &mut body);
                let target = field_target(fields, &n);
                body.line(&format!("{target} = {};", js.code));
            }
            ast::Initializer::Assert { condition, message, .. } => {
                let (cjs, _) = e.emit_cond(*condition);
                let msg = message.map(|mm| e.emit_expr(mm, None).0.code).unwrap_or("null".into());
                flush_stmts(&mut e, &mut body);
                body.line(&format!("if (!({cjs})) dart.assertFailed({msg}, null, 0, 0, \"\");"));
            }
            ast::Initializer::Super { constructor, arguments, .. } => {
                let sup = ctx.superclass_of(c);
                if let Some(s) = sup {
                    if Some(s) != ctx.object {
                        let sname = constructor.map(|n| static_member_name(ctx.name(n.sym))).unwrap_or("new".into());
                        let ssym = constructor.map(|n| n.sym).or(ctx.empty_sym);
                        let sfid = ssym.and_then(|x| ctx.program.class(s).constructors.get(&x).copied());
                        let ssig = sfid.map(|f| ctx.fn_ty(f)).unwrap_or(Ty::Dynamic);
                        let (arg_js, _, _) = e.emit_args_for(&ssig, arguments, None);
                        flush_stmts(&mut e, &mut body);
                        let sref = e.class_ref(s);
                        let base = mixin_base_ref(ctx, c, &sref);
                        let mut all: Vec<String> = vec!["this".into()];
                        if is_enum {
                            all.push("t$index".into());
                            all.push("t$name".into());
                        } else if ctx.requires_rti(s) {
                            all.push("null".into());
                        }
                        // `super.x` posicionais entram após os explícitos; nomeados juntam-se ao `opts`.
                        let mut pos_js: Vec<String> = Vec::new();
                        let mut named_js: Vec<String> = Vec::new();
                        let mut opts_js: Option<String> = None;
                        for a in arg_js {
                            if a.starts_with('{') && a.ends_with('}') {
                                opts_js = Some(a);
                            } else {
                                pos_js.push(a);
                            }
                        }
                        for p in ctor.parameters.iter().filter(|p| p.super_) {
                            let Some(n) = p.name else { continue };
                            let jsn = e.lookup_local(n.sym).map(|l| l.js.clone()).unwrap_or(js::ident(ctx.name(n.sym)));
                            if p.kind == ast::ParameterKind::Named {
                                named_js.push(format!("{}: {jsn}", js::prop_key(ctx.name(n.sym))));
                            } else {
                                pos_js.push(jsn);
                            }
                        }
                        all.extend(pos_js);
                        match (opts_js, named_js.is_empty()) {
                            (Some(o), true) => all.push(o),
                            (Some(o), false) => all.push(format!("{{...{o}, {}}}", named_js.join(", "))),
                            (None, false) => all.push(format!("{{{}}}", named_js.join(", "))),
                            (None, true) => {}
                        }
                        super_call = Some(format!("{base}.{sname}.call({});", all.join(", ")));
                    }
                }
            }
            ast::Initializer::Redirect { .. } => {}
        }
    }
    // `super.x` params (Dart 2.17) → passam para o super.
    if super_call.is_none() {
        if is_enum {
            m.use_sdk("core");
            let base = mixin_base_ref(ctx, c, "core._Enum");
            super_call = Some(format!("{base}.new.call(this, t$index, t$name);"));
        } else if ctx.superclass_of(c).is_none_or(|s| Some(s) == ctx.object) && !ctx.mixins_of(c).is_empty() {
            let base = mixin_base_ref(ctx, c, "core.Object");
            super_call = Some(format!("{base}.new.call(this);"));
        } else if let Some(s) = ctx.superclass_of(c) {
            if Some(s) != ctx.object {
                let sref = e.class_ref(s);
                let base = mixin_base_ref(ctx, c, &sref);
                let mut super_params: Vec<String> = Vec::new();
                let mut super_named: Vec<String> = Vec::new();
                for p in ctor.parameters.iter().filter(|p| p.super_) {
                    let Some(n) = p.name else { continue };
                    let jsn = e.lookup_local(n.sym).map(|l| l.js.clone()).unwrap_or(js::ident(ctx.name(n.sym)));
                    if p.kind == ast::ParameterKind::Named {
                        let key = js::prop_key(ctx.name(n.sym));
                        if p.default_value.is_none() && !p.required {
                            super_named.push(format!("...(opts && {} in opts ? {{{key}: {jsn}}} : {{}})", js::string_literal(ctx.name(n.sym))));
                        } else {
                            super_named.push(format!("{key}: {jsn}"));
                        }
                    } else {
                        super_params.push(jsn);
                    }
                }
                let mut all: Vec<String> = vec!["this".into()];
                if ctx.requires_rti(s) {
                    all.push("null".into());
                }
                all.extend(super_params);
                if !super_named.is_empty() {
                    // Posicionais opcionais omitidos do super antes de `opts`.
                    let ssym = ctx.empty_sym;
                    if let Some(sfid) = ssym.and_then(|x| ctx.program.class(s).constructors.get(&x).copied()) {
                        let n_pos = ctx.outline.functions[sfid.0 as usize].parameters.iter().filter(|p| p.kind != ast::ParameterKind::Named).count();
                        while all.len() - 1 - (if ctx.requires_rti(s) { 1 } else { 0 }) < n_pos {
                            all.push("void 0".into());
                        }
                    }
                    all.push(format!("{{{}}}", super_named.join(", ")));
                }
                super_call = Some(format!("{base}.new.call({});", all.join(", ")));
            }
        }
    }
    if let Some(sc) = super_call {
        body.line(&sc);
    }
    // No corpo, `x` de `this.x` refere-se ao campo.
    for p in ctor.parameters.iter() {
        if p.this_ {
            if let Some(n) = p.name {
                for sc in e.scopes.iter_mut() {
                    sc.remove(&n.sym);
                }
            }
        }
    }
    // Corpo.
    e.emit_body(&ctor.body);
    flush_stmts(&mut e, &mut body);
    let mut text = String::new();
    text.push_str(&format!("function({}) {{\n", params.join(", ")));
    if !e.temps.is_empty() {
        text.push_str(&format!("  let {};\n", e.temps.join(", ")));
    }
    text.push_str(&body.out);
    text.push('}');
    text
}

fn flush_stmts(e: &mut FnEmitter, body: &mut Writer) {
    let out = std::mem::take(&mut e.w.out);
    for line in out.lines() {
        body.line(line);
    }
}

fn field_target(fields: &[FieldInfo], name: &str) -> String {
    match fields.iter().find(|f| f.name == name) {
        Some(FieldInfo { storage: Some(s), .. }) => format!("this[{s}]"),
        _ => format!("this{}", js::prop_access(name)),
    }
}

/// `static nome(params) { corpo }` de um factory.
fn emit_factory(ctx: &Ctx, m: &ModState, c: ClassId, unit: UnitId, ctor: &ast::Constructor, mid: ast::MemberId, cw: &mut Writer) {
    let class = ctx.program.class(c);
    let name = ctor.name.map(|n| static_member_name(ctx.name(n.sym))).unwrap_or("new".into());
    let ctor_sym = ctor.name.map(|n| n.sym).or(ctx.empty_sym);
    let fid = ctor_sym.and_then(|s| class.constructors.get(&s).copied());
    let _ = mid;
    let generic = !ctx.class_params[c.0 as usize].is_empty();
    let mut e = FnEmitter::new(ctx, m, unit, Some(c), true);
    // Num factory os parâmetros de tipo da classe entram como parâmetros de função.
    let mut params: Vec<String> = Vec::new();
    if generic {
        params.push("_ti".into());
        e.factory_ti = true;
    }
    if ctor.redirect.is_some() {
        // Redirecionamento: as chamadas diretas já resolvem o alvo; o corpo
        // encaminha para servir aos tearoffs (`C.new` como valor).
        let body = match fid.and_then(|f| e.factory_redirect_target(f)) {
            Some((tclass, tname, is_target_factory)) => {
                let tref = e.class_ref(tclass);
                let tjs = if tname.is_empty() { "new".to_string() } else { static_member_name(&tname) };
                let mut args: Vec<String> = Vec::new();
                if ctx.requires_rti(tclass) {
                    if tclass == c && generic {
                        args.push("_ti".into());
                    } else {
                        args.push(e.rti(&ctx.this_ty_default(tclass)));
                    }
                }
                args.push("...args".into());
                if is_target_factory {
                    format!("return {tref}.{tjs}({});", args.join(", "))
                } else {
                    format!("return new {tref}.{tjs}({});", args.join(", "))
                }
            }
            None => String::new(),
        };
        let mut ps = params.clone();
        ps.push("...args".into());
        cw.line(&format!("static {}({}) {{ {body} }}", js::prop_key(&name), ps.join(", ")));
        return;
    }
    let sig = fid.map(|f| ctx.fn_ty(f));
    let (pjs, prologue) = e.declare_params(&ctor.parameters, sig.as_ref());
    if !pjs.is_empty() {
        params.push(pjs);
    }
    let ret_ty = ctx.this_ty(c);
    e.ret_ty = ret_ty.clone();
    e.emit_body(&ctor.body);
    let body = finish_body(&mut e);
    let head = format!("static {}({}) {{", js::prop_key(&name), params.join(", "));
    let text = e.wrap_async_head(AsyncKind::None, &head, &prologue, &body, &ret_ty);
    for line in text.lines() {
        cw.line(line);
    }
}

#[allow(dead_code)]
fn _unused(_: Js, _: HashMap<u32, u32>, _: BTreeMap<u32, u32>, _: u8) {
    let _ = P_ASSIGN;
}
