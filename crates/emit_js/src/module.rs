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
        } else if lib != self.lib {
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
    for (i, lib) in ctx.program.libraries.iter().enumerate() {
        let lid = LibraryId(i as u32);
        let info = &ctx.libs[i];
        if info.is_sdk || lib.units.is_empty() {
            continue;
        }
        let text = emit_library(ctx, lid);
        modulos.push((info.module_path.clone(), text));
        if Some(lid) == ctx.program.entry {
            entry_ident = info.ident.clone();
            entry_path = info.module_path.clone();
            main_async = lib_main_is_async(ctx, lid);
        }
    }
    let entrada = format!(
        "import {{ dart }} from './dart_sdk.js';\nimport {{ {entry_ident} }} from './{entry_path}';\n{}\n",
        if main_async { format!("await {entry_ident}.main();") } else { format!("{entry_ident}.main();") }
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
        for &m in &class.mixin_classes {
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

fn emit_library(ctx: &Ctx, lib: LibraryId) -> String {
    let m = ModState::new(lib);
    let info = &ctx.libs[lib.0 as usize];
    let lvar = info.js_var.clone();
    let library = ctx.program.library(lib);
    let mut body = Writer::default();

    // Classes da biblioteca (todas as unidades).
    let mut classes: Vec<ClassId> = Vec::new();
    let mut functions: Vec<FunctionElementId> = Vec::new();
    let mut variables: Vec<VariableId> = Vec::new();
    for (i, c) in ctx.program.classes.iter().enumerate() {
        if c.library == lib && c.decl.is_some() {
            classes.push(ClassId(i as u32));
        }
    }
    for (i, f) in ctx.program.functions.iter().enumerate() {
        if f.library == lib && f.class.is_none() && f.kind != FunctionKind::ImplicitAccessor {
            functions.push(FunctionElementId(i as u32));
        }
    }
    for (i, v) in ctx.program.variables.iter().enumerate() {
        if v.library == lib && v.class.is_none() && v.extension.is_none() {
            variables.push(VariableId(i as u32));
        }
    }
    let ordered = order_classes(ctx, &classes);
    for c in ordered {
        m.note_class(c);
        emit_class(ctx, &m, c, &mut body);
    }
    // Funções de topo e de extensão.
    for fid in functions {
        emit_top_function(ctx, &m, fid, &mut body);
    }
    // Variáveis de topo.
    emit_top_variables(ctx, &m, &variables, &mut body);

    // Regras rti das classes do módulo (e referenciadas).
    let rules = emit_rules(ctx, &m);

    // Prelúdio.
    let mut out = String::new();
    out.push_str(&format!("var {lvar} = Object.create(dart.library);\nexport {{ {lvar} as {} }};\n", info.ident));
    let mut sdk: BTreeSet<String> = m.sdk_used.borrow().clone();
    for s in ["dart", "dart_rti", "core", "dartx"] {
        sdk.insert(s.to_string());
    }
    let sdk_list: Vec<String> = sdk.iter().map(|s| if s == "html" || s == "svg" { format!("{s}") } else { s.clone() }).collect();
    out.push_str(&format!("import {{ {} }} from './dart_sdk.js';\n", sdk_list.join(", ")));
    for &ul in m.user_imports.borrow().iter() {
        let uinfo = &ctx.libs[ul as usize];
        out.push_str(&format!("import {{ {} as {} }} from './{}';\n", uinfo.ident, uinfo.js_var, uinfo.module_path));
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
    if !sdk.contains("async") {
        out = out.replace("import { ", "import { async, ");
    }
    for (var, (l, name)) in m.private_syms.borrow().iter() {
        let lv = &ctx.libs[*l as usize].js_var;
        out.push_str(&format!("var {var} = dart.privateName({lv}, {});\n", js::string_literal(name)));
    }
    out.push_str("dart._checkModuleNullSafetyMode(true);\n");
    out.push_str(&body.out);
    out.push_str(&rules);
    let uri = library.uri.clone();
    out.push_str(&format!(
        "dart.trackLibraries({}, {{\n  {}: {lvar}\n}}, {{\n}}, null);\n",
        js::string_literal(&info.ident),
        js::string_literal(&uri)
    ));
    out
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

fn emit_top_function(ctx: &Ctx, m: &ModState, fid: FunctionElementId, w: &mut Writer) {
    let f = ctx.program.function(fid);
    let lvar = &ctx.libs[f.library.0 as usize].js_var;
    let name = ctx.name(f.name);
    if let Some(ext) = f.extension {
        emit_extension_function(ctx, m, fid, ext, w);
        return;
    }
    match f.kind {
        FunctionKind::Getter | FunctionKind::Setter => {
            // Acessores de topo: `dart.copyProperties(L, { get x() {...} })`.
            let (text, _) = function_text(ctx, m, fid, Some(&format!("{} {}", if f.kind == FunctionKind::Getter { "get" } else { "set" }, js::prop_key(name))), None, true, None);
            w.line(&format!("dart.copyProperties({lvar}, {{"));
            for line in text.lines() {
                w.line(&format!("  {line}"));
            }
            w.line("});");
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
        function_text_ext(ctx, m, fid, &ext_tps, None)
    } else {
        function_text_ext(ctx, m, fid, &ext_tps, Some(&on_ty))
    };
    w.line(&format!("{lvar}[{}] = {text};", js::string_literal(&key)));
}

fn function_text_ext(ctx: &Ctx, m: &ModState, fid: FunctionElementId, ext_tps: &[(u32, String)], this_ty: Option<&Ty>) -> (String, AsyncKind) {
    let f = ctx.program.function(fid);
    let FunctionRef::Function { unit, function } = f.node else {
        return (String::new(), AsyncKind::None);
    };
    let af = ctx.program.unit(unit).ast.function(function);
    let mut e = FnEmitter::new(ctx, m, unit, None, true);
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

fn emit_top_variables(ctx: &Ctx, m: &ModState, vars: &[VariableId], w: &mut Writer) {
    if vars.is_empty() {
        return;
    }
    let lvar = &ctx.libs[m.lib.0 as usize].js_var;
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
    vid: VariableId,
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
    let generic = !ctx.class_params[c.0 as usize].is_empty();
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
        fields.push(FieldInfo { vid, name, storage, late: v.late, final_: v.final_, init: var.initializer, unit: fu, ty: ctx.var_ty(vid) });
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
        if let Some(sym) = &f.storage {
            let key = if f.name.starts_with('_') { format!("[{sym}]") } else { js::prop_key(&f.name) };
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
                let name = ctx.name(fname.sym).to_string();
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
                let head = match af.kind {
                    ast::FunctionKind::Getter => format!("{}get {key_js}", if af.static_ { "static " } else { "" }),
                    ast::FunctionKind::Setter => format!("{}set {key_js}", if af.static_ { "static " } else { "" }),
                    _ => format!("{}{}", if af.static_ { "static " } else { "" }, if name.starts_with('_') { key_js.clone() } else { js::prop_key(&jsname) }),
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
    if ctor_members.is_empty() && !is_mixin {
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
            let sup = format!("core._Enum.new.call(this, t$index, t$name);");
            body.line(&sup);
        } else {
            emit_super_call_default(ctx, m, c, &mut body);
        }
        let params = if is_enum { "t$index, t$name".to_string() } else if generic { "_ti".to_string() } else { String::new() };
        w.line(&format!("({cref}.{jsname} = function({params}) {{"));
        w.push_raw(&body.out);
        w.line(&format!("}}).prototype = {cref}.prototype;"));
    }
    for mid in ctor_members {
        let mem = ast.member(mid);
        let MemberKind::Constructor(ctor) = &mem.kind else { continue };
        let name = ctor.name.map(|n| ctx.name(n.sym).to_string());
        let jsname = name.clone().unwrap_or("new".into());
        ctor_names.push(jsname.clone());
        let text = emit_constructor(ctx, m, c, unit, ctor, &fields, generic, is_enum);
        w.line(&format!("({cref}.{jsname} = {text}).prototype = {cref}.prototype;"));
    }
    if is_mixin {
        w.line(&format!("({cref}[dart.mixinNew] = function() {{}}).prototype = {cref}.prototype;"));
    }

    // Recursos rti: a própria classe e as interfaces implementadas (transitivas).
    let mut recipes: Vec<String> = vec![ctx.class_recipe(c)];
    let mut iseen: HashSet<u32> = HashSet::new();
    let mut iqueue: Vec<ClassId> = class.interface_classes.clone();
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
            let jsname = if ctor_name.is_empty() { "new".to_string() } else { ctor_name };
            let targs = if !ec.type_args.is_empty() || generic {
                let tys: Vec<Ty> = ec.type_args.iter().map(|t| e.resolve_type(*t)).collect();
                let t = Ty::Iface { class: c, args: tys, nullable: false };
                format!("{}, ", e.rti(&t))
            } else {
                String::new()
            };
            lazy.push(format!("get {}() {{ return dart.const(new {cref}.{jsname}({targs}{})); }}", js::prop_key(&static_member_name(&n)), args.join(", ")));
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
    if name.starts_with('_') {
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
    if class.kind == ClassKind::Enum {
        m.use_sdk("core");
        return "core._Enum".to_string();
    }
    let mut base = match class.supertype_class {
        Some(s) if Some(s) != ctx.object => {
            let tmp = FnEmitter::new(ctx, m, class.decl.map(|d| d.unit).unwrap_or(UnitId(0)), None, true);
            tmp.class_ref(s)
        }
        _ => {
            m.use_sdk("core");
            "core.Object".to_string()
        }
    };
    let mut base_class = class.supertype_class;
    for (i, &mx) in class.mixin_classes.iter().enumerate() {
        let tmp = FnEmitter::new(ctx, m, class.decl.map(|d| d.unit).unwrap_or(UnitId(0)), None, true);
        let mref = tmp.class_ref(mx);
        let app = format!("{}$mixin{}", ctx.class_name(c), i);
        w.line(&format!("const {app} = class {app} extends {base} {{}};"));
        // Construtores encaminhadores para a superclasse.
        let ctor_names: Vec<String> = match base_class {
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
        for n in ctor_names {
            if base == "core.Object" {
                w.line(&format!("({app}.{n} = function() {{}}).prototype = {app}.prototype;"));
            } else {
                w.line(&format!("({app}.{n} = function(...args) {{ {base}.{n}.apply(this, args); }}).prototype = {app}.prototype;"));
            }
        }
        w.line(&format!("dart.applyMixin({app}, {mref});"));
        base = app;
        let _ = &mut base_class;
        base_class = Some(mx);
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
    let sup = match class.supertype_class {
        Some(s) if Some(s) != ctx.object => s,
        _ => return,
    };
    let tmp = FnEmitter::new(ctx, m, class.decl.map(|d| d.unit).unwrap_or(UnitId(0)), None, true);
    let sref = tmp.class_ref(sup);
    let sgeneric = !ctx.class_params[sup.0 as usize].is_empty();
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
    if class.mixin_classes.is_empty() {
        sref.to_string()
    } else {
        format!("{}$mixin{}", ctx.class_name(c), class.mixin_classes.len() - 1)
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
    } else if generic {
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
        let tname = target.map(|n| ctx.name(n.sym).to_string()).unwrap_or("new".into());
        let tsym = target.map(|n| n.sym).or(ctx.empty_sym);
        let tfid = tsym.and_then(|s| class.constructors.get(&s).copied());
        let tsig = tfid.map(|f| ctx.fn_ty(f)).unwrap_or(Ty::Dynamic);
        let (arg_js, _, _) = e.emit_args_for(&tsig, args, None);
        let mut all: Vec<String> = vec!["this".into()];
        if is_enum {
            all.push("t$index".into());
            all.push("t$name".into());
        } else if generic {
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
    // Campos inicializados por `this.x` e pela lista.
    let mut skip: HashSet<String> = HashSet::new();
    for p in ctor.parameters.iter() {
        if p.this_ {
            if let Some(n) = p.name {
                skip.insert(ctx.name(n.sym).to_string());
            }
        }
    }
    for i in ctor.initializers.iter() {
        if let ast::Initializer::Field { name, .. } = i {
            skip.insert(ctx.name(name.sym).to_string());
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
                let sup = class.supertype_class;
                if let Some(s) = sup {
                    if Some(s) != ctx.object {
                        let sname = constructor.map(|n| ctx.name(n.sym).to_string()).unwrap_or("new".into());
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
                        } else if !ctx.class_params[s.0 as usize].is_empty() {
                            all.push("null".into());
                        }
                        all.extend(arg_js);
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
            super_call = Some("core._Enum.new.call(this, t$index, t$name);".into());
        } else if let Some(s) = class.supertype_class {
            if Some(s) != ctx.object {
                let sref = e.class_ref(s);
                let base = mixin_base_ref(ctx, c, &sref);
                let super_params: Vec<String> = ctor
                    .parameters
                    .iter()
                    .filter(|p| p.super_)
                    .filter_map(|p| p.name.map(|n| e.lookup_local(n.sym).map(|l| l.js.clone()).unwrap_or(js::ident(ctx.name(n.sym)))))
                    .collect();
                let mut all: Vec<String> = vec!["this".into()];
                if !ctx.class_params[s.0 as usize].is_empty() {
                    all.push("null".into());
                }
                all.extend(super_params);
                super_call = Some(format!("{base}.new.call({});", all.join(", ")));
            }
        }
    }
    if let Some(sc) = super_call {
        body.line(&sc);
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
    let name = ctor.name.map(|n| ctx.name(n.sym).to_string()).unwrap_or("new".into());
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
        // Redirecionamento resolvido nas chamadas; corpo vazio.
        cw.line(&format!("static {}({}) {{}}", js::prop_key(&name), params.join(", ")));
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
