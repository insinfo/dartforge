//! Emissor de corpos: escopos, tipos locais, ambiente rti e statements.

use crate::ctx::{Ctx, Member, MemberKind};
use crate::js::{self, Js, Writer, P_ASSIGN, P_COMMA, P_COND, P_PRIMARY, P_UNARY, P_YIELD};
use crate::module::ModState;
use crate::ty::{Ty, TyParam};
use dartforge_elements::model::{ClassId, Element, FunctionElementId, LibraryId, UnitId, VariableId};
use dartforge_frontend::ast::{self, Ast, ExprId, FunctionBody, StmtId, StmtKind};
use dartforge_intern::SymbolId;
use std::collections::HashMap;

/// Variável local em escopo.
#[derive(Clone, Debug)]
pub struct Local {
    pub js: String,
    pub ty: Ty,
    /// `late x = init`: inicializador avaliado na primeira leitura.
    pub lazy_init: Option<String>,
    /// `late x;` não anulável: leitura verifica inicialização.
    pub late_check: bool,
    /// `late final x;`: só uma atribuição.
    pub late_final: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AsyncKind {
    None,
    Async,
    AsyncStar,
    SyncStar,
}

/// Estado salvo ao entrar num corpo de função aninhado.
pub struct SavedFn {
    temps: Vec<String>,
    temp_counter: u32,
    async_kind: AsyncKind,
    ret_ty: Ty,
    returns: Vec<Ty>,
    w: Writer,
    label_counter: u32,
    fn_type_params: Vec<(u32, String)>,
    is_static: bool,
    switch_labels: Vec<Option<String>>,
}

pub struct FnEmitter<'m, 'a> {
    pub ctx: &'m Ctx<'a>,
    pub m: &'m ModState,
    pub unit: UnitId,
    pub lib: LibraryId,
    pub class: Option<ClassId>,
    pub is_static: bool,
    pub scopes: Vec<HashMap<SymbolId, Local>>,
    /// Parâmetros de tipo de função em escopo (mais interno primeiro): (id, nome JS).
    pub fn_type_params: Vec<(u32, String)>,
    pub temps: Vec<String>,
    pub temp_counter: u32,
    pub async_kind: AsyncKind,
    pub ret_ty: Ty,
    pub returns: Vec<Ty>,
    pub w: Writer,
    pub label_counter: u32,
    /// Pilha de alvos de cascata: (js, tipo).
    pub cascade: Vec<(String, Ty)>,
    /// Rótulos JS de `switch` para `continue` interno etc.
    pub switch_labels: Vec<Option<String>>,
    /// Rótulos Dart → JS.
    pub labels: Vec<(String, String)>,
    /// Em modo assinatura (`_ti` como ambiente de classe).
    pub sig_mode: bool,
    pub unique: u32,
    pub in_constructor: bool,
    pub factory_ti: bool,
    pub is_closure_body: bool,
    pub rethrow_var: Vec<String>,
    pub pending_promotions: Vec<(SymbolId, Ty)>,
    pub pending_prefix: Vec<String>,
    pub extension_this: Option<Ty>,
    pub current_extension: Option<dartforge_elements::model::ExtensionId>,
    /// Dentro de uma expressão constante (instanciações implícitas viram `dart.const`).
    pub in_const: bool,
    /// Rótulos de `case` alcançáveis por `continue`: (nome Dart, rótulo JS do laço, valor da variável de estado).
    pub case_labels: Vec<(String, String, String)>,
}

impl<'m, 'a> FnEmitter<'m, 'a> {
    pub fn new(ctx: &'m Ctx<'a>, m: &'m ModState, unit: UnitId, class: Option<ClassId>, is_static: bool) -> Self {
        let lib = ctx.program.unit(unit).library;
        FnEmitter {
            ctx,
            m,
            unit,
            lib,
            class,
            is_static,
            scopes: vec![HashMap::new()],
            fn_type_params: Vec::new(),
            temps: Vec::new(),
            temp_counter: 0,
            async_kind: AsyncKind::None,
            ret_ty: Ty::Dynamic,
            returns: Vec::new(),
            w: Writer::default(),
            label_counter: 0,
            cascade: Vec::new(),
            switch_labels: Vec::new(),
            labels: Vec::new(),
            sig_mode: false,
            unique: 0,
            in_constructor: false,
            factory_ti: false,
            is_closure_body: false,
            rethrow_var: Vec::new(),
            pending_promotions: Vec::new(),
            pending_prefix: Vec::new(),
            extension_this: None,
            current_extension: None,
            in_const: false,
            case_labels: Vec::new(),
        }
    }

    pub fn ast(&self) -> &'a Ast {
        &self.ctx.program.unit(self.unit).ast
    }
    pub fn source(&self) -> &'a str {
        &self.ctx.program.unit(self.unit).source
    }
    pub fn expr(&self, e: ExprId) -> &'a ast::Expr {
        self.ast().expr(e)
    }
    pub fn name(&self, s: SymbolId) -> &'a str {
        self.ctx.interner.resolve(s)
    }
    pub fn text(&self, span: dartforge_diagnostics::Span) -> &'a str {
        &self.source()[span.start..span.end]
    }

    // -----------------------------------------------------------------------
    // Escopos e temporários
    // -----------------------------------------------------------------------

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }
    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }
    /// Declara uma local e devolve o nome JS (com renomeação de sombreamento
    /// entre escopos irmãos evitada: JS `let` tem escopo de bloco também).
    pub fn declare(&mut self, sym: SymbolId, ty: Ty) -> String {
        let base = js::ident(self.name(sym));
        let js = base;
        self.scopes.last_mut().expect("escopo").insert(sym, Local { js: js.clone(), ty, lazy_init: None, late_check: false, late_final: false });
        js
    }
    pub fn declare_js(&mut self, sym: SymbolId, js: String, ty: Ty) {
        self.scopes.last_mut().expect("escopo").insert(sym, Local { js, ty, lazy_init: None, late_check: false, late_final: false });
    }
    pub fn lookup_local(&self, sym: SymbolId) -> Option<&Local> {
        for s in self.scopes.iter().rev() {
            if let Some(l) = s.get(&sym) {
                return Some(l);
            }
        }
        None
    }
    pub fn set_local_ty(&mut self, sym: SymbolId, ty: Ty) {
        for s in self.scopes.iter_mut().rev() {
            if let Some(l) = s.get_mut(&sym) {
                l.ty = ty;
                return;
            }
        }
    }
    pub fn temp(&mut self) -> String {
        let n = format!("t${}", self.temp_counter);
        self.temp_counter += 1;
        self.temps.push(n.clone());
        n
    }
    pub fn fresh_label(&mut self) -> String {
        let n = format!("L{}", self.label_counter);
        self.label_counter += 1;
        n
    }

    /// Entra num corpo de função aninhado; devolve o estado a restaurar.
    pub fn enter_fn(&mut self, async_kind: AsyncKind, ret_ty: Ty, is_static: bool) -> SavedFn {
        let saved = SavedFn {
            temps: std::mem::take(&mut self.temps),
            temp_counter: self.temp_counter,
            async_kind: self.async_kind,
            ret_ty: std::mem::replace(&mut self.ret_ty, ret_ty),
            returns: std::mem::take(&mut self.returns),
            w: std::mem::take(&mut self.w),
            label_counter: self.label_counter,
            fn_type_params: self.fn_type_params.clone(),
            is_static: self.is_static,
            switch_labels: std::mem::take(&mut self.switch_labels),
        };
        self.async_kind = async_kind;
        self.is_static = is_static;
        self.push_scope();
        saved
    }
    /// Sai do corpo aninhado devolvendo (texto do corpo com temps declarados, tipos retornados).
    pub fn exit_fn(&mut self, saved: SavedFn) -> (String, Vec<Ty>) {
        self.pop_scope();
        let mut body = String::new();
        if !self.temps.is_empty() {
            body.push_str(&format!("let {};\n", self.temps.join(", ")));
        }
        body.push_str(&self.w.out);
        let returns = std::mem::replace(&mut self.returns, saved.returns);
        self.temps = saved.temps;
        self.temp_counter = saved.temp_counter;
        self.async_kind = saved.async_kind;
        self.ret_ty = saved.ret_ty;
        self.w = saved.w;
        self.label_counter = saved.label_counter;
        self.fn_type_params = saved.fn_type_params;
        self.is_static = saved.is_static;
        self.switch_labels = saved.switch_labels;
        (body, returns)
    }

    /// Indenta um bloco de texto já pronto dentro do escritor atual.
    pub fn write_block_text(&mut self, text: &str) {
        for line in text.lines() {
            self.w.line(line);
        }
    }

    // -----------------------------------------------------------------------
    // Referências a bibliotecas e elementos
    // -----------------------------------------------------------------------

    /// Variável JS do namespace de uma biblioteca, registrando a importação.
    pub fn lib_var(&self, lib: LibraryId) -> String {
        self.m.lib_var(self.ctx, lib)
    }

    pub fn dartx(&self, name: &str) -> String {
        self.m.dartx(name)
    }

    /// Símbolo privado `dart.privateName(lib, name)`.
    pub fn private_sym(&self, lib: LibraryId, name: &str) -> String {
        self.m.private_sym(self.ctx, lib, name)
    }

    /// Nome de propriedade JS de um membro de instância (getter/método/campo).
    /// Devolve `.nome`, `[$nome]`, `[_privado]` ou `['+']`.
    pub fn member_access(&self, recv_ty: &Ty, name: &str, setter: bool) -> String {
        if name.starts_with('_') {
            // Privado: escopo da biblioteca que o declara.
            let lib = self
                .ctx
                .lookup_member(recv_ty, name, setter)
                .map(|m| self.ctx.lib_of_class(m.class))
                .unwrap_or(self.lib);
            return format!("[{}]", self.private_sym(lib, name));
        }
        let js_name = js_member_name(name);
        if self.ctx.is_ext_member(recv_ty, name, setter) {
            return format!("[{}]", self.dartx(&js_name));
        }
        js::prop_access(&js_name)
    }

    /// Acesso a membro pelo nome JS de declaração (dentro da própria classe).
    pub fn decl_member_key(&self, class: ClassId, name: &str) -> String {
        if name.starts_with('_') {
            let lib = self.ctx.lib_of_class(class);
            return format!("[{}]", self.private_sym(lib, name));
        }
        js::prop_key(&js_member_name(name))
    }

    /// Referência JS a uma classe.
    pub fn class_ref(&self, c: ClassId) -> String {
        let lib = self.ctx.lib_of_class(c);
        format!("{}.{}", self.lib_var(lib), self.ctx.class_name(c))
    }

    /// Referência a elemento de topo.
    pub fn element_ref(&self, e: Element) -> Option<(String, Ty)> {
        match e {
            Element::Class(c) => Some((self.class_ref(c), self.ctx.t_type())),
            Element::Function(fid) => {
                let f = self.ctx.program.function(fid);
                let lib = f.library;
                let n = self.name(f.name);
                let js = format!("{}{}", self.lib_var(lib), js::prop_access(&top_level_name(n)));
                let ty = self.ctx.fn_ty(fid);
                Some((js, ty))
            }
            Element::Variable(vid) => {
                let v = self.ctx.program.variable(vid);
                let n = self.name(v.name);
                let js = format!("{}{}", self.lib_var(v.library), js::prop_access(&top_level_name(n)));
                Some((js, self.ctx.var_ty(vid)))
            }
            Element::Typedef(_) | Element::Extension(_) | Element::Prefix(..) => None,
        }
    }

    // -----------------------------------------------------------------------
    // Tipos: resolução de anotações e ambiente rti
    // -----------------------------------------------------------------------

    /// Parâmetros de tipo em escopo por nome (classe + funções).
    pub fn type_param_by_name(&self, name: &str) -> Option<Ty> {
        for (id, js) in &self.fn_type_params {
            let pname = self.ctx.param_names.borrow().get(id).cloned().unwrap_or_default();
            if pname == name {
                let _ = js;
                return Some(Ty::Param { id: *id, name: name.to_string(), nullable: false });
            }
        }
        if let Some(c) = self.class {
            for p in &self.ctx.class_params[c.0 as usize] {
                if p.name == name {
                    return Some(Ty::Param { id: p.id, name: name.to_string(), nullable: false });
                }
            }
        }
        None
    }

    /// Resolve uma anotação de tipo da AST para `Ty`.
    pub fn resolve_type(&self, t: ast::TypeId) -> Ty {
        let node = self.ast().ty(t);
        let nullable = node.nullable;
        match &node.kind {
            ast::TypeKind::Void => Ty::Void,
            ast::TypeKind::Named { name, args } => {
                let args: Vec<Ty> = args.iter().map(|a| self.resolve_type(*a)).collect();
                let (lib, sym) = if name.len() == 2 {
                    let prefix = name[0].sym;
                    (Some(prefix), name[1].sym)
                } else {
                    (None, name[0].sym)
                };
                let n = self.name(sym);
                if lib.is_none() {
                    match n {
                        "dynamic" => return Ty::Dynamic,
                        "void" => return Ty::Void,
                        "Never" => return if nullable { Ty::Null } else { Ty::Never },
                        "Null" => return Ty::Null,
                        _ => {}
                    }
                    if let Some(p) = self.type_param_by_name(n) {
                        return p.with_nullable(nullable);
                    }
                }
                let binding = match lib {
                    Some(p) => self.ctx.program.lookup_prefixed(self.lib, p, sym),
                    None => self.ctx.program.lookup(self.lib, sym),
                };
                match binding.and_then(|b| b.getter) {
                    Some(Element::Class(c)) => {
                        let class = self.ctx.program.class(c);
                        if class.kind == dartforge_elements::model::ClassKind::ExtensionType {
                            if let Some(rep) = class.representation {
                                let mut map = HashMap::new();
                                for (p, a) in self.ctx.class_params[c.0 as usize].iter().zip(args.iter()) {
                                    map.insert(p.id, a.clone());
                                }
                                let t = self.ctx.var_ty(rep).subst(&map);
                                return if nullable { t.with_nullable(true) } else { t };
                            }
                        }
                        if Some(c) == self.ctx.future_or {
                            let arg = args.into_iter().next().unwrap_or(Ty::Dynamic);
                            return Ty::FutureOr { arg: Box::new(arg), nullable };
                        }
                        let nparams = self.ctx.class_params[c.0 as usize].len();
                        let mut args = args;
                        while args.len() < nparams {
                            let p = &self.ctx.class_params[c.0 as usize][args.len()];
                            args.push(if p.bound.is_dynamic() { Ty::Dynamic } else { (*p.bound).clone() });
                        }
                        Ty::Iface { class: c, args, nullable }
                    }
                    Some(Element::Typedef(tid)) => {
                        let data = &self.ctx.outline.typedefs[tid.0 as usize];
                        let mut map = HashMap::new();
                        for (p, a) in data.type_params.iter().zip(args.iter()) {
                            map.insert(p.0, a.clone());
                        }
                        let t = self.ctx.ty_of(data.target_type).subst(&map);
                        if nullable { t.with_nullable(true) } else { t }
                    }
                    _ => Ty::Dynamic,
                }
            }
            ast::TypeKind::Function { return_type, type_params, parameters } => {
                // Parâmetros de tipo do tipo de função: ids novos.
                let mut tps = Vec::new();
                let mut scoped = self.fn_type_params.clone();
                let saved_len = scoped.len();
                let _ = saved_len;
                let mut me_params: Vec<(u32, String)> = Vec::new();
                for tp in type_params.iter() {
                    let p = self.ctx.fresh_param(self.name(tp.name.sym), Ty::Dynamic);
                    me_params.push((p.id, p.name.clone()));
                    tps.push(p);
                }
                // Resolve bounds e assinatura com os parâmetros em escopo.
                let this_ptr: *const FnEmitter<'m, 'a> = self;
                let _ = this_ptr;
                let sub = self.with_fn_params(&me_params);
                for (tp, p) in type_params.iter().zip(tps.iter_mut()) {
                    if let Some(b) = tp.bound {
                        let bt = sub.resolve_type(b);
                        self.ctx.param_bounds.borrow_mut().insert(p.id, bt.clone());
                        p.bound = Box::new(bt);
                    } else {
                        p.bound = Box::new(self.ctx.t_object_q());
                        self.ctx.param_bounds.borrow_mut().insert(p.id, self.ctx.t_object_q());
                    }
                }
                let ret = return_type.map(|r| sub.resolve_type(r)).unwrap_or(Ty::Dynamic);
                let (pos, opt, named) = sub.resolve_params(parameters);
                scoped.truncate(saved_len);
                Ty::Fn { type_params: tps, ret: Box::new(ret), pos, opt, named, nullable }
            }
            ast::TypeKind::Record { positional, named } => {
                let pos = positional.iter().map(|t| self.resolve_type(*t)).collect();
                let mut named: Vec<(String, Ty)> =
                    named.iter().map(|(n, t)| (self.name(n.sym).to_string(), self.resolve_type(*t))).collect();
                named.sort_by(|a, b| a.0.cmp(&b.0));
                Ty::Record { pos, named, nullable }
            }
        }
    }

    /// Cópia leve do emissor com parâmetros de tipo extra em escopo (só para resolver tipos).
    fn with_fn_params(&self, extra: &[(u32, String)]) -> FnEmitter<'m, 'a> {
        let mut e = FnEmitter::new(self.ctx, self.m, self.unit, self.class, self.is_static);
        e.fn_type_params = extra.to_vec();
        e.fn_type_params.extend(self.fn_type_params.iter().cloned());
        e.scopes = self.scopes.clone();
        e
    }

    pub fn resolve_params(&self, params: &[ast::Parameter]) -> (Vec<Ty>, Vec<Ty>, Vec<(String, Ty, bool)>) {
        let mut pos = Vec::new();
        let mut opt = Vec::new();
        let mut named = Vec::new();
        for p in params {
            let t = self.param_ty(p);
            match p.kind {
                ast::ParameterKind::Required => pos.push(t),
                ast::ParameterKind::Optional => opt.push(t),
                ast::ParameterKind::Named => {
                    named.push((p.name.map(|n| self.name(n.sym).to_string()).unwrap_or_default(), t, p.required))
                }
            }
        }
        named.sort_by(|a, b| a.0.cmp(&b.0));
        (pos, opt, named)
    }

    /// Tipo de um parâmetro formal (anotação, forma antiga de função, ou dynamic).
    pub fn param_ty(&self, p: &ast::Parameter) -> Ty {
        if let Some(fp) = &p.function_parameters {
            let ret = p.ty.map(|t| self.resolve_type(t)).unwrap_or(Ty::Dynamic);
            let (pos, opt, named) = self.resolve_params(fp);
            return Ty::Fn { type_params: vec![], ret: Box::new(ret), pos, opt, named, nullable: false };
        }
        match p.ty {
            Some(t) => self.resolve_type(t),
            None => Ty::Dynamic,
        }
    }

    // -----------------------------------------------------------------------
    // Receitas rti
    // -----------------------------------------------------------------------

    /// Receita de um tipo; `unbound` são parâmetros de tipos de função genéricos
    /// aninhados (`0^`), `used_fn` acumula parâmetros de função usados,
    /// `used_class` marca uso de parâmetros da classe.
    fn recipe(&self, t: &Ty, unbound: &mut Vec<u32>, used_fn: &mut Vec<u32>, used_class: &mut bool) -> String {
        match t {
            Ty::Dynamic => "@".into(),
            Ty::Void => "~".into(),
            Ty::Never => "0&".into(),
            Ty::Null => match self.ctx.null_ {
                Some(n) => self.ctx.class_recipe(n),
                None => "core|Null".into(),
            },
            Ty::Iface { class, args, nullable } => {
                let mut s = self.ctx.class_recipe(*class);
                if !args.is_empty() {
                    s.push('<');
                    let parts: Vec<String> = args.iter().map(|a| self.recipe(a, unbound, used_fn, used_class)).collect();
                    s.push_str(&parts.join(","));
                    s.push('>');
                }
                if *nullable {
                    s.push('?');
                }
                s
            }
            Ty::FutureOr { arg, nullable } => {
                let mut s = self.recipe(arg, unbound, used_fn, used_class);
                s.push('/');
                if *nullable {
                    s.push('?');
                }
                s
            }
            Ty::Param { id, nullable, name } => {
                let q = if *nullable { "?" } else { "" };
                if let Some(i) = unbound.iter().position(|x| x == id) {
                    return format!("{i}^{q}");
                }
                if let Some(c) = self.class {
                    if self.ctx.class_params[c.0 as usize].iter().any(|p| p.id == *id) {
                        *used_class = true;
                        return format!("{}.{}{}", self.ctx.class_name(c), name, q);
                    }
                }
                if self.fn_type_params.iter().any(|(pid, _)| pid == id) {
                    if !used_fn.contains(id) {
                        used_fn.push(*id);
                    }
                    // Índice preenchido depois: marcador.
                    return format!("\u{1}{id}\u{2}{q}");
                }
                // Parâmetro fora de escopo: usa o bound.
                let b = self.ctx.bound_of(*id);
                if matches!(b, Ty::Param { .. }) {
                    return format!("@{}", "");
                }
                let bt = if *nullable { b.with_nullable(true) } else { b };
                self.recipe(&bt, unbound, used_fn, used_class)
            }
            Ty::Fn { type_params, ret, pos, opt, named, nullable } => {
                let saved = unbound.clone();
                let mut new_unbound: Vec<u32> = type_params.iter().map(|p| p.id).collect();
                new_unbound.extend(saved.iter().cloned());
                *unbound = new_unbound;
                let mut s = self.recipe(ret, unbound, used_fn, used_class);
                s.push('(');
                let mut parts: Vec<String> = pos.iter().map(|a| self.recipe(a, unbound, used_fn, used_class)).collect();
                if !opt.is_empty() {
                    let o: Vec<String> = opt.iter().map(|a| self.recipe(a, unbound, used_fn, used_class)).collect();
                    parts.push(format!("[{}]", o.join(",")));
                }
                if !named.is_empty() {
                    let n: Vec<String> = named
                        .iter()
                        .map(|(n, t, r)| format!("{n}{}{}", if *r { "!" } else { ":" }, self.recipe(t, unbound, used_fn, used_class)))
                        .collect();
                    parts.push(format!("{{{}}}", n.join(",")));
                }
                s.push_str(&parts.join(","));
                s.push(')');
                if !type_params.is_empty() {
                    s.push('<');
                    let b: Vec<String> = type_params.iter().map(|p| self.recipe(&p.bound, unbound, used_fn, used_class)).collect();
                    s.push_str(&b.join(","));
                    s.push('>');
                }
                *unbound = saved;
                if *nullable {
                    s.push('?');
                }
                s
            }
            Ty::Record { pos, named, nullable } => {
                let mut s = String::from("+");
                let names: Vec<&str> = named.iter().map(|(n, _)| n.as_str()).collect();
                s.push_str(&names.join(","));
                s.push('(');
                let mut parts: Vec<String> = pos.iter().map(|a| self.recipe(a, unbound, used_fn, used_class)).collect();
                parts.extend(named.iter().map(|(_, t)| self.recipe(t, unbound, used_fn, used_class)));
                s.push_str(&parts.join(","));
                s.push(')');
                if *nullable {
                    s.push('?');
                }
                s
            }
        }
    }

    /// Expressão JS que avalia para o rti de `t` no ambiente atual.
    pub fn rti(&self, t: &Ty) -> String {
        let mut unbound = Vec::new();
        let mut used_fn = Vec::new();
        let mut used_class = false;
        let raw = self.recipe(t, &mut unbound, &mut used_fn, &mut used_class);
        self.register_recipe_classes(t);
        // Ordena os parâmetros de função usados na ordem de escopo (mais interno primeiro).
        let mut ordered: Vec<(u32, String)> = Vec::new();
        for (id, jsn) in &self.fn_type_params {
            if used_fn.contains(id) {
                ordered.push((*id, jsn.clone()));
            }
        }
        let class_env = if used_class {
            Some(if self.sig_mode || self.factory_ti { "_ti".to_string() } else { "dart_rti.instanceType(this)".to_string() })
        } else {
            None
        };
        let mut recipe = raw;
        if class_env.is_none() && ordered.len() == 1 {
            let (id, jsn) = &ordered[0];
            recipe = recipe.replace(&format!("\u{1}{id}\u{2}"), "0");
            if recipe == "0" {
                return jsn.clone();
            }
            return format!("{jsn}[_eval]({})", js::string_literal(&recipe));
        }
        for (i, (id, _)) in ordered.iter().enumerate() {
            recipe = recipe.replace(&format!("\u{1}{id}\u{2}"), &(i + 1).to_string());
        }
        if class_env.is_none() && ordered.is_empty() {
            return format!("dart_rti._Universe.eval(dart_rti._theUniverse(), {}, true)", js::string_literal(&recipe));
        }
        let mut env = match class_env {
            Some(e) => e,
            None => "dart_rti._Universe.eval(dart_rti._theUniverse(), \"@\", true)".to_string(),
        };
        for (_, jsn) in &ordered {
            env = format!("{env}[_bind]({jsn})");
        }
        format!("{env}[_eval]({})", js::string_literal(&recipe))
    }

    /// Receita "crua" em modo classe (parâmetros por índice 1..n), para as regras.
    pub fn register_recipe_classes(&self, t: &Ty) {
        match t {
            Ty::Iface { class, args, .. } => {
                self.m.note_class(*class);
                for a in args {
                    self.register_recipe_classes(a);
                }
            }
            Ty::Fn { ret, pos, opt, named, type_params, .. } => {
                self.register_recipe_classes(ret);
                pos.iter().chain(opt.iter()).for_each(|a| self.register_recipe_classes(a));
                named.iter().for_each(|(_, t, _)| self.register_recipe_classes(t));
                type_params.iter().for_each(|p| self.register_recipe_classes(&p.bound));
            }
            Ty::Record { pos, named, .. } => {
                pos.iter().for_each(|a| self.register_recipe_classes(a));
                named.iter().for_each(|(_, t)| self.register_recipe_classes(t));
            }
            Ty::FutureOr { arg, .. } => self.register_recipe_classes(arg),
            _ => {}
        }
    }

    /// `T[_is](x)` / `typeof x == 'string'`.
    pub fn is_test(&self, value: &Js, t: &Ty) -> Js {
        if let Ty::Iface { class, args, nullable: false } = t {
            if args.is_empty() {
                if Some(*class) == self.ctx.string_ {
                    return Js::new(format!("typeof {} == 'string'", value.at(P_UNARY + 1)), crate::js::P_EQ);
                }
                if Some(*class) == self.ctx.bool_ {
                    return Js::new(format!("typeof {} == 'boolean'", value.at(P_UNARY + 1)), crate::js::P_EQ);
                }
            }
        }
        if let Ty::Iface { class, nullable: true, .. } = t {
            if Some(*class) == self.ctx.object {
                return Js::prim("true");
            }
        }
        if matches!(t, Ty::Dynamic | Ty::Void) {
            return Js::prim("true");
        }
        Js::prim(format!("{}[_is]({})", self.rti(t), value.code))
    }

    pub fn as_cast(&self, value: &Js, t: &Ty) -> Js {
        if matches!(t, Ty::Dynamic | Ty::Void) {
            return value.clone();
        }
        if let Ty::Iface { class, nullable: true, .. } = t {
            if Some(*class) == self.ctx.object {
                return value.clone();
            }
        }
        Js::prim(format!("{}[_as]({})", self.rti(t), value.code))
    }

    // -----------------------------------------------------------------------
    // Corpo de função
    // -----------------------------------------------------------------------

    /// Declara os parâmetros de uma função no escopo atual e devolve a lista
    /// de parâmetros JS (`a, b = 1, opts`) e o prólogo (nomeados).
    pub fn declare_params(&mut self, params: &[ast::Parameter], tys: Option<&Ty>) -> (String, String) {
        let mut js_params: Vec<String> = Vec::new();
        let mut prologue = String::new();
        let mut has_named = false;
        let (pos_expected, opt_expected, named_expected): (Vec<Ty>, Vec<Ty>, Vec<(String, Ty, bool)>) = match tys {
            Some(Ty::Fn { pos, opt, named, .. }) => (pos.clone(), opt.clone(), named.clone()),
            _ => (vec![], vec![], vec![]),
        };
        let mut pi = 0;
        let mut oi = 0;
        for p in params {
            let Some(name) = p.name else { continue };
            let mut ty = if p.ty.is_none() && p.function_parameters.is_none() {
                match p.kind {
                    ast::ParameterKind::Required => pos_expected.get(pi).cloned().unwrap_or(Ty::Dynamic),
                    ast::ParameterKind::Optional => opt_expected.get(oi).cloned().unwrap_or(Ty::Dynamic),
                    ast::ParameterKind::Named => named_expected
                        .iter()
                        .find(|(n, _, _)| n == self.name(name.sym))
                        .map(|(_, t, _)| t.clone())
                        .unwrap_or(Ty::Dynamic),
                }
            } else {
                self.param_ty(p)
            };
            if p.this_ {
                // `this.x`: tipo do campo.
                if let Some(c) = self.class {
                    if let Some(m) = self.ctx.lookup_member(&self.ctx.this_ty(c), self.name(name.sym), false) {
                        if p.ty.is_none() {
                            ty = self.ctx.member_ty(&m);
                        }
                    }
                }
            }
            match p.kind {
                ast::ParameterKind::Required => {
                    pi += 1;
                    let jsn = self.declare(name.sym, ty);
                    js_params.push(jsn);
                }
                ast::ParameterKind::Optional => {
                    oi += 1;
                    let jsn = self.declare(name.sym, ty.clone());
                    let saved_const = self.in_const;
                    self.in_const = true;
                    let def = match p.default_value {
                        Some(d) => self.emit_expr(d, Some(&ty)).0.at(P_ASSIGN + 1),
                        None => "null".to_string(),
                    };
                    self.in_const = saved_const;
                    js_params.push(format!("{jsn} = {def}"));
                }
                ast::ParameterKind::Named => {
                    has_named = true;
                    let jsn = self.declare(name.sym, ty.clone());
                    let saved_const = self.in_const;
                    self.in_const = true;
                    let def = match p.default_value {
                        Some(d) => self.emit_expr(d, Some(&ty)).0.at(P_COND + 1),
                        None => "null".to_string(),
                    };
                    self.in_const = saved_const;
                    let key = self.name(name.sym);
                    prologue.push_str(&format!(
                        "let {jsn} = opts && {} in opts ? opts{} : {def};\n",
                        js::string_literal(key),
                        js::prop_access(key)
                    ));
                }
            }
        }
        if has_named {
            js_params.push("opts".to_string());
        }
        (js_params.join(", "), prologue)
    }

    /// Emite um corpo de função (bloco ou expressão) no escritor atual.
    pub fn emit_body(&mut self, body: &FunctionBody) {
        match body {
            FunctionBody::Block(s) => {
                let stmt = self.ast().stmt(*s);
                if let StmtKind::Block(stmts) = &stmt.kind {
                    self.hoist_locals(stmts);
                    for &st in stmts.iter() {
                        self.emit_stmt(st);
                    }
                } else {
                    self.emit_stmt(*s);
                }
            }
            FunctionBody::Expression(e) => {
                let expected = self.ret_ty.clone();
                let (js, ty) = self.emit_expr(*e, Some(&expected));
                self.returns.push(ty);
                if matches!(self.async_kind, AsyncKind::None | AsyncKind::Async) {
                    if self.ret_ty == Ty::Void && !self.is_closure_body {
                        self.w.line(&format!("{};", js.code));
                    } else {
                        self.w.line(&format!("return {};", js.code));
                    }
                } else {
                    self.w.line(&format!("{};", js.code));
                }
            }
            FunctionBody::Empty | FunctionBody::Native(_) => {}
        }
    }

    /// Declara antecipadamente as funções locais do bloco (para recursão mútua).
    pub fn hoist_locals(&mut self, _stmts: &[StmtId]) {}

    /// Constrói o texto completo de uma função: `head` é o cabeçalho
    /// (`function(a) {`, `(a) => {`, `m(a) {`); aplica a transformação async/gerador.
    pub fn wrap_async_head(&self, kind: AsyncKind, head: &str, prologue: &str, body: &str, ret_ty: &Ty) -> String {
        let inner_ty = match ret_ty {
            Ty::Iface { args, .. } if !args.is_empty() => args[0].clone(),
            Ty::FutureOr { arg, .. } => (**arg).clone(),
            _ => Ty::Dynamic,
        };
        match kind {
            AsyncKind::None => format!("{head}\n{prologue}{body}}}"),
            AsyncKind::Async => {
                self.m.use_sdk("async");
                let rti = self.rti(&inner_ty);
                format!(
                    "{head}\n{prologue}let t$completer = async._makeAsyncAwaitCompleter({rti});\n\
let t$gen = (function*() {{\n{body}}}).call(this);\n\
let t$body = async._wrapJsFunctionForAsync((t$code, t$res) => {{\n\
  let t$r;\n\
  try {{ t$r = t$code === 1 ? t$gen.throw(t$res) : t$gen.next(t$res); }}\n\
  catch (t$e) {{ return async._asyncRethrow(t$e, t$completer); }}\n\
  if (t$r.done) return async._asyncReturn(t$r.value, t$completer);\n\
  return async._asyncAwait(t$r.value, t$body, t$completer);\n\
}});\n\
return async._asyncStartSync(t$body, t$completer);\n}}"
                )
            }
            AsyncKind::AsyncStar => {
                self.m.use_sdk("async");
                let rti = self.rti(&inner_ty);
                format!(
                    "{head}\n{prologue}let t$gen = (function*() {{\n{body}}}).call(this);\n\
let t$controller;\n\
let t$body = async._wrapJsFunctionForAsync((t$code, t$res) => {{\n\
  let t$r;\n\
  try {{ t$r = t$code === 2 ? t$gen.return(void 0) : t$code === 1 ? t$gen.throw(t$res) : t$gen.next(t$res); }}\n\
  catch (t$e) {{ return async._asyncStarHelper(t$e, 1, t$controller); }}\n\
  if (t$r.done) return async._asyncStarHelper(null, 0, t$controller);\n\
  return async._asyncStarHelper(t$r.value, t$body, t$controller);\n\
}});\n\
t$controller = async._makeAsyncStarStreamController({rti}, t$body);\n\
return async._streamOfController(t$controller);\n}}"
                )
            }
            AsyncKind::SyncStar => {
                self.m.use_sdk("async");
                let rti = self.rti(&inner_ty);
                format!(
                    "{head}\n{prologue}let t$self = this;\n\
return async._makeSyncStarIterable({rti}, () => {{\n\
  let t$gen = (function*() {{\n{body}}}).call(t$self);\n\
  return (t$it, t$code, t$err) => {{\n\
    let t$r;\n\
    try {{ t$r = t$code === 1 ? t$gen.throw(t$err) : t$gen.next(); }}\n\
    catch (t$e) {{ t$it[_datum] = t$e; return 3; }}\n\
    if (t$r.done) return 0;\n\
    if (t$r.value instanceof async._IterationMarker) return t$it[_yieldStar](t$r.value.value);\n\
    t$it[_current] = t$r.value;\n\
    return 1;\n\
  }};\n\
}});\n}}"
                )
            }
        }
    }

    // -----------------------------------------------------------------------
    // Statements
    // -----------------------------------------------------------------------

    pub fn emit_stmt(&mut self, s: StmtId) {
        let stmt = self.ast().stmt(s);
        match &stmt.kind {
            StmtKind::Block(stmts) => {
                self.w.open("{");
                self.push_scope();
                for &st in stmts.iter() {
                    self.emit_stmt(st);
                }
                self.pop_scope();
                self.w.close("}");
            }
            StmtKind::Variables(list) => self.emit_var_list(list),
            StmtKind::PatternVariables { pattern, value, .. } => {
                let (vjs, vty) = self.emit_expr(*value, None);
                let t = self.temp();
                self.w.line(&format!("{t} = {};", vjs.code));
                let mut binds = Vec::new();
                let cond = self.pattern_cond(*pattern, &t, &vty, &mut binds, true);
                for (sym, ty) in &binds {
                    let jsn = self.declare(*sym, ty.clone());
                    self.w.line(&format!("let {jsn} = null;"));
                }
                if cond != "true" {
                    self.w.line(&format!(
                        "if (!({cond})) dart.throw(new core.StateError.new(\"Pattern matching error\"));"
                    ));
                    self.m.use_sdk("core");
                }
            }
            StmtKind::Function(fid) => self.emit_local_function(*fid),
            StmtKind::Expression(e) => {
                let (js, _) = self.emit_expr(*e, None);
                self.w.line(&format!("{};", js.code));
            }
            StmtKind::If { condition, case_pattern, guard, then, else_ } => {
                if let Some(pat) = case_pattern {
                    let (vjs, vty) = self.emit_expr(*condition, None);
                    let t = self.temp();
                    self.w.line(&format!("{t} = {};", vjs.code));
                    self.w.open("{");
                    self.push_scope();
                    let mut binds = Vec::new();
                    let cond = self.pattern_cond(*pat, &t, &vty, &mut binds, false);
                    for (sym, ty) in &binds {
                        let jsn = self.declare(*sym, ty.clone());
                        self.w.line(&format!("let {jsn} = null;"));
                    }
                    let mut full = cond;
                    if let Some(g) = guard {
                        let (gjs, _) = self.emit_expr(*g, Some(&self.ctx.t_bool()));
                        full = format!("{} && {}", paren_if_needed(&full), gjs.at(crate::js::P_AND));
                    }
                    self.w.open(&format!("if ({full}) {{"));
                    self.emit_stmt(*then);
                    self.w.close("}");
                    if let Some(e) = else_ {
                        self.w.open("else {");
                        self.emit_stmt(*e);
                        self.w.close("}");
                    }
                    self.pop_scope();
                    self.w.close("}");
                    return;
                }
                let (cjs, _) = self.emit_cond(*condition);
                self.w.open(&format!("if ({}) {{", cjs));
                self.push_scope();
                self.emit_stmt(*then);
                self.pop_scope();
                self.w.close("}");
                if let Some(e) = else_ {
                    self.w.open("else {");
                    self.push_scope();
                    self.emit_stmt(*e);
                    self.pop_scope();
                    self.w.close("}");
                }
            }
            StmtKind::While { condition, body } => {
                let (cjs, _) = self.emit_cond(*condition);
                self.w.open(&format!("while ({cjs}) {{"));
                self.push_scope();
                self.emit_stmt(*body);
                self.pop_scope();
                self.w.close("}");
            }
            StmtKind::DoWhile { body, condition } => {
                self.w.open("do {");
                self.push_scope();
                self.emit_stmt(*body);
                self.pop_scope();
                let (cjs, _) = self.emit_cond(*condition);
                self.w.close(&format!("}} while ({cjs});"));
            }
            StmtKind::For { init, condition, updates, body, .. } => {
                self.push_scope();
                let init_js = match init {
                    Some(ast::ForInit::Variables(list)) => {
                        let mut parts = Vec::new();
                        let declared = list.ty.map(|t| self.resolve_type(t));
                        for v in list.variables.iter() {
                            let (ijs, ity) = match v.initializer {
                                Some(e) => {
                                    let (j, t) = self.emit_expr(e, declared.as_ref());
                                    (Some(j), t)
                                }
                                None => (None, Ty::Null),
                            };
                            let ty = declared.clone().unwrap_or(ity);
                            let jsn = self.declare(v.name.sym, ty);
                            match ijs {
                                Some(j) => parts.push(format!("{jsn} = {}", j.at(P_ASSIGN + 1))),
                                None => parts.push(jsn),
                            }
                        }
                        format!("let {}", parts.join(", "))
                    }
                    Some(ast::ForInit::Expression(e)) => self.emit_expr(*e, None).0.code,
                    None => String::new(),
                };
                let cond_js = match condition {
                    Some(c) => self.emit_cond(*c).0,
                    None => String::new(),
                };
                let upd: Vec<String> = updates.iter().map(|u| self.emit_expr(*u, None).0.code).collect();
                self.w.open(&format!("for ({init_js}; {cond_js}; {}) {{", upd.join(", ")));
                self.push_scope();
                self.emit_stmt(*body);
                self.pop_scope();
                self.w.close("}");
                self.pop_scope();
            }
            StmtKind::ForIn { await_, target, iterable, body } => {
                self.emit_for_in(*await_, target, *iterable, *body);
            }
            StmtKind::Switch { value, cases } => self.emit_switch(*value, cases),
            StmtKind::Break(label) => match label {
                Some(l) => {
                    let js = self.js_label(self.name(l.sym));
                    self.w.line(&format!("break {js};"));
                }
                None => {
                    if let Some(Some(l)) = self.switch_labels.last() {
                        let l = l.clone();
                        self.w.line(&format!("break {l};"));
                    } else if let Some((_, loop_label, _)) = self.case_labels.last().cloned() {
                        self.w.line(&format!("break {loop_label};"));
                    } else {
                        self.w.line("break;");
                    }
                }
            },
            StmtKind::Continue(label) => match label {
                Some(l) => {
                    let n = self.name(l.sym).to_string();
                    if let Some((_, loop_label, state)) = self.case_labels.iter().rev().find(|(k, _, _)| *k == n).cloned() {
                        self.w.line(&format!("t$state = {state};"));
                        self.w.line(&format!("continue {loop_label};"));
                    } else {
                        let js = self.js_label(&n);
                        self.w.line(&format!("continue {js};"));
                    }
                }
                None => self.w.line("continue;"),
            },
            StmtKind::Return(e) => match e {
                Some(e) => {
                    let expected = self.ret_ty.clone();
                    let expected = match (&self.async_kind, &expected) {
                        (AsyncKind::Async, Ty::Iface { args, .. }) if !args.is_empty() => args[0].clone(),
                        (AsyncKind::Async, Ty::FutureOr { arg, .. }) => (**arg).clone(),
                        _ => expected,
                    };
                    let (js, ty) = self.emit_expr(*e, Some(&expected));
                    self.returns.push(ty);
                    self.w.line(&format!("return {};", js.code));
                }
                None => self.w.line("return;"),
            },
            StmtKind::Yield { star, value } => {
                let (js, _) = self.emit_expr(*value, None);
                match (self.async_kind, star) {
                    (AsyncKind::AsyncStar, false) => {
                        self.m.use_sdk("async");
                        self.w.line(&format!("yield async._IterationMarker.yieldSingle({});", js.code));
                    }
                    (AsyncKind::AsyncStar, true) => {
                        self.m.use_sdk("async");
                        self.w.line(&format!("yield async._IterationMarker.yieldStar({});", js.code));
                    }
                    (AsyncKind::SyncStar, true) => {
                        self.m.use_sdk("async");
                        self.w.line(&format!("yield async._IterationMarker.yieldStar({});", js.code));
                    }
                    _ => self.w.line(&format!("yield {};", js.code)),
                }
            }
            StmtKind::Try { body, catches, finally_ } => self.emit_try(*body, catches, *finally_),
            StmtKind::Labeled { labels, body } => {
                let mut js_labels = Vec::new();
                for l in labels.iter() {
                    let jl = self.fresh_label();
                    self.labels.push((self.name(l.sym).to_string(), jl.clone()));
                    js_labels.push(jl);
                }
                // Um único rótulo JS basta: os demais viram aliases para o mesmo.
                let first = js_labels[0].clone();
                for l in labels.iter().skip(1) {
                    let n = self.name(l.sym).to_string();
                    if let Some(e) = self.labels.iter_mut().rev().find(|(k, _)| *k == n) {
                        e.1 = first.clone();
                    }
                }
                let body_stmt = self.ast().stmt(*body);
                let is_loop = matches!(
                    body_stmt.kind,
                    StmtKind::For { .. } | StmtKind::ForIn { .. } | StmtKind::While { .. } | StmtKind::DoWhile { .. }
                );
                if is_loop || matches!(body_stmt.kind, StmtKind::Switch { .. }) {
                    self.w.line(&format!("{first}:"));
                    self.emit_stmt(*body);
                } else {
                    self.w.open(&format!("{first}: {{"));
                    self.emit_stmt(*body);
                    self.w.close("}");
                }
                for _ in labels.iter() {
                    self.labels.pop();
                }
            }
            StmtKind::Assert { condition, message } => {
                let (cjs, _) = self.emit_cond(*condition);
                let msg = match message {
                    Some(m) => self.emit_expr(*m, None).0.code,
                    None => "null".to_string(),
                };
                let text = js::string_literal(self.text(self.expr(*condition).span));
                self.w.line(&format!("if (!({cjs})) dart.assertFailed({msg}, null, 0, 0, {text});"));
            }
            StmtKind::Empty => self.w.line(";"),
        }
    }

    pub fn js_label(&self, name: &str) -> String {
        self.labels.iter().rev().find(|(k, _)| k == name).map(|(_, v)| v.clone()).unwrap_or_else(|| name.to_string())
    }

    /// Condição booleana (com `dart.test` quando o tipo não é `bool` estrito).
    pub fn emit_cond(&mut self, e: ExprId) -> (String, Ty) {
        let (js, ty) = self.emit_expr(e, Some(&self.ctx.t_bool()));
        if self.ctx.is_bool(&ty) {
            (js.code, ty)
        } else if ty.is_class(self.ctx.bool_) {
            (format!("dart.test({})", js.code), ty)
        } else {
            (format!("dart.dtest({})", js.code), ty)
        }
    }

    fn emit_var_list(&mut self, list: &ast::VariableList) {
        let declared = list.ty.map(|t| self.resolve_type(t));
        for v in list.variables.iter() {
            match v.initializer {
                Some(e) => {
                    let saved_const = self.in_const;
                    if list.const_ {
                        self.in_const = true;
                    }
                    let (js, ty) = self.emit_expr(e, declared.as_ref());
                    self.in_const = saved_const;
                    let ty = match &declared {
                        Some(d) => d.clone(),
                        None => ty,
                    };
                    let jsn = self.declare(v.name.sym, ty);
                    if list.late {
                        self.w.line(&format!("let {jsn} = void 0;"));
                        if let Some(l) = self.scopes.last_mut().and_then(|s| s.get_mut(&v.name.sym)) {
                            l.lazy_init = Some(js.at(crate::js::P_ASSIGN + 1));
                        }
                    } else {
                        self.w.line(&format!("let {jsn} = {};", js.code));
                    }
                }
                None => {
                    let ty = declared.clone().unwrap_or(Ty::Dynamic);
                    let jsn = self.declare(v.name.sym, ty.clone());
                    if list.late {
                        self.w.line(&format!("let {jsn} = void 0;"));
                        if let Some(l) = self.scopes.last_mut().and_then(|s| s.get_mut(&v.name.sym)) {
                            l.late_check = !ty.is_nullable();
                            l.late_final = list.final_;
                        }
                    } else {
                        self.w.line(&format!("let {jsn} = null;"));
                    }
                }
            }
        }
    }

    fn emit_local_function(&mut self, fid: ast::FunctionId) {
        let f = self.ast().function(fid);
        let name = f.name.expect("função local com nome");
        let ty = self.local_fn_ty(f);
        let jsn = self.declare(name.sym, ty.clone());
        let (fn_js, _) = self.emit_function_expr(fid, Some(&ty), false);
        self.w.line(&format!("let {jsn} = {};", fn_js.code));
    }

    /// Tipo declarado de uma função (local ou expressão) a partir da anotação.
    pub fn local_fn_ty(&mut self, f: &ast::Function) -> Ty {
        let mut tps = Vec::new();
        let mut scope_params = Vec::new();
        for tp in f.type_params.iter() {
            let p = self.ctx.fresh_param(self.name(tp.name.sym), self.ctx.t_object_q());
            scope_params.push((p.id, js::ident(&p.name)));
            tps.push(p);
        }
        let saved = self.fn_type_params.clone();
        let mut new = scope_params.clone();
        new.extend(saved.iter().cloned());
        self.fn_type_params = new;
        for (tp, p) in f.type_params.iter().zip(tps.iter_mut()) {
            if let Some(b) = tp.bound {
                let bt = self.resolve_type(b);
                self.ctx.param_bounds.borrow_mut().insert(p.id, bt.clone());
                p.bound = Box::new(bt);
            }
        }
        let ret = f.return_type.map(|r| self.resolve_type(r)).unwrap_or(Ty::Dynamic);
        let (pos, opt, named) = match &f.parameters {
            Some(ps) => self.resolve_params(ps),
            None => (vec![], vec![], vec![]),
        };
        self.fn_type_params = saved;
        Ty::Fn { type_params: tps, ret: Box::new(ret), pos, opt, named, nullable: false }
    }

    fn emit_for_in(&mut self, await_: bool, target: &ast::ForInTarget, iterable: ExprId, body: StmtId) {
        let (ijs, ity) = self.emit_expr(iterable, None);
        let elem_ty = if await_ {
            self.ctx
                .stream_
                .and_then(|s| self.ctx.as_super(&ity, s))
                .and_then(|t| t.args().first().cloned())
                .unwrap_or(Ty::Dynamic)
        } else {
            self.ctx
                .iterable_
                .and_then(|s| self.ctx.as_super(&ity, s))
                .and_then(|t| t.args().first().cloned())
                .unwrap_or(Ty::Dynamic)
        };
        self.push_scope();
        let (var_js, decl_ty, pattern) = match target {
            ast::ForInTarget::Declared { ty, name, .. } => {
                let t = ty.map(|t| self.resolve_type(t)).unwrap_or(elem_ty.clone());
                (self.declare(name.sym, t.clone()), t, None)
            }
            ast::ForInTarget::Pattern { pattern, .. } => (self.temp(), elem_ty.clone(), Some(*pattern)),
            ast::ForInTarget::Expression(e) => {
                let t = self.temp();
                (t, elem_ty.clone(), Some(*e).map(|_| ast::PatternId(u32::MAX)))
            }
        };
        if await_ {
            self.m.use_sdk("async");
            let it = self.temp();
            let elem_rti = self.rti(&elem_ty);
            self.w.line(&format!("{it} = async.StreamIterator.new({elem_rti}[_eval](\"async|StreamIterator<0>\"), {});", ijs.code));
            self.w.open("try {");
            self.w.open(&format!("while ((yield {it}.moveNext())) {{"));
            let _ = &decl_ty;
            match pattern {
                Some(p) if p.0 != u32::MAX => {
                    self.w.line(&format!("let {var_js} = {it}.current;"));
                    self.emit_pattern_bind_stmt(p, &var_js, &elem_ty);
                }
                Some(_) => {
                    if let ast::ForInTarget::Expression(e) = target {
                        let (ljs, _) = self.emit_assign_to(*e, &Js::prim(format!("{it}.current")), &elem_ty);
                        self.w.line(&format!("{};", ljs.code));
                    }
                }
                None => self.w.line(&format!("let {var_js} = {it}.current;")),
            }
            self.emit_stmt(body);
            self.w.close("}");
            self.w.close("}");
            self.w.open("finally {");
            self.w.line(&format!("yield {it}.cancel();"));
            self.w.close("}");
        } else {
            match pattern {
                Some(p) if p.0 != u32::MAX => {
                    self.w.open(&format!("for (let {var_js} of {}) {{", ijs.code));
                    self.emit_pattern_bind_stmt(p, &var_js, &elem_ty);
                }
                Some(_) => {
                    self.w.open(&format!("for (let {var_js} of {}) {{", ijs.code));
                    if let ast::ForInTarget::Expression(e) = target {
                        let (ljs, _) = self.emit_assign_to(*e, &Js::prim(var_js.clone()), &elem_ty);
                        self.w.line(&format!("{};", ljs.code));
                    }
                }
                None => self.w.open(&format!("for (let {var_js} of {}) {{", ijs.code)),
            }
            self.emit_stmt(body);
            self.w.close("}");
        }
        self.pop_scope();
    }

    /// Desestrutura `value_js` no padrão, declarando as variáveis (padrão irrefutável).
    pub fn emit_pattern_bind_stmt(&mut self, p: ast::PatternId, value_js: &str, vty: &Ty) {
        let mut binds = Vec::new();
        let cond = self.pattern_cond(p, value_js, vty, &mut binds, true);
        for (sym, ty) in &binds {
            let jsn = self.declare(*sym, ty.clone());
            self.w.line(&format!("let {jsn} = null;"));
        }
        if cond != "true" {
            self.m.use_sdk("core");
            self.w.line(&format!("if (!({cond})) dart.throw(new core.StateError.new(\"Pattern matching error\"));"));
        }
    }

    fn emit_switch(&mut self, value: ExprId, cases: &[ast::SwitchCase]) {
        let (vjs, vty) = self.emit_expr(value, None);
        // Switch clássico JS quando o valor é primitivo/enum e todos os casos são constantes.
        let all_const = cases.iter().all(|c| match c.pattern {
            None => true,
            Some(p) => self.is_simple_const_pattern(p),
        });
        let prim = self.ctx.is_js_primitive(&vty)
            || vty.class().is_some_and(|c| self.ctx.is_enum_class(c))
            || matches!(vty, Ty::Iface { class, .. } if Some(class) == self.ctx.type_);
        if all_const && prim {
            let label = self.fresh_label();
            let has_case_labels = cases.iter().any(|c| !c.labels.is_empty());
            self.switch_labels.push(None);
            if has_case_labels {
                // `continue rótulo` para outro case: laço com variável de estado.
                self.w.line(&format!("t$state = {};", vjs.code));
                self.temps.push("t$state".into());
                self.temps.dedup();
                self.w.line(&format!("{label}:"));
                self.w.open("while (true) {");
                self.w.open("switch (t$state) {");
                let mut n_pushed = 0;
                for c in cases {
                    if let Some(p) = c.pattern {
                        let consts = self.const_pattern_values(p);
                        if let Some(first) = consts.first() {
                            for l in c.labels.iter() {
                                self.case_labels.push((self.name(l.sym).to_string(), label.clone(), first.clone()));
                                n_pushed += 1;
                            }
                        }
                    }
                }
                for c in cases {
                    match c.pattern {
                        None => self.w.line("default:"),
                        Some(p) => {
                            for cv in self.const_pattern_values(p) {
                                self.w.line(&format!("case {}:", cv));
                            }
                        }
                    }
                    self.w.open("{");
                    self.push_scope();
                    for &s in c.body.iter() {
                        self.emit_stmt(s);
                    }
                    self.pop_scope();
                    if !self.ends_with_jump(&c.body) {
                        self.w.line(&format!("break {label};"));
                    }
                    self.w.close("}");
                }
                self.w.close("}");
                self.w.line("break;");
                self.w.close("}");
                for _ in 0..n_pushed {
                    self.case_labels.pop();
                }
                self.switch_labels.pop();
                return;
            }
            self.w.line(&format!("{label}:"));
            self.w.open(&format!("switch ({}) {{", vjs.code));
            for c in cases {
                let has_stmts = !c.body.is_empty();
                match c.pattern {
                    None => self.w.line("default:"),
                    Some(p) => {
                        let consts = self.const_pattern_values(p);
                        for cv in consts {
                            self.w.line(&format!("case {}:", cv));
                        }
                    }
                }
                if has_stmts {
                    self.w.open("{");
                    self.push_scope();
                    for &s in c.body.iter() {
                        self.emit_stmt(s);
                    }
                    self.pop_scope();
                    // Dart 3: sem fallthrough implícito.
                    if !self.ends_with_jump(&c.body) {
                        self.w.line(&format!("break {label};"));
                    }
                    self.w.close("}");
                }
            }
            self.w.close("}");
            self.switch_labels.pop();
            return;
        }
        // Forma geral: bloco rotulado com testes de padrão.
        let label = self.fresh_label();
        let t = self.temp();
        self.w.line(&format!("{t} = {};", vjs.code));
        self.switch_labels.push(Some(label.clone()));
        self.w.open(&format!("{label}: {{"));
        for c in cases {
            self.w.open("{");
            self.push_scope();
            let cond = match c.pattern {
                None => "true".to_string(),
                Some(p) => {
                    let mut binds = Vec::new();
                    let cond = self.pattern_cond(p, &t, &vty, &mut binds, false);
                    for (sym, ty) in &binds {
                        let jsn = self.declare(*sym, ty.clone());
                        self.w.line(&format!("let {jsn} = null;"));
                    }
                    cond
                }
            };
            let mut full = cond;
            if let Some(g) = c.guard {
                let (gjs, _) = self.emit_expr(g, Some(&self.ctx.t_bool()));
                full = format!("{} && {}", paren_if_needed(&full), gjs.at(crate::js::P_AND));
            }
            self.w.open(&format!("if ({full}) {{"));
            for &s in c.body.iter() {
                self.emit_stmt(s);
            }
            if !self.ends_with_jump(&c.body) {
                self.w.line(&format!("break {label};"));
            }
            self.w.close("}");
            self.pop_scope();
            self.w.close("}");
        }
        self.w.close("}");
        self.switch_labels.pop();
    }

    fn ends_with_jump(&self, body: &[StmtId]) -> bool {
        let Some(last) = body.last() else { return false };
        matches!(
            self.ast().stmt(*last).kind,
            StmtKind::Break(_) | StmtKind::Continue(_) | StmtKind::Return(_)
        ) || matches!(&self.ast().stmt(*last).kind, StmtKind::Expression(e) if matches!(self.expr(*e).kind, ast::ExprKind::Throw(_) | ast::ExprKind::Rethrow))
    }

    pub fn is_simple_const_pattern(&self, p: ast::PatternId) -> bool {
        match &self.ast().pattern(p).kind {
            ast::PatternKind::Constant(e) => self.is_switchable_const(*e),
            ast::PatternKind::Or(a, b) => self.is_simple_const_pattern(*a) && self.is_simple_const_pattern(*b),
            ast::PatternKind::Parenthesized(p) => self.is_simple_const_pattern(*p),
            _ => false,
        }
    }

    fn is_switchable_const(&self, e: ExprId) -> bool {
        match &self.expr(e).kind {
            ast::ExprKind::Int(_) | ast::ExprKind::Bool(_) | ast::ExprKind::Null => true,
            ast::ExprKind::String(lit) => lit.constant_value().is_some(),
            ast::ExprKind::Identifier(_) | ast::ExprKind::Property { .. } => true,
            ast::ExprKind::Unary { op: ast::UnaryOp::Neg, operand } => self.is_switchable_const(*operand),
            ast::ExprKind::Parenthesized(i) => self.is_switchable_const(*i),
            _ => false,
        }
    }

    fn const_pattern_values(&mut self, p: ast::PatternId) -> Vec<String> {
        match &self.ast().pattern(p).kind {
            ast::PatternKind::Constant(e) => vec![self.emit_expr(*e, None).0.code],
            ast::PatternKind::Or(a, b) => {
                let mut v = self.const_pattern_values(*a);
                v.extend(self.const_pattern_values(*b));
                v
            }
            ast::PatternKind::Parenthesized(p) => self.const_pattern_values(*p),
            _ => vec![],
        }
    }

    fn emit_try(&mut self, body: StmtId, catches: &[ast::CatchClause], finally_: Option<StmtId>) {
        self.w.open("try {");
        self.push_scope();
        self.emit_stmt(body);
        self.pop_scope();
        self.w.close("}");
        if !catches.is_empty() {
            let e = format!("t$e{}", self.unique);
            self.unique += 1;
            self.w.open(&format!("catch ({e}) {{"));
            let ex = format!("t$ex{}", self.unique);
            let st = format!("t$st{}", self.unique);
            self.unique += 1;
            self.w.line(&format!("let {ex} = dart.getThrown({e});"));
            if catches.iter().any(|c| c.stack_trace.is_some()) {
                self.w.line(&format!("let {st} = dart.stackTrace({e});"));
            }
            let mut first = true;
            let mut catch_all = false;
            for c in catches {
                let on_ty = c.on_type.map(|t| self.resolve_type(t));
                let is_all = match &on_ty {
                    None => true,
                    Some(Ty::Dynamic) => true,
                    Some(Ty::Iface { class, nullable, .. }) => Some(*class) == self.ctx.object && *nullable,
                    _ => false,
                };
                let cond = match &on_ty {
                    Some(t) if !is_all => self.is_test(&Js::prim(ex.clone()), t).code,
                    _ => "true".to_string(),
                };
                let head = if first {
                    format!("if ({cond}) {{")
                } else {
                    format!("else if ({cond}) {{")
                };
                first = false;
                self.w.open(&head);
                self.push_scope();
                if let Some(n) = c.exception {
                    let ty = match &on_ty {
                        Some(t) if !is_all => t.clone(),
                        _ => self.ctx.t_object(),
                    };
                    let jsn = self.declare(n.sym, ty);
                    self.w.line(&format!("let {jsn} = {ex};"));
                }
                if let Some(n) = c.stack_trace {
                    let ty = self.ctx.stack_trace.map(Ty::iface).unwrap_or(Ty::Dynamic);
                    let jsn = self.declare(n.sym, ty);
                    self.w.line(&format!("let {jsn} = {st};"));
                }
                self.rethrow_var.push(e.clone());
                self.emit_stmt(c.body);
                self.rethrow_var.pop();
                self.pop_scope();
                self.w.close("}");
                if is_all {
                    catch_all = true;
                    break;
                }
            }
            if !catch_all {
                self.w.line(&format!("else throw {e};"));
            }
            self.w.close("}");
        }
        if let Some(f) = finally_ {
            self.w.open("finally {");
            self.push_scope();
            self.emit_stmt(f);
            self.pop_scope();
            self.w.close("}");
        }
    }

    /// Verdadeiro se o corpo contém `await`/`yield` (para decidir gerador).
    pub fn member_lookup_this(&self, name: &str, setter: bool) -> Option<Member> {
        let c = self.class?;
        self.ctx.lookup_member(&self.ctx.this_ty(c), name, setter)
    }

    pub fn member_of(&self, m: &Member) -> (String, Ty) {
        let _ = m;
        (String::new(), Ty::Dynamic)
    }
}

/// Nome JS de membro: `[]` → `_get`, `[]=` → `_set`, `==` → `_equals`, `unary-` → `_negate`.
pub fn js_member_name(name: &str) -> String {
    match name {
        "[]" => "_get".into(),
        "[]=" => "_set".into(),
        "==" => "_equals".into(),
        "unary-" => "_negate".into(),
        _ => name.to_string(),
    }
}

/// Nome de propriedade de elemento de topo (mesmo que o Dart).
pub fn top_level_name(name: &str) -> String {
    name.to_string()
}

/// Nome de membro estático (`as`, `name`, `prototype` ganham `_`).
pub fn static_member_name(name: &str) -> String {
    match name {
        "as" | "name" | "prototype" => format!("{name}_"),
        _ if name.ends_with('_') => format!("{name}_"),
        _ => name.to_string(),
    }
}

pub fn paren_if_needed(s: &str) -> String {
    if s == "true" || s.chars().all(|c| c.is_alphanumeric() || c == '$' || c == '_') {
        s.to_string()
    } else {
        format!("({s})")
    }
}

