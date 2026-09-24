//! Emissão de expressões.

use crate::body::{js_member_name, static_member_name, AsyncKind, FnEmitter};
use crate::ctx::{Member, MemberKind};
use crate::js::{self, Js, P_ADD, P_AND, P_ASSIGN, P_BITAND, P_BITOR, P_BITXOR, P_COMMA, P_COND, P_EQ, P_MUL, P_OR, P_PRIMARY, P_REL, P_SHIFT, P_UNARY, P_YIELD};
use crate::ty::Ty;
use dartforge_elements::model::{ClassId, ClassKind, Element, FunctionKind, LibraryId};
use dartforge_frontend::ast::{self, AssignOp, BinaryOp, CollectionElement, ExprId, ExprKind, UnaryOp};
use std::collections::HashMap;

/// Guarda de null-shorting: `temp` recebe `init`; se for nulo, a cadeia vale `null`.
#[derive(Clone, Debug)]
pub struct Guard {
    pub temp: String,
    pub init: String,
}

/// Alvo resolvido de um identificador simples.
pub enum IdentTarget {
    Local(String, Ty),
    /// Membro de instância via `this` implícito.
    ThisMember(Member),
    /// Membro estático da classe envolvente (ou de superclasse).
    Static(ClassId, MemberKind),
    Element(Element),
    Prefix(dartforge_intern::SymbolId),
    TypeParam(Ty),
    /// Membro do receptor implícito `$this` de uma extensão.
    ExtThisMember(Member),
    /// Membro de extensão aplicável a `this`.
    ThisExt,
    /// Membro de instância da própria extensão (chamado com `$this`).
    ExtMember(dartforge_elements::model::ExtensionId, dartforge_elements::model::FunctionElementId),
    ExtStatic(dartforge_elements::model::ExtensionId, dartforge_elements::model::FunctionElementId),
    /// Campo estático da própria extensão.
    ExtField(dartforge_elements::model::ExtensionId),
    Unknown,
}

impl<'m, 'a> FnEmitter<'m, 'a> {
    /// Resolve um identificador simples.
    pub fn resolve_ident(&self, sym: dartforge_intern::SymbolId) -> IdentTarget {
        if let Some(l) = self.lookup_local(sym) {
            return IdentTarget::Local(l.js.clone(), l.ty.clone());
        }
        let n = self.name(sym);
        if let Some(t) = self.type_param_by_name(n) {
            return IdentTarget::TypeParam(t);
        }
        if let Some(t) = self.extension_this.clone() {
            // Membros da própria extensão, depois do tipo `on`.
            if let Some(ext) = self.current_extension {
                let e = self.ctx.program.extension(ext);
                if let Some(&fid) = e.instance_members.get(&sym) {
                    return IdentTarget::ExtMember(ext, fid);
                }
                if let Some(&fid) = e.static_members.get(&sym) {
                    return IdentTarget::ExtStatic(ext, fid);
                }
                if e.fields.iter().any(|v| self.ctx.program.variable(*v).name == sym) {
                    return IdentTarget::ExtField(ext);
                }
            }
            if let Some(m) = self.ctx.lookup_member(&t, n, false) {
                return IdentTarget::ExtThisMember(m);
            }
            if let Ty::Record { pos, named, .. } = &t {
                let is_field = n.strip_prefix('$').and_then(|x| x.parse::<usize>().ok()).is_some_and(|i| i >= 1 && i <= pos.len())
                    || named.iter().any(|(k, _)| k == n);
                if is_field {
                    return IdentTarget::ExtThisMember(Member { class: self.ctx.object.unwrap_or(ClassId(0)), kind: MemberKind::Field(dartforge_elements::model::VariableId(0)), subst: HashMap::new() });
                }
            }
            if self.find_extension_member(&t, n, false).is_some() {
                return IdentTarget::ExtThisMember(Member { class: self.ctx.object.unwrap_or(ClassId(0)), kind: MemberKind::Field(dartforge_elements::model::VariableId(0)), subst: HashMap::new() });
            }
        }
        if let Some(c) = self.class {
            if !self.is_static {
                if let Some(m) = self.ctx.lookup_member(&self.ctx.this_ty(c), n, false) {
                    return IdentTarget::ThisMember(m);
                }
            }
            // Estáticos da classe e superclasses.
            let mut cur = Some(c);
            while let Some(k) = cur {
                if let Some(mk) = self.ctx.declared_static(k, n, false) {
                    return IdentTarget::Static(k, mk);
                }
                cur = self.ctx.superclass_of(k);
            }
            let class = self.ctx.program.class(c);
            if class.kind == ClassKind::Enum {
                if let Some(&vid) = class.enum_constants.iter().find(|v| self.ctx.program.variable(**v).name == sym) {
                    return IdentTarget::Static(c, MemberKind::Field(vid));
                }
            }
            if !self.is_static && self.ctx.program.lookup(self.lib, sym).is_none() {
                if self.find_extension_member(&self.ctx.this_ty(c), n, false).is_some() {
                    return IdentTarget::ThisExt;
                }
            }
        }
        if let Some(b) = self.ctx.program.lookup(self.lib, sym) {
            if let Some(Element::Prefix(_, p)) = b.getter {
                return IdentTarget::Prefix(p);
            }
            if let Some(e) = b.getter {
                return IdentTarget::Element(e);
            }
            if let Some(e) = b.setter {
                return IdentTarget::Element(e);
            }
        }
        if self.ctx.program.library(self.lib).prefixes.contains_key(&sym) {
            return IdentTarget::Prefix(sym);
        }
        IdentTarget::Unknown
    }

    /// Emite uma expressão; `expected` é o tipo de contexto (inferência descendente).
    pub fn emit_expr(&mut self, e: ExprId, expected: Option<&Ty>) -> (Js, Ty) {
        let expr = self.expr(e);
        let registro = self.registrar_contexto_atalho(e, expected);
        let (js, ty) = match &expr.kind {
            ExprKind::Property { .. } | ExprKind::Index { .. } | ExprKind::Call { .. } => {
                let (js, ty, guards) = self.emit_selector(e, expected);
                (self.wrap_guards(js, guards), ty)
            }
            _ => self.emit_expr_inner(e, expected),
        };
        if let Some((raiz, anterior)) = registro {
            match anterior {
                Some(a) => {
                    self.contextos_atalho.insert(raiz, a);
                }
                None => {
                    self.contextos_atalho.remove(&raiz);
                }
            }
        }
        match expected {
            Some(exp) => self.coerce_to(js, ty, exp),
            None => (js, ty),
        }
    }

    /// Atalho de ponto (3.10): se `e` é um nó de uma cadeia de seletores
    /// (`.x`, `(…)`, `[…]`, `<T>`, `!`) cuja raiz é `.id`, o contexto da
    /// cadeia (`expected`) vira o *shorthand context* da raiz, salvo se um
    /// nó mais externo (ou `==`, ou um padrão) já registrou um contexto
    /// conhecido. Devolve o que restaurar ao fim da emissão de `e`.
    fn registrar_contexto_atalho(&mut self, e: ExprId, expected: Option<&Ty>) -> Option<(ExprId, Option<Option<Ty>>)> {
        if !self.atalhos {
            return None;
        }
        if !matches!(
            self.expr(e).kind,
            ExprKind::Property { .. } | ExprKind::Index { .. } | ExprKind::Call { .. } | ExprKind::TypeArguments { .. } | ExprKind::Unary { op: UnaryOp::NullAssert, .. }
        ) {
            return None;
        }
        let raiz = self.ast().raiz_de_atalho(e)?;
        let anterior = self.contextos_atalho.get(&raiz).cloned();
        if matches!(anterior, Some(Some(_))) || (anterior.is_some() && expected.is_none()) {
            return None;
        }
        self.contextos_atalho.insert(raiz, expected.cloned());
        Some((raiz, anterior))
    }

    /// Registra `ctx` como contexto do atalho na raiz de `e` (se houver)
    /// enquanto `f` emite; `==` e padrões dão o contexto assim (spec 3.10,
    /// "Special case for `==`").
    pub fn com_contexto_de_atalho<R>(&mut self, e: ExprId, ctx: &Ty, f: impl FnOnce(&mut Self) -> R) -> R {
        let raiz = if self.atalhos { self.ast().raiz_de_atalho(e) } else { None };
        let Some(raiz) = raiz else { return f(self) };
        let anterior = self.contextos_atalho.insert(raiz, Some(ctx.clone()));
        let r = f(self);
        match anterior {
            Some(a) => {
                self.contextos_atalho.insert(raiz, a);
            }
            None => {
                self.contextos_atalho.remove(&raiz);
            }
        }
        r
    }

    /// A declaração que o contexto de um atalho denota (spec 3.10): `C`,
    /// `C<…>` e `C?` denotam `C`; `FutureOr<S>` denota o que `S` denota;
    /// tipo de função, record, parâmetro de tipo e `dynamic` não denotam
    /// nada.
    pub fn declaracao_do_contexto(&self, t: &Ty) -> Option<ClassId> {
        match t {
            Ty::Iface { class, args, .. } if Some(*class) == self.ctx.future_or => args.first().and_then(|a| self.declaracao_do_contexto(a)),
            Ty::Iface { class, .. } => Some(*class),
            Ty::FutureOr { arg, .. } => self.declaracao_do_contexto(arg),
            _ => None,
        }
    }

    /// `D` de um atalho de ponto: pelo contexto registrado para ele, senão
    /// pelo `expected` da própria posição. Sem declaração, registra o erro
    /// de linguagem (fora de emissão especulativa).
    pub(crate) fn classe_do_atalho(&mut self, e: ExprId, expected: Option<&Ty>) -> Option<ClassId> {
        let ctx = match self.contextos_atalho.get(&e) {
            Some(Some(t)) => Some(t.clone()),
            _ => expected.cloned(),
        };
        let d = ctx.as_ref().and_then(|t| self.declaracao_do_contexto(t));
        if d.is_none() {
            let ExprKind::DotShorthand { name, .. } = &self.expr(e).kind else { return None };
            let msg = format!("nenhum tipo de contexto para achar o atalho de ponto '.{}'", self.name(name.sym));
            self.erro_de_linguagem(self.expr(e).span, msg);
        }
        d
    }

    /// Registra um erro de linguagem na emissão (ignorado quando a emissão
    /// é só para descobrir um tipo).
    pub fn erro_de_linguagem(&self, span: dartforge_diagnostics::Span, msg: String) {
        if self.especulando > 0 {
            return;
        }
        let arquivo = self.ctx.program.unit(self.unit).path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
        self.ctx.erros.borrow_mut().push(dartforge_diagnostics::Diagnostic::new(
            format!("{}{arquivo}:{}: {msg}", dartforge_types::codes::ERRO_DE_LINGUAGEM, span.start),
            span,
        ));
    }

    /// `.id` / `.new` sem chamada: `D.id` (getter, campo, constante de enum,
    /// tear-off de método ou de construtor).
    fn emit_atalho(&mut self, e: ExprId, expected: Option<&Ty>) -> (Js, Ty) {
        let ExprKind::DotShorthand { name, .. } = &self.expr(e).kind else { unreachable!() };
        let nome = self.name(name.sym).to_string();
        let span = self.expr(e).span;
        let Some(d) = self.classe_do_atalho(e, expected) else { return (Js::prim("null"), Ty::Dynamic) };
        match self.static_member_get(d, &nome) {
            Some(r) => r,
            None => {
                let msg = format!("'{}' não tem membro estático nem construtor '{nome}' para o atalho de ponto", self.ctx.class_name(d));
                self.erro_de_linguagem(span, msg);
                (Js::prim("null"), Ty::Dynamic)
            }
        }
    }

    /// Coerções implícitas: instanciação de tearoff genérico e `.call` de classe invocável.
    pub fn coerce_to(&mut self, js: Js, ty: Ty, expected: &Ty) -> (Js, Ty) {
        if ty.is_dynamic() && !self.in_const {
            let needs_cast = match expected {
                Ty::Iface { class, nullable, .. } => !(Some(*class) == self.ctx.object && *nullable),
                Ty::Fn { .. } | Ty::Record { .. } | Ty::FutureOr { .. } => true,
                _ => false,
            };
            if needs_cast && !expected.mentions_params() && js.code != "null" {
                return (self.as_cast(&js, expected), expected.clone());
            }
            return (js, ty);
        }
        let Ty::Fn { type_params: etps, .. } = expected else { return (js, ty) };
        if !etps.is_empty() {
            return (js, ty);
        }
        match &ty {
            Ty::Fn { type_params, .. } if !type_params.is_empty() => {
                // Instancia com os argumentos inferidos do tipo esperado.
                let free: Vec<u32> = type_params.iter().map(|p| p.id).collect();
                let inner = match &ty {
                    Ty::Fn { ret, pos, opt, named, nullable, .. } => Ty::Fn { type_params: vec![], ret: ret.clone(), pos: pos.clone(), opt: opt.clone(), named: named.clone(), nullable: *nullable },
                    t => t.clone(),
                };
                let mut subst = HashMap::new();
                self.match_type(&inner, expected, &free, &mut subst);
                let mut rtis = Vec::new();
                for p in type_params {
                    let t = subst.get(&p.id).cloned().unwrap_or_else(|| self.default_type_arg(&p.bound));
                    subst.insert(p.id, t.clone());
                    rtis.push(self.rti(&t));
                }
                (Js::prim(format!("dart.gbind({}, {})", js.code, rtis.join(", "))), inner.subst(&subst))
            }
            Ty::Iface { class, nullable: false, .. } if Some(*class) != self.ctx.function_ => {
                // Classe invocável: tearoff de `call`.
                match self.ctx.lookup_member(&ty, "call", false) {
                    Some(m) if matches!(m.kind, MemberKind::Method(_)) => {
                        let mty = self.ctx.member_ty(&m);
                        let key = self.member_key_string(&ty, "call");
                        (Js::prim(format!("dart.bind({}, {key})", js.code)), mty)
                    }
                    _ => (js, ty),
                }
            }
            _ => (js, ty),
        }
    }

    /// Nome de membro como expressão JS (string, símbolo `dartx` ou privado) para `dart.bind`.
    pub fn member_key_string(&self, recv_ty: &Ty, name: &str) -> String {
        if name.starts_with('_') {
            let lib = self.ctx.lookup_member(recv_ty, name, false).map(|m| self.ctx.lib_of_class(m.class)).unwrap_or(self.lib);
            self.private_sym(lib, name)
        } else if self.ctx.is_ext_member(recv_ty, name, false) {
            self.dartx(&js_member_name(name))
        } else {
            js::string_literal(&js_member_name(name))
        }
    }

    pub fn wrap_guards(&mut self, js: Js, guards: Vec<Guard>) -> Js {
        if guards.is_empty() {
            return js;
        }
        let mut code = js.code;
        for g in guards.iter().rev() {
            code = format!("{} = {}, {} == null ? null : {}", g.temp, g.init, g.temp, code);
        }
        Js::new(code, P_COMMA).paren()
    }

    fn emit_expr_inner(&mut self, e: ExprId, expected: Option<&Ty>) -> (Js, Ty) {
        let expr = self.expr(e);
        match &expr.kind {
            ExprKind::Int(span) => {
                let text = self.text(*span).replace('_', "");
                let v = parse_int(&text);
                let is_double_ctx = expected.is_some_and(|t| self.ctx.is_double(t));
                if is_double_ctx {
                    (Js::prim(format!("{v}.0")), self.ctx.t_double())
                } else {
                    (Js::prim(v), self.ctx.t_int())
                }
            }
            ExprKind::Double(span) => {
                let text = self.text(*span).replace('_', "");
                let v: f64 = text.parse().unwrap_or(0.0);
                let s = if v.fract() == 0.0 && v.abs() < 1e21 {
                    format!("{v:.1}")
                } else {
                    let t = text.trim_start_matches('0');
                    if t.starts_with('.') { format!("0{t}") } else { t.to_string() }
                };
                (Js::prim(s), self.ctx.t_double())
            }
            ExprKind::Bool(b) => (Js::prim(if *b { "true" } else { "false" }), self.ctx.t_bool()),
            ExprKind::Null => (Js::prim("null"), Ty::Null),
            ExprKind::String(lit) => self.emit_string_lit(lit),
            ExprKind::Symbol(names) => {
                let text: Vec<&str> = names.iter().map(|n| self.name(n.sym)).collect();
                self.m.use_sdk("_internal");
                (
                    Js::prim(format!("dart.const(new _internal.Symbol.new({}))", js::string_literal(&text.join(".")))),
                    self.ctx.symbol_.map(Ty::iface).unwrap_or(Ty::Dynamic),
                )
            }
            ExprKind::Identifier(name) => self.emit_identifier(name.sym, e),
            ExprKind::DotShorthand { .. } => self.emit_atalho(e, expected),
            ExprKind::This => {
                if let Some(t) = &self.extension_this {
                    return (Js::prim("$this"), t.clone());
                }
                let ty = self.class.map(|c| self.ctx.this_ty(c)).unwrap_or(Ty::Dynamic);
                (Js::prim("this"), ty)
            }
            ExprKind::Super => {
                let ty = self.super_ty();
                (Js::prim("super"), ty)
            }
            ExprKind::Parenthesized(inner) => {
                let (js, ty) = self.emit_expr(*inner, expected);
                (js.paren(), ty)
            }
            ExprKind::List { const_, type_args, elements } => self.emit_list_literal(*const_, type_args, elements, expected),
            ExprKind::SetOrMap { const_, type_args, elements } => self.emit_set_or_map(*const_, type_args, elements, expected),
            ExprKind::Record { positional, named, const_ } if *const_ && positional.len() == 1 && named.is_empty() => {
                let saved = self.in_const;
                self.in_const = true;
                let r = self.emit_expr(positional[0], expected);
                self.in_const = saved;
                r
            }
            ExprKind::Record { positional, named, .. } => self.emit_record(positional, named),
            ExprKind::InstanceCreation { keyword, ty, constructor, arguments } => {
                self.emit_instance_creation(*keyword, *ty, constructor.as_ref(), arguments, expected)
            }
            ExprKind::FunctionExpression(fid) => {
                let saved_const = self.in_const;
                self.in_const = false;
                let r = self.emit_function_expr(*fid, expected, true);
                self.in_const = saved_const;
                r
            }
            ExprKind::TypeArguments { target, type_args } => {
                let tys: Vec<Ty> = type_args.iter().map(|t| self.resolve_type(*t)).collect();
                if let ExprKind::Identifier(id) = &self.expr(*target).kind {
                    if let IdentTarget::Element(Element::Class(c)) = self.resolve_ident(id.sym) {
                        let t = Ty::Iface { class: c, args: tys, nullable: false };
                        let rti = self.rti(&t);
                        return (Js::prim(format!("dart_rti.createRuntimeType({rti})")), self.ctx.t_type());
                    }
                }
                let (fjs, fty) = self.emit_expr(*target, None);
                let rtis: Vec<String> = tys.iter().map(|t| self.rti(t)).collect();
                let ret_ty = match &fty {
                    Ty::Fn { type_params, .. } => {
                        let mut map = HashMap::new();
                        for (p, a) in type_params.iter().zip(tys.iter()) {
                            map.insert(p.id, a.clone());
                        }
                        match fty.subst(&map) {
                            Ty::Fn { ret, pos, opt, named, nullable, .. } => Ty::Fn { type_params: vec![], ret, pos, opt, named, nullable },
                            t => t,
                        }
                    }
                    _ => Ty::Dynamic,
                };
                (Js::prim(format!("dart.gbind({}, {})", fjs.code, rtis.join(", "))), ret_ty)
            }
            ExprKind::Unary { op, operand } => self.emit_unary(*op, *operand),
            ExprKind::Binary { op, left, right } => self.emit_binary(*op, *left, *right, expected),
            ExprKind::Conditional { condition, then, else_ } => {
                self.pending_promotions.clear();
                self.negated_promotions.clear();
                let (c, _) = self.emit_cond(*condition);
                let promos = std::mem::take(&mut self.pending_promotions);
                let neg = std::mem::take(&mut self.negated_promotions);
                self.push_scope();
                for (sym, t) in &promos {
                    if let Some(loc) = self.lookup_local(*sym).cloned() {
                        self.declare_js(*sym, loc.js, t.clone());
                    }
                }
                let (a, at) = self.emit_expr(*then, expected);
                self.pop_scope();
                self.push_scope();
                for (sym, t) in &neg {
                    if let Some(loc) = self.lookup_local(*sym).cloned() {
                        self.declare_js(*sym, loc.js, t.clone());
                    }
                }
                let (b, bt) = self.emit_expr(*else_, expected);
                self.pop_scope();
                let ty = self.ctx.lub(&at, &bt);
                (Js::new(format!("{c} ? {} : {}", a.at(P_COND), b.at(P_COND)), P_COND), ty)
            }
            ExprKind::Is { value, ty, negated } => {
                let (v, vty) = self.emit_expr(*value, None);
                let t = self.resolve_type(*ty);
                let test = self.is_test(&v, &t);
                // Promoção de variável local.
                if let ExprKind::Identifier(n) = &self.expr(*value).kind {
                    if self.lookup_local(n.sym).is_some() && (self.ctx.is_subtype(&t, &vty) || vty.is_dynamic()) {
                        if *negated {
                            self.negated_promotions.push((n.sym, t.clone()));
                        } else {
                            self.pending_promotions.push((n.sym, t.clone()));
                        }
                    }
                }
                if *negated {
                    (Js::new(format!("!{}", test.at(P_UNARY)), P_UNARY), self.ctx.t_bool())
                } else {
                    (test, self.ctx.t_bool())
                }
            }
            ExprKind::As { value, ty } => {
                let t = self.resolve_type(*ty);
                let (v, vty) = self.emit_expr(*value, Some(&t));
                if self.ctx.is_subtype(&vty, &t) && !vty.is_dynamic() {
                    return (v, t);
                }
                (self.as_cast(&v, &t), t)
            }
            ExprKind::Assign { op, target, value } => self.emit_assign(*op, *target, *value),
            ExprKind::PatternAssign { pattern, value } => {
                let (vjs, vty) = self.emit_expr(*value, None);
                let t = self.temp();
                let mut binds = Vec::new();
                self.pattern_assign = true;
                let cond = self.pattern_cond(*pattern, &t, &vty, &mut binds, true);
                self.pattern_assign = false;
                let code = if cond == "true" {
                    format!("{t} = {}, {t}", vjs.code)
                } else {
                    self.m.use_sdk("core");
                    format!("{t} = {}, ({cond}) || dart.throw(new core.StateError.new(\"Pattern matching error\")), {t}", vjs.code)
                };
                (Js::new(code, P_COMMA).paren(), vty)
            }
            ExprKind::Cascade { target, sections, null_aware } => {
                let (tjs, tty) = self.emit_expr(*target, expected);
                let t = self.temp();
                self.cascade.push((t.clone(), tty.clone()));
                let mut parts = Vec::new();
                for s in sections.iter() {
                    let (sjs, _) = self.emit_expr(*s, None);
                    parts.push(sjs.into_at(P_ASSIGN));
                }
                self.cascade.pop();
                let body = if parts.is_empty() { t.clone() } else { format!("{}, {t}", parts.join(", ")) };
                let code = if *null_aware {
                    format!("{t} = {}, {t} == null ? null : ({body})", tjs.code)
                } else {
                    format!("{t} = {}, {body}", tjs.code)
                };
                (Js::new(code, P_COMMA).paren(), tty)
            }
            ExprKind::CascadeTarget => {
                let (t, ty) = self.cascade.last().cloned().unwrap_or(("null".into(), Ty::Dynamic));
                (Js::prim(t), ty)
            }
            ExprKind::Await(inner) => {
                let (v, vty) = self.emit_expr(*inner, None);
                let ty = match &vty {
                    Ty::FutureOr { arg, .. } => (**arg).clone(),
                    Ty::Iface { .. } => match self.ctx.future_.and_then(|f| self.ctx.as_super(&vty, f)) {
                        Some(ft) => ft.args().first().cloned().unwrap_or(Ty::Dynamic),
                        None => vty.clone(),
                    },
                    _ => Ty::Dynamic,
                };
                (Js::new(format!("yield {}", v.at(P_YIELD)), P_YIELD).paren(), ty)
            }
            ExprKind::Throw(inner) => {
                let (v, _) = self.emit_expr(*inner, None);
                (Js::prim(format!("dart.throw({})", v.code)), Ty::Never)
            }
            ExprKind::Rethrow => {
                let v = self.rethrow_var.last().cloned().unwrap_or_else(|| "null".into());
                (Js::prim(format!("dart.rethrow({v})")), Ty::Never)
            }
            ExprKind::Switch { value, cases } => self.emit_switch_expr(*value, cases, expected),
            ExprKind::Property { .. } | ExprKind::Index { .. } | ExprKind::Call { .. } => {
                let (js, ty, guards) = self.emit_selector(e, expected);
                (self.wrap_guards(js, guards), ty)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Literais
    // -----------------------------------------------------------------------

    fn emit_string_lit(&mut self, lit: &ast::StringLit) -> (Js, Ty) {
        let mut parts: Vec<String> = Vec::new();
        let mut pending: Vec<u16> = Vec::new();
        for part in lit.parts.iter() {
            match part {
                ast::StringPart::Text(t) => pending.extend(t.code_units()),
                ast::StringPart::Interpolation(e) => {
                    if !pending.is_empty() {
                        parts.push(js::string_literal_units(pending.drain(..)));
                    }
                    let (v, vty) = self.emit_expr(*e, None);
                    if self.ctx.is_string(&vty) {
                        parts.push(v.into_at(P_ADD));
                    } else if self.ctx.is_num_like(&vty) && !vty.is_class(self.ctx.num_) || self.ctx.is_bool(&vty) {
                        parts.push(format!("dart.strSafe({})", v.code));
                    } else {
                        parts.push(format!("dart.str({})", v.code));
                    }
                }
            }
        }
        if !pending.is_empty() || parts.is_empty() {
            parts.push(js::string_literal_units(pending.drain(..)));
        }
        if parts.len() == 1 {
            let is_lit = parts[0].starts_with('"');
            return (Js::new(parts.remove(0), if is_lit { P_PRIMARY } else { P_PRIMARY }), self.ctx.t_string());
        }
        // Garante que a concatenação começa por string.
        if !parts[0].starts_with('"') {
            parts.insert(0, "\"\"".into());
        }
        (Js::new(parts.join(" + "), P_ADD), self.ctx.t_string())
    }

    /// Tipo de elemento esperado para um literal de coleção.
    fn expected_arg(&self, expected: Option<&Ty>, class: Option<ClassId>, idx: usize) -> Option<Ty> {
        let e = expected?;
        let c = class?;
        let sup = self.ctx.as_super(&e.non_null(), c)?;
        sup.args().get(idx).cloned()
    }

    fn emit_list_literal(&mut self, const_: bool, type_args: &[ast::TypeId], elements: &[CollectionElement], expected: Option<&Ty>) -> (Js, Ty) {
        let const_ = const_ || self.in_const;
        let saved_const = self.in_const;
        if const_ {
            self.in_const = true;
        }
        let r = self.emit_list_literal_inner(const_, type_args, elements, expected);
        self.in_const = saved_const;
        r
    }

    fn emit_list_literal_inner(&mut self, const_: bool, type_args: &[ast::TypeId], elements: &[CollectionElement], expected: Option<&Ty>) -> (Js, Ty) {
        let elem_ty = if let Some(t) = type_args.first() {
            self.resolve_type(*t)
        } else if let Some(t) = self.expected_arg(expected, self.ctx.list_, 0).or_else(|| self.expected_arg(expected, self.ctx.iterable_, 0)) {
            t
        } else {
            self.infer_elements_ty(elements)
        };
        let list_ty = self.ctx.t_list(elem_ty.clone());
        let simple = elements.iter().all(|e| matches!(e, CollectionElement::Expression(_)));
        if simple {
            let items: Vec<String> = elements
                .iter()
                .map(|e| match e {
                    CollectionElement::Expression(x) => self.emit_expr(*x, Some(&elem_ty)).0.into_at(P_ASSIGN),
                    _ => unreachable!(),
                })
                .collect();
            if const_ {
                let rti = self.rti(&elem_ty);
                return (Js::prim(format!("dart.constList({rti}, [{}])", items.join(", "))), list_ty);
            }
            if elem_ty.is_dynamic() {
                return (Js::prim(format!("[{}]", items.join(", "))), list_ty);
            }
            let jsarr = self.ctx.js_array.map(|c| Ty::iface_args(c, vec![elem_ty.clone()])).unwrap_or(list_ty.clone());
            let rti = self.rti(&jsarr);
            self.m.use_sdk("_interceptors");
            return (Js::prim(format!("_interceptors.JSArray.of({rti}, [{}])", items.join(", "))), list_ty);
        }
        // Forma geral: IIFE construindo a lista.
        let t = self.temp();
        let saved_w = std::mem::take(&mut self.w);
        let jsarr = self.ctx.js_array.map(|c| Ty::iface_args(c, vec![elem_ty.clone()])).unwrap_or(list_ty.clone());
        let rti = self.rti(&jsarr);
        self.m.use_sdk("_interceptors");
        if const_ {
            crate::linha!(self.w, "{t} = [];");
        } else {
            crate::linha!(self.w, "{t} = _interceptors.JSArray.of({rti}, []);");
        }
        let t2 = t.clone();
        for el in elements {
            self.emit_collection_element(el, &elem_ty, None, &|s: &mut Self, v: String| {
                crate::linha!(s.w, "{t2}.push({v});");
            });
        }
        if const_ {
            let er = self.rti(&elem_ty);
            crate::linha!(self.w, "return dart.constList({er}, {t});");
        } else {
            crate::linha!(self.w, "return {t};");
        }
        let body = std::mem::replace(&mut self.w, saved_w).out;
        (self.iife(&body), list_ty)
    }

    fn infer_elements_ty(&mut self, elements: &[CollectionElement]) -> Ty {
        // Sem emitir: tipa os elementos numa cópia descartável do escritor.
        let mut tys = Vec::new();
        let saved = std::mem::take(&mut self.w);
        let saved_temps = self.temps.len();
        let saved_counter = self.temp_counter;
        for el in elements {
            match el {
                CollectionElement::Expression(e) | CollectionElement::NullAwareExpression(e) => {
                    let t = self.type_of(*e);
                    tys.push(if matches!(el, CollectionElement::NullAwareExpression(_)) { t.non_null() } else { t });
                }
                CollectionElement::Spread { value, .. } => {
                    let t = self.type_of(*value);
                    if let Some(it) = self.ctx.iterable_.and_then(|i| self.ctx.as_super(&t.non_null(), i)) {
                        tys.push(it.args().first().cloned().unwrap_or(Ty::Dynamic));
                    } else if !matches!(t, Ty::Null) {
                        tys.push(Ty::Dynamic);
                    }
                }
                CollectionElement::If { then, else_, .. } => {
                    tys.push(self.infer_elements_ty(std::slice::from_ref(then)));
                    if let Some(e) = else_ {
                        tys.push(self.infer_elements_ty(std::slice::from_ref(e)));
                    }
                }
                CollectionElement::For { body, .. } | CollectionElement::ForIn { body, .. } => {
                    self.push_scope();
                    if let CollectionElement::ForIn { target: ast::ForInTarget::Declared { name, ty, .. }, iterable, .. } = el {
                        let it = self.type_of(*iterable);
                        let et = ty.map(|t| self.resolve_type(t)).unwrap_or_else(|| {
                            self.ctx.iterable_.and_then(|i| self.ctx.as_super(&it, i)).and_then(|t| t.args().first().cloned()).unwrap_or(Ty::Dynamic)
                        });
                        self.declare(name.sym, et);
                    }
                    if let CollectionElement::For { init: Some(ast::ForInit::Variables(list)), .. } = el {
                        let declared = list.ty.map(|t| self.resolve_type(t));
                        for v in list.variables.iter() {
                            let t = match (declared.clone(), v.initializer) {
                                (Some(d), _) => d,
                                (None, Some(i)) => self.type_of(i),
                                _ => Ty::Dynamic,
                            };
                            self.declare(v.name.sym, t);
                        }
                    }
                    tys.push(self.infer_elements_ty(std::slice::from_ref(body)));
                    self.pop_scope();
                }
                CollectionElement::MapEntry { .. } => tys.push(Ty::Dynamic),
            }
        }
        self.w = saved;
        self.temps.truncate(saved_temps);
        self.temp_counter = saved_counter;
        let mut it = tys.into_iter();
        match it.next() {
            None => Ty::Dynamic,
            Some(first) => it.fold(first, |a, b| self.ctx.lub(&a, &b)),
        }
    }

    /// Tipo estático de uma expressão sem emitir (descarta o texto).
    pub fn type_of(&mut self, e: ExprId) -> Ty {
        let saved = std::mem::take(&mut self.w);
        let saved_temps = self.temps.len();
        let saved_counter = self.temp_counter;
        let saved_ret = self.returns.len();
        self.especulando += 1;
        let (_, t) = self.emit_expr(e, None);
        self.especulando -= 1;
        self.w = saved;
        self.temps.truncate(saved_temps);
        self.temp_counter = saved_counter;
        self.returns.truncate(saved_ret);
        t
    }

    /// IIFE (ou `yield*` de gerador dentro de geradores).
    pub fn iife(&self, body: &str) -> Js {
        match self.async_kind {
            AsyncKind::None => Js::prim(format!("(() => {{\n{body}}})()")),
            _ => Js::new(format!("yield* (function*() {{\n{body}}}).call(this)"), P_YIELD).paren(),
        }
    }

    fn emit_collection_element(
        &mut self,
        el: &CollectionElement,
        elem_ty: &Ty,
        value_ty: Option<&Ty>,
        add: &dyn Fn(&mut Self, String),
    ) {
        match el {
            CollectionElement::Expression(e) => {
                let (v, _) = self.emit_expr(*e, Some(elem_ty));
                add(self, v.code);
            }
            CollectionElement::NullAwareExpression(e) => {
                // Contexto anulável para o operando (spec 3.8): o `null` é
                // descartado aqui, não chega ao literal.
                let (v, _) = self.emit_expr(*e, Some(&elem_ty.with_nullable(true)));
                let t = self.temp();
                crate::linha!(self.w, "{t} = {};", v.code);
                crate::abre!(self.w, "if ({t} != null) {{");
                add(self, t);
                self.w.close("}");
            }
            CollectionElement::MapEntry { key, value, null_aware_key: false, null_aware_value: false } => {
                let (k, _) = self.emit_expr(*key, Some(elem_ty));
                let (v, _) = self.emit_expr(*value, value_ty);
                add(self, format!("{}\u{0}{}", k.code, v.code));
            }
            CollectionElement::MapEntry { key, value, null_aware_key, null_aware_value } => {
                // Entrada null-aware (3.8): a chave antes do valor; chave
                // null-aware nula descarta a entrada SEM avaliar o valor.
                let kctx = if *null_aware_key { elem_ty.with_nullable(true) } else { elem_ty.clone() };
                let (k, _) = self.emit_expr(*key, Some(&kctx));
                let tk = self.temp();
                crate::linha!(self.w, "{tk} = {};", k.code);
                if *null_aware_key {
                    crate::abre!(self.w, "if ({tk} != null) {{");
                }
                let vctx = value_ty.map(|t| if *null_aware_value { t.with_nullable(true) } else { t.clone() });
                let (v, _) = self.emit_expr(*value, vctx.as_ref());
                let tv = self.temp();
                crate::linha!(self.w, "{tv} = {};", v.code);
                if *null_aware_value {
                    crate::abre!(self.w, "if ({tv} != null) {{");
                }
                add(self, format!("{tk}\u{0}{tv}"));
                if *null_aware_value {
                    self.w.close("}");
                }
                if *null_aware_key {
                    self.w.close("}");
                }
            }
            CollectionElement::Spread { value, null_aware } => {
                let (v, vty) = self.emit_expr(*value, None);
                let t = self.temp();
                crate::linha!(self.w, "{t} = {};", v.code);
                if *null_aware {
                    crate::abre!(self.w, "if ({t} != null) {{");
                }
                if value_ty.is_some() {
                    // Mapa: percorre entradas.
                    let entries = self.member_access(&vty.non_null(), "entries", false);
                    let x = self.temp();
                    crate::abre!(self.w, "for ({x} of {t}{entries}) {{");
                    add(self, format!("{x}.key\u{0}{x}.value"));
                    self.w.close("}");
                } else {
                    let x = self.temp();
                    crate::abre!(self.w, "for ({x} of {t}) {{");
                    add(self, x);
                    self.w.close("}");
                }
                if *null_aware {
                    self.w.close("}");
                }
            }
            CollectionElement::If { condition, case_pattern, guard, then, else_ } => {
                if let Some(pat) = case_pattern {
                    let (vjs, vty) = self.emit_expr(*condition, None);
                    let t = self.temp();
                    crate::linha!(self.w, "{t} = {};", vjs.code);
                    self.push_scope();
                    let mut binds = Vec::new();
                    let cond = self.pattern_cond(*pat, &t, &vty, &mut binds, false);
                    for (_, _, jsn) in &binds {
                        crate::linha!(self.w, "let {jsn} = null;");
                    }
                    let mut full = cond;
                    if let Some(g) = guard {
                        let (gjs, _) = self.emit_expr(*g, Some(&self.ctx.t_bool()));
                        full = format!("({full}) && {}", gjs.at(P_AND));
                    }
                    crate::abre!(self.w, "if ({full}) {{");
                    self.emit_collection_element(then, elem_ty, value_ty, add);
                    self.w.close("}");
                    if let Some(e) = else_ {
                        self.w.open("else {");
                        self.emit_collection_element(e, elem_ty, value_ty, add);
                        self.w.close("}");
                    }
                    self.pop_scope();
                    return;
                }
                let (c, _) = self.emit_cond(*condition);
                crate::abre!(self.w, "if ({c}) {{");
                self.emit_collection_element(then, elem_ty, value_ty, add);
                self.w.close("}");
                if let Some(e) = else_ {
                    self.w.open("else {");
                    self.emit_collection_element(e, elem_ty, value_ty, add);
                    self.w.close("}");
                }
            }
            CollectionElement::For { init, condition, updates, body, .. } => {
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
                self.emit_collection_element(body, elem_ty, value_ty, add);
                self.w.close("}");
                self.pop_scope();
            }
            CollectionElement::ForIn { target, iterable, body, await_ } => {
                let (ijs, ity) = self.emit_expr(*iterable, None);
                let et = self
                    .ctx
                    .iterable_
                    .and_then(|i| self.ctx.as_super(&ity, i))
                    .and_then(|t| t.args().first().cloned())
                    .unwrap_or(Ty::Dynamic);
                self.push_scope();
                let _ = await_;
                match target {
                    ast::ForInTarget::Declared { name, ty, .. } => {
                        let t = ty.map(|t| self.resolve_type(t)).unwrap_or(et.clone());
                        let jsn = self.declare(name.sym, t);
                        crate::abre!(self.w, "for (let {jsn} of {}) {{", ijs.code);
                    }
                    ast::ForInTarget::Pattern { pattern, .. } => {
                        let t = self.temp();
                        crate::abre!(self.w, "for ({t} of {}) {{", ijs.code);
                        self.emit_pattern_bind_stmt(*pattern, &t, &et);
                    }
                    ast::ForInTarget::Expression(e) => {
                        let t = self.temp();
                        crate::abre!(self.w, "for ({t} of {}) {{", ijs.code);
                        let (ljs, _) = self.emit_assign_to(*e, &Js::prim(t), &et);
                        crate::linha!(self.w, "{};", ljs.code);
                    }
                }
                self.emit_collection_element(body, elem_ty, value_ty, add);
                self.w.close("}");
                self.pop_scope();
            }
        }
    }

    fn emit_set_or_map(&mut self, const_: bool, type_args: &[ast::TypeId], elements: &[CollectionElement], expected: Option<&Ty>) -> (Js, Ty) {
        let const_ = const_ || self.in_const;
        let saved_const = self.in_const;
        if const_ {
            self.in_const = true;
        }
        let r = self.emit_set_or_map_inner(const_, type_args, elements, expected);
        self.in_const = saved_const;
        r
    }

    fn emit_set_or_map_inner(&mut self, const_: bool, type_args: &[ast::TypeId], elements: &[CollectionElement], expected: Option<&Ty>) -> (Js, Ty) {
        let is_map = if type_args.len() == 2 {
            true
        } else if type_args.len() == 1 {
            false
        } else if elements.is_empty() {
            !expected.is_some_and(|t| t.non_null().class() == self.ctx.set_ || self.ctx.set_.is_some_and(|s| self.ctx.as_super(&t.non_null(), s).is_some() && t.non_null().class() != self.ctx.map_))
        } else {
            let mut any_map = false;
            let mut any_set = false;
            for e in elements {
                self.classify_element(e, &mut any_map, &mut any_set);
            }
            any_map && !any_set
        };
        if is_map {
            let (kt, vt) = if type_args.len() == 2 {
                (self.resolve_type(type_args[0]), self.resolve_type(type_args[1]))
            } else if let (Some(k), Some(v)) = (self.expected_arg(expected, self.ctx.map_, 0), self.expected_arg(expected, self.ctx.map_, 1)) {
                (k, v)
            } else {
                self.infer_map_tys(elements)
            };
            let map_ty = self.ctx.t_map(kt.clone(), vt.clone());
            let simple = elements.iter().all(|e| matches!(e, CollectionElement::MapEntry { null_aware_key: false, null_aware_value: false, .. }));
            if simple {
                let mut items = Vec::new();
                for e in elements {
                    if let CollectionElement::MapEntry { key, value, .. } = e {
                        items.push(self.emit_expr(*key, Some(&kt)).0.into_at(P_ASSIGN));
                        items.push(self.emit_expr(*value, Some(&vt)).0.into_at(P_ASSIGN));
                    }
                }
                if const_ {
                    let kr = self.rti(&kt);
                    let vr = self.rti(&vt);
                    return (Js::prim(format!("dart.constMap({kr}, {vr}, [{}])", items.join(", "))), map_ty);
                }
                let (cls, rti) = self.map_impl(&kt, &vt);
                if items.is_empty() {
                    return (Js::prim(format!("new {cls}.new({rti})")), map_ty);
                }
                return (Js::prim(format!("new {cls}.from({rti}, [{}])", items.join(", "))), map_ty);
            }
            let (cls, rti) = self.map_impl(&kt, &vt);
            let t = self.temp();
            let saved_w = std::mem::take(&mut self.w);
            if const_ {
                crate::linha!(self.w, "{t} = [];");
            } else {
                crate::linha!(self.w, "{t} = new {cls}.new({rti});");
            }
            let set = self.member_access(&map_ty, "[]=", false);
            let t2 = t.clone();
            for el in elements {
                self.emit_collection_element(el, &kt, Some(&vt), &|s: &mut Self, v: String| {
                    let (k, val) = v.split_once('\u{0}').unwrap_or((&v, "null"));
                    if const_ {
                        crate::linha!(s.w, "{t2}.push({k}, {val});");
                    } else {
                        crate::linha!(s.w, "{t2}{set}({k}, {val});");
                    }
                });
            }
            if const_ {
                let kr = self.rti(&kt);
                let vr = self.rti(&vt);
                crate::linha!(self.w, "return dart.constMap({kr}, {vr}, {t});");
            } else {
                crate::linha!(self.w, "return {t};");
            }
            let body = std::mem::replace(&mut self.w, saved_w).out;
            return (self.iife(&body), map_ty);
        }
        // Set
        let et = if let Some(t) = type_args.first() {
            self.resolve_type(*t)
        } else if let Some(t) = self.expected_arg(expected, self.ctx.set_, 0).or_else(|| self.expected_arg(expected, self.ctx.iterable_, 0)) {
            t
        } else {
            self.infer_elements_ty(elements)
        };
        let set_ty = self.ctx.t_set(et.clone());
        self.m.use_sdk("collection");
        let lhs = self.ctx.linked_hash_set.map(|c| Ty::iface_args(c, vec![et.clone()])).unwrap_or(set_ty.clone());
        let rti = self.rti(&lhs);
        let simple = elements.iter().all(|e| matches!(e, CollectionElement::Expression(_)));
        if simple {
            let items: Vec<String> = elements
                .iter()
                .map(|e| match e {
                    CollectionElement::Expression(x) => self.emit_expr(*x, Some(&et)).0.into_at(P_ASSIGN),
                    _ => unreachable!(),
                })
                .collect();
            if const_ {
                let er = self.rti(&et);
                return (Js::prim(format!("dart.constSet({er}, [{}])", items.join(", "))), set_ty);
            }
            if items.is_empty() {
                return (Js::prim(format!("collection.LinkedHashSet.new({rti})")), set_ty);
            }
            return (Js::prim(format!("collection.LinkedHashSet.from({rti}, [{}])", items.join(", "))), set_ty);
        }
        let t = self.temp();
        let saved_w = std::mem::take(&mut self.w);
        if const_ {
            crate::linha!(self.w, "{t} = [];");
        } else {
            crate::linha!(self.w, "{t} = collection.LinkedHashSet.new({rti});");
        }
        let t2 = t.clone();
        for el in elements {
            self.emit_collection_element(el, &et, None, &|s: &mut Self, v: String| {
                if const_ {
                    crate::linha!(s.w, "{t2}.push({v});");
                } else {
                    crate::linha!(s.w, "{t2}.add({v});");
                }
            });
        }
        if const_ {
            let er = self.rti(&et);
            crate::linha!(self.w, "return dart.constSet({er}, {t});");
        } else {
            crate::linha!(self.w, "return {t};");
        }
        let body = std::mem::replace(&mut self.w, saved_w).out;
        (self.iife(&body), set_ty)
    }

    /// Marca se o elemento indica mapa (entrada) ou conjunto (expressão).
    fn classify_element(&mut self, e: &CollectionElement, any_map: &mut bool, any_set: &mut bool) {
        match e {
            CollectionElement::MapEntry { .. } => *any_map = true,
            CollectionElement::Expression(_) | CollectionElement::NullAwareExpression(_) => *any_set = true,
            CollectionElement::Spread { value, .. } => {
                let t = self.type_of(*value);
                if self.ctx.map_.is_some_and(|m| self.ctx.as_super(&t.non_null(), m).is_some()) {
                    *any_map = true;
                } else if !t.is_dynamic() {
                    *any_set = true;
                }
            }
            CollectionElement::If { then, else_, .. } => {
                self.classify_element(then, any_map, any_set);
                if let Some(e) = else_ {
                    self.classify_element(e, any_map, any_set);
                }
            }
            CollectionElement::For { body, .. } | CollectionElement::ForIn { body, .. } => {
                self.classify_element(body, any_map, any_set);
            }
        }
    }

    fn infer_map_tys(&mut self, elements: &[CollectionElement]) -> (Ty, Ty) {
        let mut ks = Vec::new();
        let mut vs = Vec::new();
        for el in elements {
            match el {
                CollectionElement::MapEntry { key, value, null_aware_key, null_aware_value } => {
                    // Parte null-aware entra sem o `null` (spec 3.8, NonNull).
                    let k = self.type_of(*key);
                    let v = self.type_of(*value);
                    ks.push(if *null_aware_key { k.non_null() } else { k });
                    vs.push(if *null_aware_value { v.non_null() } else { v });
                }
                CollectionElement::Spread { value, .. } => {
                    let t = self.type_of(*value);
                    if let Some(mt) = self.ctx.map_.and_then(|m| self.ctx.as_super(&t.non_null(), m)) {
                        ks.push(mt.args().first().cloned().unwrap_or(Ty::Dynamic));
                        vs.push(mt.args().get(1).cloned().unwrap_or(Ty::Dynamic));
                    }
                }
                CollectionElement::If { then, else_, .. } => {
                    let (k, v) = self.infer_map_tys(std::slice::from_ref(then));
                    if !k.is_dynamic() {
                        ks.push(k);
                        vs.push(v);
                    }
                    if let Some(e) = else_ {
                        let (k, v) = self.infer_map_tys(std::slice::from_ref(e));
                        if !k.is_dynamic() {
                            ks.push(k);
                            vs.push(v);
                        }
                    }
                }
                CollectionElement::For { body, .. } | CollectionElement::ForIn { body, .. } => {
                    self.push_scope();
                    if let CollectionElement::ForIn { target: ast::ForInTarget::Declared { name, ty, .. }, iterable, .. } = el {
                        let it = self.type_of(*iterable);
                        let et = ty.map(|t| self.resolve_type(t)).unwrap_or_else(|| {
                            self.ctx.iterable_.and_then(|i| self.ctx.as_super(&it, i)).and_then(|t| t.args().first().cloned()).unwrap_or(Ty::Dynamic)
                        });
                        self.declare(name.sym, et);
                    }
                    if let CollectionElement::For { init: Some(ast::ForInit::Variables(list)), .. } = el {
                        let declared = list.ty.map(|t| self.resolve_type(t));
                        for v in list.variables.iter() {
                            let t = match (declared.clone(), v.initializer) {
                                (Some(d), _) => d,
                                (None, Some(i)) => self.type_of(i),
                                _ => Ty::Dynamic,
                            };
                            self.declare(v.name.sym, t);
                        }
                    }
                    let (k, v) = self.infer_map_tys(std::slice::from_ref(body));
                    self.pop_scope();
                    if !k.is_dynamic() {
                        ks.push(k);
                        vs.push(v);
                    }
                }
                _ => {}
            }
        }
        let fold = |s: &Self, v: Vec<Ty>| -> Ty {
            let mut it = v.into_iter();
            match it.next() {
                None => Ty::Dynamic,
                Some(f) => it.fold(f, |a, b| s.ctx.lub(&a, &b)),
            }
        };
        (fold(self, ks), fold(self, vs))
    }

    /// Classe de implementação de mapa (`IdentityMap` para chaves primitivas) e seu rti.
    pub(crate) fn map_impl(&mut self, kt: &Ty, vt: &Ty) -> (String, String) {
        self.m.use_sdk("_js_helper");
        let identity = self.ctx.is_js_primitive(kt);
        let name = if identity { "IdentityMap" } else { "LinkedMap" };
        let kr = self.recipe_plain(kt);
        let vr = self.recipe_plain(vt);
        let _ = (kr, vr);
        let cls = format!("_js_helper.{name}");
        // Rti: `_js_helper|IdentityMap<K,V>`.
        let lib = self.ctx.program.libraries.iter().position(|l| l.uri == "dart:_js_helper");
        let rti = match lib.and_then(|i| {
            let sym = self.ctx.sym(name)?;
            match self.ctx.program.libraries[i].declared.get(&sym)?.getter {
                Some(Element::Class(c)) => Some(c),
                _ => None,
            }
        }) {
            Some(c) => self.rti(&Ty::iface_args(c, vec![kt.clone(), vt.clone()])),
            None => self.rti(&self.ctx.t_map(kt.clone(), vt.clone())),
        };
        (cls, rti)
    }

    pub fn recipe_plain(&self, t: &Ty) -> String {
        let _ = t;
        String::new()
    }

    fn emit_record(&mut self, positional: &[ExprId], named: &[(ast::Name, ExprId)]) -> (Js, Ty) {
        let mut pos_js = Vec::new();
        let mut pos_ty = Vec::new();
        for e in positional {
            let (j, t) = self.emit_expr(*e, None);
            pos_js.push(j.into_at(P_ASSIGN));
            pos_ty.push(t);
        }
        let mut named_items: Vec<(String, String, Ty)> = Vec::new();
        for (n, e) in named {
            let (j, t) = self.emit_expr(*e, None);
            named_items.push((self.name(n.sym).to_string(), j.into_at(P_ASSIGN), t));
        }
        // Ordem de avaliação preservada por temps quando há nomeados fora de ordem.
        let mut sorted = named_items.clone();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        let in_order = named_items.iter().map(|x| &x.0).eq(sorted.iter().map(|x| &x.0));
        // Nomeados antes de algum posicional na fonte? Então a ordem de avaliação exige temps.
        let mut interleaved = false;
        let mut seen_named = false;
        for (n, e) in named.iter().map(|(n, e)| (Some(n), *e)).chain(std::iter::empty()) {
            let _ = (n, e);
        }
        {
            let mut named_spans: Vec<usize> = named.iter().map(|(_, e)| self.expr(*e).span.start).collect();
            let pos_spans: Vec<usize> = positional.iter().map(|e| self.expr(*e).span.start).collect();
            named_spans.sort();
            if let (Some(&first_named), Some(&last_pos)) = (named_spans.first(), pos_spans.last()) {
                if first_named < last_pos {
                    interleaved = true;
                }
            }
            let _ = &mut seen_named;
        }
        let mut prefix = String::new();
        let mut pos_js = pos_js;
        let sorted_vals: Vec<String> = if (in_order && !interleaved) || (named_items.is_empty()) {
            sorted.iter().map(|x| x.1.clone()).collect()
        } else {
            // Avalia tudo na ordem da fonte em temps.
            let mut temps: HashMap<String, String> = HashMap::new();
            let mut order: Vec<(usize, Option<String>, String)> = Vec::new();
            for (i, e) in positional.iter().enumerate() {
                order.push((self.expr(*e).span.start, None, pos_js[i].clone()));
            }
            for ((n, e), (_, v, _)) in named.iter().zip(named_items.iter()) {
                order.push((self.expr(*e).span.start, Some(self.name(n.sym).to_string()), v.clone()));
            }
            order.sort_by_key(|x| x.0);
            let mut new_pos: Vec<String> = Vec::new();
            for (_, name, v) in order {
                let t = self.temp();
                prefix.push_str(&format!("{t} = {v}, "));
                match name {
                    Some(n) => {
                        temps.insert(n, t);
                    }
                    None => new_pos.push(t),
                }
            }
            pos_js = new_pos;
            sorted.iter().map(|x| temps[&x.0].clone()).collect()
        };
        let names: Vec<String> = sorted.iter().map(|x| x.0.clone()).collect();
        let shape = format!("{};{}", pos_js.len() + names.len(), names.join(","));
        let named_js = if names.is_empty() {
            "void 0".to_string()
        } else {
            format!("[{}]", names.iter().map(|n| js::string_literal(n)).collect::<Vec<_>>().join(", "))
        };
        let mut values = pos_js.clone();
        values.extend(sorted_vals);
        let mut code = format!("dart.recordLiteral({}, {}, {named_js}, [{}])", js::string_literal(&shape), pos_js.len(), values.join(", "));
        if self.in_const && prefix.is_empty() {
            let lv = self.lib_var(self.lib);
            code = format!("{lv}.$C({}, () => {code})", js::string_literal(&code));
        }
        let ty = Ty::Record { pos: pos_ty, named: sorted.into_iter().map(|(n, _, t)| (n, t)).collect(), nullable: false };
        if prefix.is_empty() {
            (Js::prim(code), ty)
        } else {
            (Js::new(format!("{prefix}{code}"), P_COMMA).paren(), ty)
        }
    }

    // -----------------------------------------------------------------------
    // Identificadores e acesso a membros
    // -----------------------------------------------------------------------

    /// Valor de um identificador (sem nó de expressão).
    pub fn emit_identifier_value(&mut self, sym: dartforge_intern::SymbolId) -> (Js, Ty) {
        self.emit_identifier(sym, ExprId(u32::MAX))
    }

    fn emit_identifier(&mut self, sym: dartforge_intern::SymbolId, _e: ExprId) -> (Js, Ty) {
        let n = self.name(sym).to_string();
        match self.resolve_ident(sym) {
            IdentTarget::Local(js, ty) => {
                if let Some(l) = self.lookup_local(sym).cloned() {
                    if let Some(init) = &l.lazy_init {
                        return (Js::new(format!("{js} === void 0 ? {js} = {init} : {js}", ), P_COND).paren(), ty);
                    }
                    if l.late_check {
                        self.m.use_sdk("_internal");
                        return (Js::new(format!("{js} === void 0 ? dart.throw(new _internal.LateError.localNI({})) : {js}", js::string_literal(&n)), P_COND).paren(), ty);
                    }
                }
                (Js::prim(js), ty)
            }
            IdentTarget::ThisMember(m) => self.emit_member_get(&Js::prim("this"), &self.class.map(|c| self.ctx.this_ty(c)).unwrap_or(Ty::Dynamic), &n, Some(m)),
            IdentTarget::Static(c, mk) => self.emit_static_get(c, &n, mk),
            IdentTarget::Element(el) => self.emit_element_get(el, &n),
            IdentTarget::TypeParam(t) => (Js::prim(format!("dart_rti.createRuntimeType({})", self.rti(&t))), self.ctx.t_type()),
            IdentTarget::ExtThisMember(_) => {
                let t = self.extension_this.clone().unwrap_or(Ty::Dynamic);
                self.emit_member_get(&Js::prim("$this"), &t, &n, None)
            }
            IdentTarget::ThisExt => {
                let t = self.class.map(|c| self.ctx.this_ty(c)).unwrap_or(Ty::Dynamic);
                self.emit_member_get(&Js::prim("this"), &t, &n, None)
            }
            IdentTarget::ExtField(ext) => {
                let e = self.ctx.program.extension(ext);
                let ext_name = self.extension_js_name(ext);
                let lib_var = self.lib_var(e.library);
                let ty = e.fields.iter().find(|v| self.ctx.program.variable(**v).name == sym).map(|v| self.ctx.var_ty(*v)).unwrap_or(Ty::Dynamic);
                (Js::prim(format!("{lib_var}[{}]", js::string_literal(&format!("{ext_name}|{n}")))), ty)
            }
            IdentTarget::ExtStatic(ext, fid) => {
                let e = self.ctx.program.extension(ext);
                let ext_name = self.extension_js_name(ext);
                let lib_var = self.lib_var(e.library);
                let f = self.ctx.program.function(fid);
                let js = format!("{lib_var}[{}]", js::string_literal(&format!("{ext_name}|{n}")));
                if f.kind == FunctionKind::Getter {
                    return (Js::prim(format!("{js}()")), self.ctx.ty_of(self.ctx.outline.functions[fid.0 as usize].return_type));
                }
                let ty = self.ctx.fn_ty(fid);
                (self.tearoff_static(&js, &ty), ty)
            }
            IdentTarget::ExtMember(ext, _) => {
                let t = self.extension_this.clone().unwrap_or(Ty::Dynamic);
                let _ = ext;
                match self.try_extension_get(&Js::prim("$this"), &t, &n) {
                    Some(r) => r,
                    None => (Js::prim("null"), Ty::Dynamic),
                }
            }
            IdentTarget::Prefix(_) => (Js::prim("null"), Ty::Dynamic),
            IdentTarget::Unknown => {
                // Literais de tipo especiais.
                let special = match n.as_str() {
                    "dynamic" => Some(Ty::Dynamic),
                    "Never" => Some(Ty::Never),
                    _ => None,
                };
                if let Some(t) = special {
                    return (Js::prim(format!("dart_rti.createRuntimeType({})", self.rti(&t))), self.ctx.t_type());
                }
                (Js::prim(js::ident(&n)), Ty::Dynamic)
            }
        }
    }

    pub fn emit_element_get(&mut self, el: Element, n: &str) -> (Js, Ty) {
        match el {
            Element::Function(fid) => {
                let (js, ty) = self.element_ref(el).expect("função");
                // Tearoff de função de topo.
                let f = self.ctx.program.function(fid);
                if self.ctx.is_js_member(fid) {
                    return match f.kind {
                        FunctionKind::Getter => (self.js_null_check(Js::prim(js), &MemberKind::Getter(fid)), self.ctx.ty_of(self.ctx.outline.functions[fid.0 as usize].return_type)),
                        FunctionKind::Setter => (Js::prim(js), Ty::Dynamic),
                        _ => (Js::prim(format!("dart.tearoffInterop({js}, {})", self.ctx.js_null_checkable(&MemberKind::Method(fid)))), ty),
                    };
                }
                match f.kind {
                    FunctionKind::Getter => (Js::prim(js), self.ctx.ty_of(self.ctx.outline.functions[fid.0 as usize].return_type)),
                    FunctionKind::Setter => (Js::prim(js), Ty::Dynamic),
                    _ => (self.tearoff_static(&js, &ty), ty),
                }
            }
            Element::Variable(vid) => {
                let (js, ty) = self.element_ref(el).expect("variável");
                if self.ctx.is_js_var(vid) {
                    return (self.js_null_check(Js::prim(js), &MemberKind::Field(vid)), ty);
                }
                (Js::prim(js), ty)
            }
            Element::Class(c) => {
                let rti = self.rti(&self.ctx.this_ty_default(c));
                (Js::prim(format!("dart_rti.createRuntimeType({rti})")), self.ctx.t_type())
            }
            _ => (Js::prim(js::ident(n)), Ty::Dynamic),
        }
    }

    /// Tearoff de função estática/topo: `dart.fn(f, rti)` ou `dart.gFn`.
    pub fn tearoff_static(&mut self, js: &str, ty: &Ty) -> Js {
        match ty {
            Ty::Fn { type_params, .. } if !type_params.is_empty() => {
                let rti = self.rti(ty);
                let defaults: Vec<String> = type_params.iter().map(|p| self.rti(&self.default_type_arg(&p.bound))).collect();
                Js::prim(format!("dart.gFn({js}, {rti}, dart.constList(dart_rti._Universe.eval(dart_rti._theUniverse(), \"@\", true), [{}]))", defaults.join(", ")))
            }
            Ty::Fn { .. } => {
                let rti = self.rti(ty);
                Js::prim(format!("dart.fn({js}, {rti})"))
            }
            _ => Js::prim(js.to_string()),
        }
    }

    pub fn default_type_arg(&self, bound: &Ty) -> Ty {
        if bound.mentions_params() {
            Ty::Dynamic
        } else if let Ty::Iface { class, nullable: true, .. } = bound {
            if Some(*class) == self.ctx.object {
                return Ty::Dynamic;
            }
            bound.clone()
        } else {
            bound.clone()
        }
    }

    pub fn emit_static_get(&mut self, c: ClassId, n: &str, mk: MemberKind) -> (Js, Ty) {
        if self.ctx.is_js_class(c) && self.ctx.is_js_member_kind(&mk) {
            let js = self.ctx.js_static_ref(Some(c), self.ctx.lib_of_class(c), &mk, n);
            return match mk {
                MemberKind::Method(fid) => (Js::prim(format!("dart.tearoffInterop({js}, {})", self.ctx.js_null_checkable(&mk))), self.ctx.fn_ty(fid)),
                MemberKind::Getter(fid) => (self.js_null_check(Js::prim(js), &mk), self.ctx.ty_of(self.ctx.outline.functions[fid.0 as usize].return_type)),
                MemberKind::Setter(_) => (Js::prim(js), Ty::Dynamic),
                MemberKind::Field(vid) => (self.js_null_check(Js::prim(js), &mk), self.ctx.var_ty(vid)),
            };
        }
        let cls = self.class_ref(c);
        let js = format!("{cls}{}", js::prop_access(&static_member_name(n)));
        match mk {
            MemberKind::Method(fid) => {
                let ty = self.ctx.fn_ty(fid);
                (self.tearoff_static(&js, &ty), ty)
            }
            MemberKind::Getter(fid) => (Js::prim(js), self.ctx.ty_of(self.ctx.outline.functions[fid.0 as usize].return_type)),
            MemberKind::Setter(_) => (Js::prim(js), Ty::Dynamic),
            MemberKind::Field(vid) => (Js::prim(js), self.ctx.var_ty(vid)),
        }
    }

    /// `recv.name` (getter, campo, ou tearoff de método).
    pub fn emit_member_get(&mut self, recv: &Js, recv_ty: &Ty, name: &str, member: Option<Member>) -> (Js, Ty) {
        if self.forced_ext.is_some() {
            if let Some(r) = self.try_extension_get(recv, recv_ty, name) {
                return r;
            }
            self.forced_ext = None;
        }
        let member = member.or_else(|| self.ctx.lookup_member(recv_ty, name, false));
        // Membros de Object com helpers.
        let recv_nn = recv_ty.non_null();
        match name {
            "hashCode" if !self.is_user_class_recv(&recv_nn) => {
                return (Js::prim(format!("dart.hashCode({})", recv.code)), self.ctx.t_int());
            }
            "runtimeType" if !self.is_user_class_recv(&recv_nn) => {
                return (Js::prim(format!("dart.runtimeType({})", recv.code)), self.ctx.t_type());
            }
            "toString" | "noSuchMethod" if !self.is_user_class_recv(&recv_nn) => {
                let ty = member.as_ref().map(|m| self.ctx.member_ty(m)).unwrap_or(Ty::Dynamic);
                return (Js::prim(format!("dart.{}Tearoff({})", name, recv.code)), ty);
            }
            _ => {}
        }
        if recv_ty.is_dynamic() && member.is_none() {
            return (Js::prim(format!("dart.dload({}, {})", recv.code, js::string_literal(&crate::body::js_member_name(name)))), Ty::Dynamic);
        }
        let Some(m) = member else {
            // Extensão?
            if let Some(r) = self.try_extension_get(recv, recv_ty, name) {
                return r;
            }
            if let Ty::Fn { .. } = recv_nn {
                if name == "call" {
                    return (recv.clone(), recv_ty.clone());
                }
            }
            if let Ty::Record { pos, named, .. } = &recv_nn {
                if let Some(idx) = name.strip_prefix('$').and_then(|s| s.parse::<usize>().ok()) {
                    if idx >= 1 && idx <= pos.len() {
                        return (Js::prim(format!("{}.${idx}", recv.at(P_PRIMARY))), pos[idx - 1].clone());
                    }
                }
                if let Some((_, t)) = named.iter().find(|(n, _)| n == name) {
                    return (Js::prim(format!("{}{}", recv.at(P_PRIMARY), js::prop_access(name))), t.clone());
                }
            }
            return (Js::prim(format!("dart.dload({}, {})", recv.code, js::string_literal(&crate::body::js_member_name(name)))), Ty::Dynamic);
        };
        let ty = self.ctx.member_ty(&m);
        let access = self.member_access(recv_ty, name, false);
        let js = format!("{}{access}", recv.at(P_PRIMARY));
        if self.ctx.is_js_member_kind(&m.kind) {
            // Interop: propriedade direta; `tearoffInterop` para métodos.
            return match m.kind {
                MemberKind::Method(_) => (Js::prim(format!("dart.tearoffInterop({js}, {})", self.ctx.js_null_checkable(&m.kind))), ty),
                _ => (self.js_null_check(Js::prim(js), &m.kind), ty),
            };
        }
        match m.kind {
            MemberKind::Method(_) => {
                // Tearoff de método de instância.
                let key = if name.starts_with('_') {
                    self.private_sym(self.ctx.lib_of_class(m.class), name)
                } else if self.ctx.is_ext_member(recv_ty, name, false) {
                    self.dartx(&js_member_name(name))
                } else {
                    js::string_literal(&js_member_name(name))
                };
                (Js::prim(format!("dart.bind({}, {key})", recv.code)), ty)
            }
            _ => (Js::prim(js), ty),
        }
    }

    fn is_user_class_recv(&self, t: &Ty) -> bool {
        match t {
            Ty::Iface { class, nullable: false, .. } => {
                let lib = self.ctx.lib_of_class(*class);
                !self.ctx.libs[lib.0 as usize].is_sdk
            }
            _ => false,
        }
    }

    /// Getter de extensão aplicável.
    pub fn try_extension_get(&mut self, recv: &Js, recv_ty: &Ty, name: &str) -> Option<(Js, Ty)> {
        let found = self.find_extension_member(recv_ty, name, false);
        self.forced_ext = None;
        let (ext, fid, subst) = found?;
        let f = self.ctx.program.function(fid);
        let e = self.ctx.program.extension(ext);
        let ext_name = self.extension_js_name(ext);
        let lib_var = self.lib_var(e.library);
        let ty = self.ctx.fn_ty(fid).subst_prop(&subst);
        let targs: Vec<String> = self.ctx.outline.extensions[ext.0 as usize]
            .type_params
            .iter()
            .map(|p| self.rti(subst.get(&p.0).unwrap_or(&Ty::Dynamic)))
            .collect();
        let mut args = targs;
        args.push(recv.code.clone());
        match f.kind {
            FunctionKind::Getter => {
                let ret = match &ty {
                    Ty::Fn { ret, .. } => (**ret).clone(),
                    _ => Ty::Dynamic,
                };
                Some((Js::prim(format!("{lib_var}[{}]({})", js::string_literal(&format!("{ext_name}|get#{name}")), args.join(", "))), ret))
            }
            FunctionKind::Function | FunctionKind::Operator => {
                // Tearoff de método de extensão: closure.
                let (pos_n, ..) = match &ty {
                    Ty::Fn { pos, .. } => (pos.len(), ()),
                    _ => (0, ()),
                };
                let params: Vec<String> = (0..pos_n).map(|i| format!("a{i}")).collect();
                let mut call_args = args.clone();
                call_args.extend(params.iter().cloned());
                let rti = self.rti(&ty);
                Some((
                    Js::prim(format!(
                        "dart.fn(({}) => {lib_var}[{}]({}), {rti})",
                        params.join(", "),
                        js::string_literal(&format!("{ext_name}|{name}")),
                        call_args.join(", ")
                    )),
                    ty,
                ))
            }
            _ => None,
        }
    }

    /// Rtis dos parâmetros de tipo da extensão (na ordem), a partir da substituição.
    pub fn ext_type_args(&mut self, ext: dartforge_elements::model::ExtensionId, subst: &HashMap<u32, Ty>) -> Vec<String> {
        self.ctx.outline.extensions[ext.0 as usize]
            .type_params
            .iter()
            .map(|p| self.rti(subst.get(&p.0).unwrap_or(&Ty::Dynamic)))
            .collect()
    }

    pub fn extension_js_name(&self, ext: dartforge_elements::model::ExtensionId) -> String {
        let e = self.ctx.program.extension(ext);
        match e.name {
            Some(n) => self.name(n).to_string(),
            None => {
                // Índice entre as extensões sem nome da biblioteca.
                let mut idx = 0;
                for (i, x) in self.ctx.program.extensions.iter().enumerate() {
                    if x.library == e.library && x.name.is_none() {
                        if i == ext.0 as usize {
                            break;
                        }
                        idx += 1;
                    }
                }
                format!("_extension#{idx}")
            }
        }
    }

    /// Procura membro de extensão aplicável ao tipo do receptor.
    pub fn find_extension_member(
        &self,
        recv_ty: &Ty,
        name: &str,
        setter: bool,
    ) -> Option<(dartforge_elements::model::ExtensionId, dartforge_elements::model::FunctionElementId, HashMap<u32, Ty>)> {
        let key = if setter { format!("{name}_=") } else { name.to_string() };
        let sym = self.ctx.sym(&key)?;
        if let Some(forced) = self.forced_ext {
            let e = self.ctx.program.extension(forced);
            let fid = *e.instance_members.get(&sym)?;
            let data = &self.ctx.outline.extensions[forced.0 as usize];
            let on = self.ctx.ty_of(data.on);
            let mut subst = HashMap::new();
            let params: Vec<u32> = data.type_params.iter().map(|p| p.0).collect();
            self.match_type(&on, recv_ty, &params, &mut subst);
            return Some((forced, fid, subst));
        }
        let recv_nn = if recv_ty.is_nullable() { recv_ty.clone() } else { recv_ty.clone() };
        let mut best: Option<(dartforge_elements::model::ExtensionId, dartforge_elements::model::FunctionElementId, HashMap<u32, Ty>, u32)> = None;
        for ext in self.ctx.visible_extensions(self.lib) {
            let e = self.ctx.program.extension(ext);
            let Some(&fid) = e.instance_members.get(&sym) else { continue };
            let data = &self.ctx.outline.extensions[ext.0 as usize];
            let on = self.ctx.ty_of(data.on);
            let mut subst = HashMap::new();
            // Liga parâmetros da extensão comparando com o receptor.
            let params: Vec<u32> = data.type_params.iter().map(|p| p.0).collect();
            let ok = self.match_type(&on, &recv_nn, &params, &mut subst);
            if !ok {
                continue;
            }
            let on_s = on.subst(&subst);
            if !self.ctx.is_subtype(&recv_nn, &on_s) && !recv_nn.is_dynamic() {
                if !(recv_nn.is_nullable() && self.ctx.is_subtype(&recv_nn.non_null(), &on_s) && !on_s.is_nullable()) {
                    continue;
                } else {
                    continue;
                }
            }
            // Especificidade: `on` mais específico vence; empate → biblioteca própria.
            let score = if e.library == self.lib { 1 } else { 0 };
            let better = match &best {
                None => true,
                Some((bext, _, bsubst, bscore)) => {
                    let bon = self.ctx.ty_of(self.ctx.outline.extensions[bext.0 as usize].on).subst(bsubst);
                    let a_more = self.ctx.is_subtype(&on_s, &bon) && !self.ctx.is_subtype(&bon, &on_s);
                    let b_more = self.ctx.is_subtype(&bon, &on_s) && !self.ctx.is_subtype(&on_s, &bon);
                    a_more || (!b_more && score > *bscore)
                }
            };
            if better {
                best = Some((ext, fid, subst, score));
            }
        }
        best.map(|(a, b, c, _)| (a, b, c))
    }

    /// Casa `pattern` (com parâmetros livres `params`) contra `actual`, preenchendo `subst`.
    pub fn match_type(&self, pattern: &Ty, actual: &Ty, params: &[u32], subst: &mut HashMap<u32, Ty>) -> bool {
        match pattern {
            Ty::Param { id, .. } if params.contains(id) => {
                if matches!(actual, Ty::Dynamic | Ty::Never) {
                    return true;
                }
                if let Some(prev) = subst.get(id) {
                    let l = self.ctx.lub(prev, actual);
                    subst.insert(*id, l);
                } else {
                    subst.insert(*id, actual.non_null_if(pattern.is_nullable()));
                }
                true
            }
            Ty::Iface { class, args, .. } => {
                let actual_r = self.ctx.resolve_param_bound(actual);
                let Some(sup) = self.ctx.as_super(&actual_r.non_null(), *class) else {
                    return matches!(actual, Ty::Dynamic | Ty::Never | Ty::Null);
                };
                for (p, a) in args.iter().zip(sup.args().iter()) {
                    self.match_type(p, a, params, subst);
                }
                true
            }
            Ty::Fn { ret, pos, opt, named, .. } => {
                if let Ty::Fn { ret: r2, pos: p2, opt: o2, named: n2, .. } = actual {
                    self.match_type(ret, r2, params, subst);
                    for (a, b) in pos.iter().chain(opt.iter()).zip(p2.iter().chain(o2.iter())) {
                        self.match_type(a, b, params, subst);
                    }
                    for (n, t, _) in named {
                        if let Some((_, t2, _)) = n2.iter().find(|(m, _, _)| m == n) {
                            self.match_type(t, t2, params, subst);
                        }
                    }
                }
                true
            }
            Ty::FutureOr { arg, .. } => {
                if let Ty::FutureOr { arg: a2, .. } = actual {
                    return self.match_type(arg, a2, params, subst);
                }
                if let Some(ft) = self.ctx.future_.and_then(|f| self.ctx.as_super(&actual.non_null(), f)) {
                    return self.match_type(arg, ft.args().first().unwrap_or(&Ty::Dynamic), params, subst);
                }
                self.match_type(arg, actual, params, subst)
            }
            Ty::Record { pos, named, .. } => {
                if let Ty::Record { pos: p2, named: n2, .. } = actual {
                    for (a, b) in pos.iter().zip(p2.iter()) {
                        self.match_type(a, b, params, subst);
                    }
                    for (n, t) in named {
                        if let Some((_, t2)) = n2.iter().find(|(m, _)| m == n) {
                            self.match_type(t, t2, params, subst);
                        }
                    }
                }
                true
            }
            _ => true,
        }
    }

    // -----------------------------------------------------------------------
    // Seletores (com null-shorting)
    // -----------------------------------------------------------------------

    /// Emite um seletor (`a.b`, `a[i]`, `f(x)`) devolvendo o texto sem as
    /// guardas de `?.`, o tipo e as guardas acumuladas.
    pub fn emit_selector(&mut self, e: ExprId, expected: Option<&Ty>) -> (Js, Ty, Vec<Guard>) {
        let expr = self.expr(e);
        match &expr.kind {
            ExprKind::Property { target, name, null_aware } => {
                let n = self.name(name.sym).to_string();
                // Casos estáticos: prefixo, classe, enum.
                if let Some(r) = self.try_static_property(*target, &n) {
                    return (r.0, r.1, vec![]);
                }
                if let ExprKind::Super = self.expr(*target).kind {
                    let sup_ty = self.super_ty();
                    let m = self.ctx.lookup_member(&sup_ty, &n, false);
                    let ty = m.as_ref().map(|m| self.ctx.member_ty(m)).unwrap_or(Ty::Dynamic);
                    let access = self.member_access(&sup_ty, &n, false);
                    return (Js::prim(format!("{}{access}", self.super_ref())), ty, vec![]);
                }
                let (tjs, tty, mut guards) = self.emit_target(*target);
                let (recv, recv_ty) = if *null_aware {
                    let t = self.temp();
                    guards.push(Guard { temp: t.clone(), init: tjs.code });
                    (Js::prim(t), tty.non_null())
                } else {
                    (tjs, tty)
                };
                let (js, ty) = self.emit_member_get(&recv, &recv_ty, &n, None);
                let ty = if *null_aware { ty.with_nullable(true) } else { ty };
                (js, ty, guards)
            }
            ExprKind::Index { target, index, null_aware } => {
                let (tjs, tty, mut guards) = self.emit_target(*target);
                let (recv, recv_ty) = if *null_aware {
                    let t = self.temp();
                    guards.push(Guard { temp: t.clone(), init: tjs.code });
                    (Js::prim(t), tty.non_null())
                } else {
                    (tjs, tty)
                };
                let (ijs, _) = self.emit_expr(*index, None);
                let (js, ty) = self.emit_index_get(&recv, &recv_ty, &ijs);
                let ty = if *null_aware { ty.with_nullable(true) } else { ty };
                (js, ty, guards)
            }
            ExprKind::Call { target, arguments } => self.emit_call(*target, arguments, expected),
            _ => {
                let (js, ty) = self.emit_expr(e, expected);
                (js, ty, vec![])
            }
        }
    }

    /// Tipo da superclasse (Object quando não há `extends`).
    pub fn super_ty(&self) -> Ty {
        self.class
            .and_then(|c| self.ctx.direct_supers(&self.ctx.this_ty(c)).first().cloned())
            .filter(|t| matches!(t, Ty::Iface { .. }))
            .or_else(|| self.ctx.object.map(Ty::iface))
            .unwrap_or(Ty::Dynamic)
    }

    pub fn super_ref(&self) -> String {
        if self.async_kind == AsyncKind::None {
            "super".to_string()
        } else {
            match self.class {
                Some(c) => format!("Object.getPrototypeOf({}.prototype)", self.class_ref(c)),
                None => "super".to_string(),
            }
        }
    }

    /// Alvo de um seletor: se também for seletor, propaga guardas.
    pub fn emit_target(&mut self, target: ExprId) -> (Js, Ty, Vec<Guard>) {
        let t = self.expr(target);
        // `Ext(x).membro`: aplicação explícita de extensão.
        if let ExprKind::Call { target: ct, arguments } = &t.kind {
            if let ExprKind::Identifier(id) = &self.expr(*ct).kind {
                if let IdentTarget::Element(Element::Extension(ext)) = self.resolve_ident(id.sym) {
                    if let Some(a) = arguments.args.first() {
                        let (js, ty) = self.emit_expr(a.value, None);
                        self.forced_ext = Some(ext);
                        return (js, ty, vec![]);
                    }
                }
            }
        }
        match &t.kind {
            ExprKind::Property { .. } | ExprKind::Index { .. } | ExprKind::Call { .. } => self.emit_selector(target, None),
            _ => {
                let (js, ty) = self.emit_expr(target, None);
                (js, ty, vec![])
            }
        }
    }

    /// `prefix.x`, `Class.static`, `Enum.value`, `prefix.Class.static`.
    pub fn try_static_property(&mut self, target: ExprId, name: &str) -> Option<(Js, Ty)> {
        let t = self.expr(target);
        match &t.kind {
            ExprKind::Identifier(id) => match self.resolve_ident(id.sym) {
                IdentTarget::Prefix(p) => {
                    if name == "loadLibrary" {
                        return None;
                    }
                    let sym = self.ctx.sym(name)?;
                    let b = self.ctx.program.lookup_prefixed(self.lib, p, sym)?;
                    let el = b.getter.or(b.setter)?;
                    Some(self.emit_element_get(el, name))
                }
                IdentTarget::Element(Element::Class(c)) => self.static_member_get(c, name),
                IdentTarget::Element(Element::Extension(ext)) => {
                    let e = self.ctx.program.extension(ext);
                    let sym = self.ctx.sym(name)?;
                    let ext_name = self.extension_js_name(ext);
                    let lib_var = self.lib_var(e.library);
                    if let Some(&vid) = e.fields.iter().find(|v| self.ctx.program.variable(**v).name == sym) {
                        return Some((Js::prim(format!("{lib_var}[{}]", js::string_literal(&format!("{ext_name}|{name}")))), self.ctx.var_ty(vid)));
                    }
                    if let Some(&fid) = e.static_members.get(&sym) {
                        let f = self.ctx.program.function(fid);
                        let js = format!("{lib_var}[{}]", js::string_literal(&format!("{ext_name}|{name}")));
                        if f.kind == FunctionKind::Getter {
                            return Some((Js::prim(format!("{js}()")), self.ctx.ty_of(self.ctx.outline.functions[fid.0 as usize].return_type)));
                        }
                        let ty = self.ctx.fn_ty(fid);
                        return Some((self.tearoff_static(&js, &ty), ty));
                    }
                    None
                }
                IdentTarget::TypeParam(_) => None,
                _ => None,
            },
            ExprKind::Property { target: t2, name: n2, .. } => {
                // prefix.Class.member
                if let ExprKind::Identifier(id) = &self.expr(*t2).kind {
                    if let IdentTarget::Prefix(p) = self.resolve_ident(id.sym) {
                        let sym = self.ctx.sym(self.name(n2.sym))?;
                        let b = self.ctx.program.lookup_prefixed(self.lib, p, sym)?;
                        if let Some(Element::Class(c)) = b.getter {
                            return self.static_member_get(c, name);
                        }
                    }
                }
                None
            }
            ExprKind::TypeArguments { target: t2, type_args } => {
                // `C<T>.new` / `C<T>.named`: tearoff de construtor instanciado.
                if let ExprKind::Identifier(id) = &self.expr(*t2).kind {
                    if let IdentTarget::Element(Element::Class(c)) = self.resolve_ident(id.sym) {
                        let targs: Vec<Ty> = type_args.iter().map(|t| self.resolve_type(*t)).collect();
                        return self.ctor_tearoff(c, targs, name);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Tearoff de construtor `C<args>.name` como closure.
    pub fn ctor_tearoff(&mut self, c: ClassId, targs: Vec<Ty>, name: &str) -> Option<(Js, Ty)> {
        let class = self.ctx.program.class(c);
        let key = if name == "new" { self.ctx.empty_sym } else { self.ctx.sym(name) };
        let fid = *class.constructors.get(&key?)?;
        let params = &self.ctx.class_params[c.0 as usize];
        let mut subst = HashMap::new();
        for (p, a) in params.iter().zip(targs.iter()) {
            subst.insert(p.id, a.clone());
        }
        let ty = self.ctx.fn_ty(fid).subst_prop(&subst);
        let inst = Ty::Iface { class: c, args: targs.clone(), nullable: false };
        let ty = match ty {
            Ty::Fn { pos, opt, named, nullable, .. } => Ty::Fn { type_params: vec![], ret: Box::new(inst.clone()), pos, opt, named, nullable },
            t => t,
        };
        let (pos_n, ..) = match &ty {
            Ty::Fn { pos, .. } => (pos.len(), ()),
            _ => (0, ()),
        };
        let params_js: Vec<String> = (0..pos_n).map(|i| format!("a{i}")).collect();
        let f = self.ctx.program.function(fid);
        let cname = if name == "new" { "new".to_string() } else { static_member_name(name) };
        // O tearoff estático existe quando a classe não tem parâmetros próprios
        // (ele mesmo passa o `rti` da classe ao construtor).
        if self.ctx.class_params[c.0 as usize].is_empty() {
            let rti = self.rti(&ty);
            return Some((Js::prim(format!("dart.fn({}[{}], {rti})", self.class_ref(c), js::string_literal(&format!("_#{cname}#tearOff")))), ty));
        }
        let mut args = params_js.clone();
        if self.ctx.requires_rti(c) {
            args.insert(0, self.rti(&inst));
        }
        let call = if f.factory {
            format!("{}.{cname}({})", self.class_ref(c), args.join(", "))
        } else {
            format!("new {}.{cname}({})", self.class_ref(c), args.join(", "))
        };
        let rti = self.rti(&ty);
        Some((Js::prim(format!("dart.fn(({}) => {call}, {rti})", params_js.join(", "))), ty))
    }

    pub fn static_member_get(&mut self, c: ClassId, name: &str) -> Option<(Js, Ty)> {
        let mut cur = Some(c);
        while let Some(k) = cur {
            if let Some(mk) = self.ctx.declared_static(k, name, false) {
                return Some(self.emit_static_get(k, name, mk));
            }
            cur = self.ctx.superclass_of(k);
        }
        // Constantes de enum.
        {
            let class = self.ctx.program.class(c);
            if class.kind == ClassKind::Enum {
                if let Some(sym) = self.ctx.sym(name) {
                    if class.enum_constants.iter().any(|v| self.ctx.program.variable(*v).name == sym) {
                        let cls = self.class_ref(c);
                        return Some((Js::prim(format!("{cls}{}", js::prop_access(&static_member_name(name)))), Ty::iface(c)));
                    }
                }
            }
        }
        // Construtor como tearoff: `C.new`/`C.named`.
        let class = self.ctx.program.class(c);
        let key = if name == "new" { self.ctx.empty_sym } else { self.ctx.sym(name) };
        if let Some(sym) = key {
            if class.constructors.contains_key(&sym) {
                let targs: Vec<Ty> = self.ctx.class_params[c.0 as usize].iter().map(|_| Ty::Dynamic).collect();
                return self.ctor_tearoff(c, targs, name);
            }
            if let Some(&fid) = class.constructors.get(&sym) {
                let ty = self.ctx.fn_ty(fid);
                let (pos_n, ..) = match &ty {
                    Ty::Fn { pos, .. } => (pos.len(), ()),
                    _ => (0, ()),
                };
                let params: Vec<String> = (0..pos_n).map(|i| format!("a{i}")).collect();
                let f = self.ctx.program.function(fid);
                let cname = if name == "new" { "new".to_string() } else { static_member_name(name) };
                let call = if f.factory {
                    format!("{}.{cname}({})", self.class_ref(c), params.join(", "))
                } else {
                    format!("new {}.{cname}({})", self.class_ref(c), params.join(", "))
                };
                let rti = self.rti(&ty);
                return Some((Js::prim(format!("dart.fn(({}) => {call}, {rti})", params.join(", "))), ty));
            }
        }
        None
    }

    /// `recv[i]`.
    pub fn emit_index_get(&mut self, recv: &Js, recv_ty: &Ty, idx: &Js) -> (Js, Ty) {
        if recv_ty.is_dynamic() {
            return (Js::prim(format!("dart.dsend({}, '_get', [{}])", recv.code, idx.code)), Ty::Dynamic);
        }
        match self.ctx.lookup_member(recv_ty, "[]", false) {
            Some(m) => {
                let ty = match self.ctx.member_ty(&m) {
                    Ty::Fn { ret, .. } => (*ret).clone(),
                    _ => Ty::Dynamic,
                };
                let access = self.member_access(recv_ty, "[]", false);
                (Js::prim(format!("{}{access}({})", recv.at(P_PRIMARY), idx.code)), ty)
            }
            None => {
                if let Some((ext, fid, subst)) = self.find_extension_member(recv_ty, "[]", false) {
                    let e = self.ctx.program.extension(ext);
                    let lib_var = self.lib_var(e.library);
                    let ext_name = self.extension_js_name(ext);
                    let ty = match self.ctx.fn_ty(fid).subst_prop(&subst) {
                        Ty::Fn { ret, .. } => (*ret).clone(),
                        _ => Ty::Dynamic,
                    };
                    let mut args = self.ext_type_args(ext, &subst);
                    args.push(recv.code.clone());
                    args.push(idx.code.clone());
                    return (Js::prim(format!("{lib_var}[{}]({})", js::string_literal(&format!("{ext_name}|[]")), args.join(", "))), ty);
                }
                (Js::prim(format!("dart.dsend({}, '_get', [{}])", recv.code, idx.code)), Ty::Dynamic)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Operadores
    // -----------------------------------------------------------------------

    fn emit_unary(&mut self, op: UnaryOp, operand: ExprId) -> (Js, Ty) {
        match op {
            UnaryOp::Not => {
                let (c, _) = self.emit_cond(operand);
                let p = std::mem::take(&mut self.pending_promotions);
                let n = std::mem::take(&mut self.negated_promotions);
                self.pending_promotions = n;
                self.negated_promotions = p;
                (Js::new(format!("!{}", Js::new(c, P_PRIMARY).at(P_UNARY)), P_UNARY), self.ctx.t_bool())
            }
            UnaryOp::Neg => {
                let (v, vty) = self.emit_expr(operand, None);
                if self.ctx.is_num_like_nullable(&vty) {
                    let v = self.not_null(&v, &vty);
                    return (Js::new(format!("-{}", v.at(P_UNARY)), P_UNARY), vty.non_null());
                }
                if vty.is_dynamic() {
                    return (Js::prim(format!("dart.dsend({}, '_negate', [])", v.code)), Ty::Dynamic);
                }
                match self.ctx.lookup_member(&vty, "unary-", false) {
                    Some(m) => {
                        let ret = match self.ctx.member_ty(&m) {
                            Ty::Fn { ret, .. } => (*ret).clone(),
                            _ => Ty::Dynamic,
                        };
                        let access = self.member_access(&vty, "unary-", false);
                        (Js::prim(format!("{}{access}()", v.at(P_PRIMARY))), ret)
                    }
                    None => {
                        if let Some((ext, fid, subst)) = self.find_extension_member(&vty, "unary-", false) {
                            let e = self.ctx.program.extension(ext);
                            let lib_var = self.lib_var(e.library);
                            let ext_name = self.extension_js_name(ext);
                            let ret = match self.ctx.fn_ty(fid).subst_prop(&subst) {
                                Ty::Fn { ret, .. } => (*ret).clone(),
                                _ => Ty::Dynamic,
                            };
                            let mut args = self.ext_type_args(ext, &subst);
                            args.push(v.code.clone());
                            return (Js::prim(format!("{lib_var}[{}]({})", js::string_literal(&format!("{ext_name}|unary-")), args.join(", "))), ret);
                        }
                        (Js::prim(format!("dart.dsend({}, '_negate', [])", v.code)), Ty::Dynamic)
                    }
                }
            }
            UnaryOp::BitNot => {
                let (v, vty) = self.emit_expr(operand, None);
                if self.ctx.is_num_like_nullable(&vty) {
                    let v = self.not_null(&v, &vty);
                    return (Js::new(format!("~{} >>> 0", v.at(P_UNARY)), P_SHIFT), self.ctx.t_int());
                }
                if vty.is_dynamic() {
                    return (Js::prim(format!("dart.dsend({}, '~', [])", v.code)), Ty::Dynamic);
                }
                match self.ctx.lookup_member(&vty, "~", false) {
                    Some(m) => {
                        let ret = match self.ctx.member_ty(&m) {
                            Ty::Fn { ret, .. } => (*ret).clone(),
                            _ => Ty::Dynamic,
                        };
                        let access = self.member_access(&vty, "~", false);
                        (Js::prim(format!("{}{access}()", v.at(P_PRIMARY))), ret)
                    }
                    None => (Js::prim(format!("dart.dsend({}, '~', [])", v.code)), Ty::Dynamic),
                }
            }
            UnaryOp::NullAssert => {
                let (v, vty) = self.emit_expr(operand, None);
                if !vty.is_nullable() && !vty.is_dynamic() {
                    return (v, vty);
                }
                (Js::prim(format!("dart.nullCheck({})", v.code)), vty.non_null())
            }
            UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec => {
                let is_inc = matches!(op, UnaryOp::PrefixInc | UnaryOp::PostfixInc);
                let prefix = matches!(op, UnaryOp::PrefixInc | UnaryOp::PrefixDec);
                let bop = if is_inc { BinaryOp::Add } else { BinaryOp::Sub };
                self.emit_compound(operand, bop, None, Some(1), prefix)
            }
        }
    }

    pub fn not_null(&self, v: &Js, ty: &Ty) -> Js {
        if ty.is_nullable() && !ty.is_dynamic() {
            Js::prim(format!("dart.notNull({})", v.code))
        } else {
            v.clone()
        }
    }

    /// Operador binário aplicado a valores já emitidos, com tipos.
    pub fn emit_binop_values(&mut self, op: BinaryOp, l: Js, lt: &Ty, r: Js, rt: &Ty) -> (Js, Ty) {
        let ctx = self.ctx;
        let num_l = ctx.is_num_like_nullable(lt);
        let num_r = ctx.is_num_like_nullable(rt) || matches!(rt, Ty::Dynamic);
        if num_l && (num_r || ctx.is_num_like_nullable(rt)) && !matches!(op, BinaryOp::Eq | BinaryOp::NotEq | BinaryOp::And | BinaryOp::Or | BinaryOp::IfNull) {
            let ln = self.not_null(&l, lt);
            let rn = self.not_null(&r, rt);
            let is_int = ctx.is_int(&lt.non_null()) && ctx.is_int(&rt.non_null());
            let is_double = ctx.is_double(&lt.non_null()) || ctx.is_double(&rt.non_null());
            let arith_ty = if is_int { ctx.t_int() } else if is_double { ctx.t_double() } else { ctx.t_num() };
            let dyn_r = matches!(rt, Ty::Dynamic);
            let rn = if dyn_r { Js::prim(format!("dart.notNull({})", rn.code)) } else { rn };
            return match op {
                BinaryOp::Add => (Js::new(format!("{} + {}", ln.at(P_ADD), rn.at(P_ADD + 1)), P_ADD), arith_ty),
                BinaryOp::Sub => (Js::new(format!("{} - {}", ln.at(P_ADD), rn.at(P_ADD + 1)), P_ADD), arith_ty),
                BinaryOp::Mul => (Js::new(format!("{} * {}", ln.at(P_MUL), rn.at(P_MUL + 1)), P_MUL), arith_ty),
                BinaryOp::Div => (Js::new(format!("{} / {}", ln.at(P_MUL), rn.at(P_MUL + 1)), P_MUL), ctx.t_double()),
                BinaryOp::TruncDiv => (Js::prim(format!("{}[{}]({})", ln.at(P_PRIMARY), self.dartx("~/"), rn.code)), ctx.t_int()),
                BinaryOp::Rem => (Js::prim(format!("{}[{}]({})", ln.at(P_PRIMARY), self.dartx("%"), rn.code)), arith_ty),
                BinaryOp::Shl => (Js::prim(format!("{}[{}]({})", ln.at(P_PRIMARY), self.dartx("<<"), rn.code)), ctx.t_int()),
                BinaryOp::Shr => (Js::prim(format!("{}[{}]({})", ln.at(P_PRIMARY), self.dartx(">>"), rn.code)), ctx.t_int()),
                BinaryOp::UShr => (Js::prim(format!("{}[{}]({})", ln.at(P_PRIMARY), self.dartx(">>>"), rn.code)), ctx.t_int()),
                BinaryOp::BitAnd => (Js::new(format!("({} & {}) >>> 0", ln.at(P_BITAND), rn.at(P_BITAND + 1)), P_SHIFT), ctx.t_int()),
                BinaryOp::BitOr => (Js::new(format!("({} | {}) >>> 0", ln.at(P_BITOR), rn.at(P_BITOR + 1)), P_SHIFT), ctx.t_int()),
                BinaryOp::BitXor => (Js::new(format!("({} ^ {}) >>> 0", ln.at(P_BITXOR), rn.at(P_BITXOR + 1)), P_SHIFT), ctx.t_int()),
                BinaryOp::Lt => (Js::new(format!("{} < {}", ln.at(P_REL), rn.at(P_REL + 1)), P_REL), ctx.t_bool()),
                BinaryOp::Gt => (Js::new(format!("{} > {}", ln.at(P_REL), rn.at(P_REL + 1)), P_REL), ctx.t_bool()),
                BinaryOp::LtEq => (Js::new(format!("{} <= {}", ln.at(P_REL), rn.at(P_REL + 1)), P_REL), ctx.t_bool()),
                BinaryOp::GtEq => (Js::new(format!("{} >= {}", ln.at(P_REL), rn.at(P_REL + 1)), P_REL), ctx.t_bool()),
                _ => unreachable!(),
            };
        }
        if ctx.is_string(&lt.non_null()) && op == BinaryOp::Add && !lt.is_nullable() {
            let rn = if ctx.is_string(rt) { r } else { Js::prim(format!("dart.nullCheck({})", r.code)) };
            return (Js::new(format!("{} + {}", l.at(P_ADD), rn.at(P_ADD + 1)), P_ADD), ctx.t_string());
        }
        if ctx.is_string(&lt.non_null()) && op == BinaryOp::Mul && !lt.is_nullable() {
            return (Js::prim(format!("{}[{}]({})", l.at(P_PRIMARY), self.dartx("*"), r.code)), ctx.t_string());
        }
        match op {
            BinaryOp::Eq | BinaryOp::NotEq => {
                let neg = op == BinaryOp::NotEq;
                let js = self.emit_equals(&l, lt, &r, rt);
                if neg {
                    (Js::new(format!("!{}", js.at(P_UNARY)), P_UNARY), ctx.t_bool())
                } else {
                    (js, ctx.t_bool())
                }
            }
            _ => {
                let name = binop_name(op);
                if lt.is_dynamic() {
                    return (Js::prim(format!("dart.dsend({}, {}, [{}])", l.code, js::string_literal(&crate::body::js_member_name(name)), r.code)), Ty::Dynamic);
                }
                match ctx.lookup_member(lt, name, false) {
                    Some(m) => {
                        let ret = match ctx.member_ty(&m) {
                            Ty::Fn { ret, .. } => (*ret).clone(),
                            _ => Ty::Dynamic,
                        };
                        let access = self.member_access(lt, name, false);
                        (Js::prim(format!("{}{access}({})", l.at(P_PRIMARY), r.code)), ret)
                    }
                    None => {
                        if let Some((ext, fid, subst)) = self.find_extension_member(lt, name, false) {
                            let e = ctx.program.extension(ext);
                            let lib_var = self.lib_var(e.library);
                            let ext_name = self.extension_js_name(ext);
                            let ret = match ctx.fn_ty(fid).subst_prop(&subst) {
                                Ty::Fn { ret, .. } => (*ret).clone(),
                                _ => Ty::Dynamic,
                            };
                            let mut args = self.ext_type_args(ext, &subst);
                            args.push(l.code.clone());
                            args.push(r.code.clone());
                            return (Js::prim(format!("{lib_var}[{}]({})", js::string_literal(&format!("{ext_name}|{name}")), args.join(", "))), ret);
                        }
                        (Js::prim(format!("dart.dsend({}, {}, [{}])", l.code, js::string_literal(&crate::body::js_member_name(name)), r.code)), Ty::Dynamic)
                    }
                }
            }
        }
    }

    /// Registra promoção `x != null` / `x == null` para a variável `sym`.
    pub fn note_null_promotion(&mut self, sym: dartforge_intern::SymbolId, ty: &Ty, negated: bool) {
        if ty.is_nullable() && !ty.is_dynamic() {
            let nn = ty.non_null();
            if negated {
                self.pending_promotions.push((sym, nn));
            } else {
                self.negated_promotions.push((sym, nn));
            }
        }
    }

    pub fn emit_equals(&mut self, l: &Js, lt: &Ty, r: &Js, rt: &Ty) -> Js {
        let ctx = self.ctx;
        if matches!(lt, Ty::Null) || matches!(rt, Ty::Null) || l.code == "null" || r.code == "null" {
            return Js::new(format!("{} == {}", l.at(P_EQ), r.at(P_EQ + 1)), P_EQ);
        }
        let prim = ctx.is_js_primitive(&lt.non_null()) && (ctx.is_js_primitive(&rt.non_null()) || rt.is_dynamic());
        let is_enum = lt.class().is_some_and(|c| ctx.is_enum_class(c));
        if prim || is_enum {
            if !lt.is_nullable() || !rt.is_nullable() {
                return Js::new(format!("{} === {}", l.at(P_EQ), r.at(P_EQ + 1)), P_EQ);
            }
            return Js::new(format!("{} == {}", l.at(P_EQ), r.at(P_EQ + 1)), P_EQ);
        }
        Js::prim(format!("dart.equals({}, {})", l.code, r.code))
    }

    fn emit_binary(&mut self, op: BinaryOp, left: ExprId, right: ExprId, expected: Option<&Ty>) -> (Js, Ty) {
        match op {
            BinaryOp::And => {
                let (l, _) = self.emit_cond(left);
                let promos = std::mem::take(&mut self.pending_promotions);
                self.push_scope();
                for (sym, t) in &promos {
                    if let Some(loc) = self.lookup_local(*sym).cloned() {
                        self.declare_js(*sym, loc.js, t.clone());
                    }
                }
                let (r, _) = self.emit_cond(right);
                self.pop_scope();
                self.pending_promotions = promos;
                return (Js::new(format!("{} && {}", Js::new(l, P_PRIMARY).at(P_AND), Js::new(r, P_PRIMARY).at(P_AND + 1)), P_AND), self.ctx.t_bool());
            }
            BinaryOp::Or => {
                let (l, _) = self.emit_cond(left);
                self.pending_promotions.clear();
                let (r, _) = self.emit_cond(right);
                self.pending_promotions.clear();
                return (Js::new(format!("{} || {}", Js::new(l, P_PRIMARY).at(P_OR), Js::new(r, P_PRIMARY).at(P_OR + 1)), P_OR), self.ctx.t_bool());
            }
            BinaryOp::IfNull => {
                let (l, lt) = self.emit_expr(left, expected);
                let (r, rt) = self.emit_expr(right, expected.or(Some(&lt.non_null())));
                let t = self.temp();
                let ty = self.ctx.lub(&lt.non_null(), &rt);
                return (Js::new(format!("{t} = {}, {t} == null ? {} : {t}", l.code, r.at(P_COND)), P_COMMA).paren(), ty);
            }
            _ => {}
        }
        // `super == x` / `super + x`: chamada direta do operador da superclasse.
        if let ExprKind::Super = self.expr(left).kind {
            let sup_ty = self.super_ty();
            let name = binop_name(op);
            let is_eq = matches!(op, BinaryOp::Eq | BinaryOp::NotEq);
            let mname = if is_eq { "==" } else { name };
            let (r, _) = self.emit_expr(right, None);
            let access = self.member_access(&sup_ty, mname, false);
            let call = format!("{}{access}({})", self.super_ref(), r.code);
            let ret = if is_eq {
                self.ctx.t_bool()
            } else {
                self.ctx.lookup_member(&sup_ty, mname, false).map(|m| match self.ctx.member_ty(&m) {
                    Ty::Fn { ret, .. } => (*ret).clone(),
                    _ => Ty::Dynamic,
                }).unwrap_or(Ty::Dynamic)
            };
            if op == BinaryOp::NotEq {
                return (Js::new(format!("!{call}"), P_UNARY), ret);
            }
            return (Js::prim(call), ret);
        }
        let (l, lt) = self.emit_expr(left, None);
        if matches!(op, BinaryOp::Eq | BinaryOp::NotEq) {
            if let (ExprKind::Identifier(n), ExprKind::Null) = (&self.expr(left).kind, &self.expr(right).kind) {
                if self.lookup_local(n.sym).is_some() {
                    self.note_null_promotion(n.sym, &lt, op == BinaryOp::NotEq);
                }
            }
        }
        let r_expected: Option<Ty> = if self.ctx.is_num_like_nullable(&lt) || matches!(op, BinaryOp::Eq | BinaryOp::NotEq) {
            None
        } else {
            self.ctx.lookup_member(&lt.non_null(), binop_name(op), false).and_then(|m| match self.ctx.member_ty(&m) {
                Ty::Fn { pos, .. } => pos.first().cloned(),
                _ => None,
            })
        };
        // `e == .x` (3.10): o atalho à direita usa o tipo estático da esquerda.
        let (r, rt) = if matches!(op, BinaryOp::Eq | BinaryOp::NotEq) {
            let esquerda = lt.clone();
            self.com_contexto_de_atalho(right, &esquerda, |s| s.emit_expr(right, r_expected.as_ref()))
        } else {
            self.emit_expr(right, r_expected.as_ref())
        };
        self.emit_binop_values(op, l, &lt, r, &rt)
    }

    // -----------------------------------------------------------------------
    // Atribuições
    // -----------------------------------------------------------------------

    fn emit_assign(&mut self, op: AssignOp, target: ExprId, value: ExprId) -> (Js, Ty) {
        match op {
            AssignOp::Assign => {
                let tty = self.target_ty(target);
                let (v, vty) = self.emit_expr(value, tty.as_ref());
                // Atualiza tipo de local sem anotação de valor conhecido (promoção por atribuição).
                let (js, _) = self.emit_assign_to(target, &v, &vty);
                (js, vty)
            }
            AssignOp::Compound(BinaryOp::IfNull) => self.emit_compound(target, BinaryOp::IfNull, Some(value), None, true),
            AssignOp::Compound(bop) => self.emit_compound(target, bop, Some(value), None, true),
        }
    }

    /// Tipo estático do alvo de atribuição (sem emitir).
    pub fn target_ty(&mut self, target: ExprId) -> Option<Ty> {
        let t = self.expr(target);
        match &t.kind {
            ExprKind::Identifier(n) => match self.resolve_ident(n.sym) {
                IdentTarget::Local(_, ty) => Some(ty),
                IdentTarget::ThisMember(m) => {
                    let name = self.name(n.sym).to_string();
                    let m2 = self.ctx.lookup_member(&self.ctx.this_ty(m.class), &name, true).unwrap_or(m);
                    Some(self.ctx.member_ty(&m2))
                }
                IdentTarget::Static(_, mk) => match mk {
                    MemberKind::Field(v) => Some(self.ctx.var_ty(v)),
                    MemberKind::Setter(f) => self.ctx.outline.functions[f.0 as usize].parameters.first().map(|p| self.ctx.ty_of(p.ty)),
                    _ => None,
                },
                IdentTarget::Element(Element::Variable(v)) => Some(self.ctx.var_ty(v)),
                _ => None,
            },
            ExprKind::Property { target: recv, name, .. } => {
                let n = self.name(name.sym).to_string();
                let rty = self.type_of(*recv);
                let m = self.ctx.lookup_member(&rty.non_null(), &n, true)?;
                Some(self.ctx.member_ty(&m))
            }
            ExprKind::Index { target: recv, .. } => {
                let rty = self.type_of(*recv);
                let m = self.ctx.lookup_member(&rty.non_null(), "[]=", false)?;
                match self.ctx.member_ty(&m) {
                    Ty::Fn { pos, .. } => pos.get(1).cloned(),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Atribui `v` ao alvo. Devolve a expressão (cujo valor é `v`).
    pub fn emit_assign_to(&mut self, target: ExprId, v: &Js, vty: &Ty) -> (Js, Ty) {
        let t = self.expr(target);
        match &t.kind {
            ExprKind::Identifier(n) => {
                let name = self.name(n.sym).to_string();
                match self.resolve_ident(n.sym) {
                    IdentTarget::Local(js, lty) => {
                        if let Some(l) = self.lookup_local(n.sym).cloned() {
                            if l.late_final {
                                self.m.use_sdk("_internal");
                                return (Js::new(format!("{js} === void 0 ? {js} = {} : dart.throw(new _internal.LateError.localAI({}))", v.at(P_ASSIGN), js::string_literal(&name)), P_COND).paren(), lty);
                            }
                        }
                        // Promoção por atribuição: local `dynamic`/nullable recebe tipo do valor quando compatível.
                        if lty.is_nullable() && !lty.is_dynamic() && self.ctx.is_subtype(vty, &lty) && !vty.is_dynamic() && !matches!(vty, Ty::Null) {
                            self.set_local_ty(n.sym, vty.clone());
                        } else if matches!(vty, Ty::Null) && lty.is_nullable() {
                            // mantém
                        }
                        (Js::new(format!("{js} = {}", v.at(P_ASSIGN)), P_ASSIGN), lty)
                    }
                    IdentTarget::ThisMember(m) => {
                        let this_ty = self.ctx.this_ty(m.class);
                        let m2 = self.ctx.lookup_member(&this_ty, &name, true).unwrap_or(m);
                        let access = self.member_access(&this_ty, &name, true);
                        let v = if self.ctx.is_js_member_kind(&m2.kind) { self.assert_interop(v.clone(), vty) } else { v.clone() };
                        (Js::new(format!("this{access} = {}", v.at(P_ASSIGN)), P_ASSIGN), self.ctx.member_ty(&m2))
                    }
                    IdentTarget::ThisExt => {
                        let t = self.class.map(|c| self.ctx.this_ty(c)).unwrap_or(Ty::Dynamic);
                        if let Some((ext, _, subst)) = self.find_extension_member(&t, &name, true) {
                            let e = self.ctx.program.extension(ext);
                            let lib_var = self.lib_var(e.library);
                            let ext_name = self.extension_js_name(ext);
                            let mut args = self.ext_type_args(ext, &subst);
                            args.push("this".into());
                            args.push(v.code.clone());
                            return (Js::prim(format!("{lib_var}[{}]({})", js::string_literal(&format!("{ext_name}|set#{name}")), args.join(", "))), vty.clone());
                        }
                        (Js::prim("null"), Ty::Dynamic)
                    }
                    IdentTarget::ExtField(ext) => {
                        let e = self.ctx.program.extension(ext);
                        let lib_var = self.lib_var(e.library);
                        let ext_name = self.extension_js_name(ext);
                        (Js::new(format!("{lib_var}[{}] = {}", js::string_literal(&format!("{ext_name}|{name}")), v.at(P_ASSIGN)), P_ASSIGN), vty.clone())
                    }
                    IdentTarget::ExtThisMember(_) | IdentTarget::ExtMember(..) | IdentTarget::ExtStatic(..) => {
                        let t = self.extension_this.clone().unwrap_or(Ty::Dynamic);
                        if let Some((ext, _, subst)) = self.find_extension_member(&t, &name, true) {
                            let e = self.ctx.program.extension(ext);
                            let lib_var = self.lib_var(e.library);
                            let ext_name = self.extension_js_name(ext);
                            let mut args = self.ext_type_args(ext, &subst);
                            args.push("$this".into());
                            args.push(v.code.clone());
                            return (Js::prim(format!("{lib_var}[{}]({})", js::string_literal(&format!("{ext_name}|set#{name}")), args.join(", "))), vty.clone());
                        }
                        let access = self.member_access(&t, &name, true);
                        (Js::new(format!("$this{access} = {}", v.at(P_ASSIGN)), P_ASSIGN), Ty::Dynamic)
                    }
                    IdentTarget::Static(c, mk) => {
                        if self.ctx.is_js_class(c) {
                            let smk = self.ctx.declared_static(c, &name, true).unwrap_or(mk);
                            if self.ctx.is_js_member_kind(&smk) {
                                let target_js = self.ctx.js_static_ref(Some(c), self.ctx.lib_of_class(c), &smk, &name);
                                let v = self.assert_interop(v.clone(), vty);
                                return (Js::new(format!("{target_js} = {}", v.at(P_ASSIGN)), P_ASSIGN), Ty::Dynamic);
                            }
                        }
                        let cls = self.class_ref(c);
                        (Js::new(format!("{cls}{} = {}", js::prop_access(&static_member_name(&name)), v.at(P_ASSIGN)), P_ASSIGN), Ty::Dynamic)
                    }
                    IdentTarget::Element(el) => {
                        let mut v = v.clone();
                        let js = match el {
                            Element::Variable(vid) if self.ctx.is_js_var(vid) => {
                                let var = self.ctx.program.variable(vid);
                                v = self.assert_interop(v, vty);
                                self.ctx.js_static_ref(None, var.library, &MemberKind::Field(vid), &name)
                            }
                            Element::Variable(vid) => {
                                let var = self.ctx.program.variable(vid);
                                format!("{}{}", self.lib_var(var.library), js::prop_access(&name))
                            }
                            Element::Function(fid) if self.ctx.is_js_member(fid) => {
                                let f = self.ctx.program.function(fid);
                                v = self.assert_interop(v, vty);
                                self.ctx.js_static_ref(None, f.library, &MemberKind::Setter(fid), &name)
                            }
                            Element::Function(fid) => {
                                let f = self.ctx.program.function(fid);
                                format!("{}{}", self.lib_var(f.library), js::prop_access(&name))
                            }
                            _ => js::ident(&name),
                        };
                        (Js::new(format!("{js} = {}", v.at(P_ASSIGN)), P_ASSIGN), Ty::Dynamic)
                    }
                    _ => (Js::new(format!("{} = {}", js::ident(&name), v.at(P_ASSIGN)), P_ASSIGN), Ty::Dynamic),
                }
            }
            ExprKind::Property { target: recv, name, null_aware } => {
                let n = self.name(name.sym).to_string();
                // Estático via classe/prefixo.
                if let Some((target_js, interop)) = self.try_static_lvalue(*recv, &n) {
                    let v = if interop { self.assert_interop(v.clone(), vty) } else { v.clone() };
                    return (Js::new(format!("{target_js} = {}", v.at(P_ASSIGN)), P_ASSIGN), Ty::Dynamic);
                }
                if let ExprKind::Super = self.expr(*recv).kind {
                    let sup_ty = self.super_ty();
                    let access = self.member_access(&sup_ty, &n, true);
                    return (Js::new(format!("{}{access} = {}", self.super_ref(), v.at(P_ASSIGN)), P_ASSIGN), Ty::Dynamic);
                }
                let (rjs, rty, mut guards) = self.emit_target(*recv);
                let (recv_js, recv_ty) = if *null_aware {
                    let t = self.temp();
                    guards.push(Guard { temp: t.clone(), init: rjs.code });
                    (Js::prim(t), rty.non_null())
                } else {
                    (rjs, rty)
                };
                let js = if recv_ty.is_dynamic() || self.ctx.lookup_member(&recv_ty, &n, true).is_none() {
                    if let Some((ext, fid, subst)) = self.find_extension_member(&recv_ty, &n, true) {
                        let e = self.ctx.program.extension(ext);
                        let lib_var = self.lib_var(e.library);
                        let ext_name = self.extension_js_name(ext);
                        let _ = fid;
                        let mut args = self.ext_type_args(ext, &subst);
                        args.push(recv_js.code.clone());
                        args.push(v.code.clone());
                        Js::prim(format!("{lib_var}[{}]({})", js::string_literal(&format!("{ext_name}|set#{n}")), args.join(", ")))
                    } else if let Ty::Record { .. } = recv_ty {
                        Js::new(format!("{}{} = {}", recv_js.at(P_PRIMARY), js::prop_access(&n), v.at(P_ASSIGN)), P_ASSIGN)
                    } else {
                        Js::prim(format!("dart.dput({}, {}, {})", recv_js.code, js::string_literal(&crate::body::js_member_name(&n)), v.code))
                    }
                } else {
                    let access = self.member_access(&recv_ty, &n, true);
                    let interop = self.ctx.lookup_member(&recv_ty, &n, true).is_some_and(|m| self.ctx.is_js_member_kind(&m.kind));
                    let v = if interop { self.assert_interop(v.clone(), vty) } else { v.clone() };
                    Js::new(format!("{}{access} = {}", recv_js.at(P_PRIMARY), v.at(P_ASSIGN)), P_ASSIGN)
                };
                (self.wrap_guards(js, guards), vty.clone())
            }
            ExprKind::Index { target: recv, index, null_aware } => {
                let (rjs, rty, mut guards) = self.emit_target(*recv);
                let (recv_js, recv_ty) = if *null_aware {
                    let t = self.temp();
                    guards.push(Guard { temp: t.clone(), init: rjs.code });
                    (Js::prim(t), rty.non_null())
                } else {
                    (rjs, rty)
                };
                let (ijs, _) = self.emit_expr(*index, None);
                let js = self.emit_index_set(&recv_js, &recv_ty, &ijs, v);
                (self.wrap_guards(js, guards), vty.clone())
            }
            ExprKind::Parenthesized(inner) => self.emit_assign_to(*inner, v, vty),
            ExprKind::CascadeTarget => (Js::prim("null"), Ty::Dynamic),
            _ => {
                let (js, _) = self.emit_expr(target, None);
                (Js::new(format!("{} = {}", js.code, v.at(P_ASSIGN)), P_ASSIGN), Ty::Dynamic)
            }
        }
    }

    fn try_static_lvalue(&mut self, recv: ExprId, name: &str) -> Option<(String, bool)> {
        let t = self.expr(recv);
        match &t.kind {
            ExprKind::Identifier(id) => match self.resolve_ident(id.sym) {
                IdentTarget::Prefix(p) => {
                    let sym = self.ctx.sym(name)?;
                    let b = self.ctx.program.lookup_prefixed(self.lib, p, sym)?;
                    match b.setter.or(b.getter)? {
                        Element::Variable(vid) if self.ctx.is_js_var(vid) => {
                            let var = self.ctx.program.variable(vid);
                            Some((self.ctx.js_static_ref(None, var.library, &MemberKind::Field(vid), name), true))
                        }
                        Element::Variable(vid) => {
                            let var = self.ctx.program.variable(vid);
                            Some((format!("{}{}", self.lib_var(var.library), js::prop_access(name)), false))
                        }
                        Element::Function(fid) if self.ctx.is_js_member(fid) => {
                            let f = self.ctx.program.function(fid);
                            Some((self.ctx.js_static_ref(None, f.library, &MemberKind::Setter(fid), name), true))
                        }
                        Element::Function(fid) => {
                            let f = self.ctx.program.function(fid);
                            Some((format!("{}{}", self.lib_var(f.library), js::prop_access(name)), false))
                        }
                        _ => None,
                    }
                }
                IdentTarget::Element(Element::Class(c)) => {
                    let mut cur = Some(c);
                    while let Some(k) = cur {
                        if let Some(mk) = self.ctx.declared_static(k, name, true).or_else(|| self.ctx.declared_static(k, name, false)) {
                            if self.ctx.is_js_class(k) && self.ctx.is_js_member_kind(&mk) {
                                return Some((self.ctx.js_static_ref(Some(k), self.ctx.lib_of_class(k), &mk, name), true));
                            }
                            return Some((format!("{}{}", self.class_ref(k), js::prop_access(&static_member_name(name))), false));
                        }
                        cur = self.ctx.superclass_of(k);
                    }
                    None
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// `recv[i] = v`; a expressão vale `v`.
    pub fn emit_index_set(&mut self, recv: &Js, recv_ty: &Ty, idx: &Js, v: &Js) -> Js {
        if recv_ty.is_dynamic() {
            return Js::prim(format!("dart.dsend({}, '_set', [{}, {}])", recv.code, idx.code, v.code));
        }
        match self.ctx.lookup_member(recv_ty, "[]=", false) {
            Some(_) => {
                let access = self.member_access(recv_ty, "[]=", false);
                let t = self.temp();
                let mut parts = Vec::new();
                let recv_js = if is_simple(&recv.code) {
                    recv.at(P_PRIMARY).into_owned()
                } else {
                    let tr = self.temp();
                    parts.push(format!("{tr} = {}", recv.code));
                    tr
                };
                let idx_js = if is_simple(&idx.code) && (is_simple(&v.code) || v.code.starts_with('"')) {
                    idx.code.clone()
                } else {
                    let ti = self.temp();
                    parts.push(format!("{ti} = {}", idx.code));
                    ti
                };
                parts.push(format!("{t} = {}", v.code));
                parts.push(format!("{recv_js}{access}({idx_js}, {t})"));
                parts.push(t);
                Js::new(parts.join(", "), P_COMMA).paren()
            }
            None => {
                if let Some((ext, _, subst)) = self.find_extension_member(recv_ty, "[]=", false) {
                    let e = self.ctx.program.extension(ext);
                    let lib_var = self.lib_var(e.library);
                    let ext_name = self.extension_js_name(ext);
                    let t = self.temp();
                    let mut args = self.ext_type_args(ext, &subst);
                    args.push(recv.code.clone());
                    args.push(idx.code.clone());
                    args.push(t.clone());
                    return Js::new(format!("{t} = {}, {lib_var}[{}]({}), {t}", v.code, js::string_literal(&format!("{ext_name}|[]=")), args.join(", ")), P_COMMA).paren();
                }
                Js::prim(format!("dart.dsend({}, '_set', [{}, {}])", recv.code, idx.code, v.code))
            }
        }
    }

    /// `x op= v`, `++x`, `x++`.
    fn emit_compound(&mut self, target: ExprId, bop: BinaryOp, value: Option<ExprId>, literal: Option<i64>, prefix: bool) -> (Js, Ty) {
        // Lê o alvo uma vez com temps para receptores/índices.
        let t = self.expr(target);
        let (read, read_ty, write): (Js, Ty, Box<dyn Fn(&mut Self, &Js) -> Js>) = match &t.kind {
            ExprKind::Identifier(_) | ExprKind::Parenthesized(_) => {
                let (r, rt) = self.emit_expr(target, None);
                let tgt = target;
                (r, rt, Box::new(move |s: &mut Self, v: &Js| s.emit_assign_to(tgt, v, &Ty::Dynamic).0))
            }
            ExprKind::Property { target: recv, name, null_aware } => {
                let n = self.name(name.sym).to_string();
                if let Some((sjs, _)) = self.try_static_lvalue(*recv, &n) {
                    let rt = self.type_of(target);
                    let sjs2 = sjs.clone();
                    (Js::prim(sjs), rt, Box::new(move |_s: &mut Self, v: &Js| Js::new(format!("{sjs2} = {}", v.at(P_ASSIGN)), P_ASSIGN)))
                } else if let ExprKind::Super = self.expr(*recv).kind {
                    let sup_ty = self.super_ty();
                    let m = self.ctx.lookup_member(&sup_ty, &n, false);
                    let rt = m.map(|m| self.ctx.member_ty(&m)).unwrap_or(Ty::Dynamic);
                    let get = self.member_access(&sup_ty, &n, false);
                    let set = self.member_access(&sup_ty, &n, true);
                    let sr = self.super_ref();
                    let sr2 = sr.clone();
                    (Js::prim(format!("{sr}{get}")), rt, Box::new(move |_s: &mut Self, v: &Js| Js::new(format!("{sr2}{set} = {}", v.at(P_ASSIGN)), P_ASSIGN)))
                } else {
                    let (rjs, rty, guards) = self.emit_target(*recv);
                    let rty = if *null_aware { rty.non_null() } else { rty };
                    let recv_js = if is_simple(&rjs.code) {
                        rjs.code.clone()
                    } else {
                        let tmp = self.temp();
                        // Prefixo de inicialização do temp fica na leitura.
                        self.pending_prefix.push(format!("{tmp} = {}", rjs.code));
                        tmp
                    };
                    let _ = guards;
                    let (r, rt) = self.emit_member_get(&Js::prim(recv_js.clone()), &rty, &n, None);
                    let recv_ty2 = rty.clone();
                    let n2 = n.clone();
                    (r, rt, Box::new(move |s: &mut Self, v: &Js| {
                        if recv_ty2.is_dynamic() || s.ctx.lookup_member(&recv_ty2, &n2, true).is_none() {
                            Js::prim(format!("dart.dput({recv_js}, {}, {})", js::string_literal(&n2), v.code))
                        } else {
                            let access = s.member_access(&recv_ty2, &n2, true);
                            Js::new(format!("{recv_js}{access} = {}", v.at(P_ASSIGN)), P_ASSIGN)
                        }
                    }))
                }
            }
            ExprKind::Index { target: recv, index, .. } => {
                let (rjs, rty, _guards) = self.emit_target(*recv);
                let recv_js = if is_simple(&rjs.code) {
                    rjs.code.clone()
                } else {
                    let tmp = self.temp();
                    self.pending_prefix.push(format!("{tmp} = {}", rjs.code));
                    tmp
                };
                let (ijs, _) = self.emit_expr(*index, None);
                let idx_js = if is_simple(&ijs.code) {
                    ijs.code.clone()
                } else {
                    let tmp = self.temp();
                    self.pending_prefix.push(format!("{tmp} = {}", ijs.code));
                    tmp
                };
                let (r, rt) = self.emit_index_get(&Js::prim(recv_js.clone()), &rty, &Js::prim(idx_js.clone()));
                let rty2 = rty.clone();
                (r, rt, Box::new(move |s: &mut Self, v: &Js| s.emit_index_set(&Js::prim(recv_js.clone()), &rty2, &Js::prim(idx_js.clone()), v)))
            }
            _ => {
                let (r, rt) = self.emit_expr(target, None);
                (r, rt, Box::new(|_s: &mut Self, v: &Js| v.clone()))
            }
        };
        let prefix_parts: Vec<String> = std::mem::take(&mut self.pending_prefix);
        if bop == BinaryOp::IfNull {
            let (v, vty) = match value {
                Some(e) => self.emit_expr(e, Some(&read_ty.non_null())),
                None => (Js::prim("null"), Ty::Null),
            };
            let t = self.temp();
            let assign = write(self, &v);
            let ty = self.ctx.lub(&read_ty.non_null(), &vty);
            let mut parts = prefix_parts;
            parts.push(format!("{t} = {}", read.code));
            parts.push(format!("{t} == null ? {} : {t}", assign.at(P_COND)));
            return (Js::new(parts.join(", "), P_COMMA).paren(), ty);
        }
        let (v, vty) = match (value, literal) {
            (Some(e), _) => self.emit_expr(e, None),
            (None, Some(n)) => (Js::prim(n.to_string()), self.ctx.t_int()),
            _ => (Js::prim("1"), self.ctx.t_int()),
        };
        if prefix {
            let (res, rty) = self.emit_binop_values(bop, read, &read_ty, v, &vty);
            let assign = write(self, &res);
            let code = if prefix_parts.is_empty() {
                assign.code
            } else {
                format!("{}, {}", prefix_parts.join(", "), assign.code)
            };
            let ty = if literal.is_some() { read_ty } else { rty };
            (Js::new(code, P_COMMA).paren(), ty)
        } else {
            // Pós-incremento: valor antigo.
            let old = self.temp();
            let (res, _) = self.emit_binop_values(bop, Js::prim(old.clone()), &read_ty, v, &vty);
            let assign = write(self, &res);
            let mut parts = prefix_parts;
            parts.push(format!("{old} = {}", read.code));
            parts.push(assign.code);
            parts.push(old);
            (Js::new(parts.join(", "), P_COMMA).paren(), read_ty)
        }
    }

    // -----------------------------------------------------------------------
    // Switch-expressão
    // -----------------------------------------------------------------------

    fn emit_switch_expr(&mut self, value: ExprId, cases: &[ast::SwitchExprCase], expected: Option<&Ty>) -> (Js, Ty) {
        let (vjs, vty) = self.emit_expr(value, None);
        let saved_w = std::mem::take(&mut self.w);
        let t = self.temp();
        crate::linha!(self.w, "{t} = {};", vjs.code);
        let mut result_ty: Option<Ty> = None;
        for c in cases {
            self.w.open("{");
            self.push_scope();
            let mut binds = Vec::new();
            let cond = self.pattern_cond(c.pattern, &t, &vty, &mut binds, false);
            for (_, _, jsn) in &binds {
                crate::linha!(self.w, "let {jsn} = null;");
            }
            let mut full = cond;
            if let Some(g) = c.guard {
                let (gjs, _) = self.emit_expr(g, Some(&self.ctx.t_bool()));
                full = format!("({full}) && {}", gjs.at(P_AND));
            }
            let (bjs, bty) = self.emit_expr(c.body, expected);
            result_ty = Some(match result_ty {
                None => bty,
                Some(r) => self.ctx.lub(&r, &bty),
            });
            crate::linha!(self.w, "if ({full}) return {};", bjs.code);
            self.pop_scope();
            self.w.close("}");
        }
        self.m.use_sdk("core");
        self.w.line("dart.throw(new core.StateError.new(\"Pattern matching error\"));");
        let body = std::mem::replace(&mut self.w, saved_w).out;
        (self.iife(&body), result_ty.unwrap_or(Ty::Dynamic))
    }

    // -----------------------------------------------------------------------
    // Instanciação
    // -----------------------------------------------------------------------

    fn emit_instance_creation(
        &mut self,
        keyword: Option<ast::CreationKeyword>,
        ty: ast::TypeId,
        constructor: Option<&ast::Name>,
        arguments: &ast::Arguments,
        expected: Option<&Ty>,
    ) -> (Js, Ty) {
        let mut t = self.resolve_type(ty);
        let is_const = keyword == Some(ast::CreationKeyword::Const) || (keyword.is_none() && self.in_const);
        let mut ctor_name = constructor.map(|n| self.name(n.sym).to_string()).unwrap_or_default();
        // `const C.nome(...)`: o parser pode ler `C.nome` como tipo prefixado.
        if !matches!(t, Ty::Iface { .. }) && constructor.is_none() {
            if let ast::TypeKind::Named { name: parts, .. } = &self.ast().ty(ty).kind {
                if parts.len() == 2 {
                    if let Some(Element::Class(c)) = self.ctx.program.lookup(self.lib, parts[0].sym).and_then(|b| b.getter) {
                        t = self.ctx.this_ty_default(c);
                        ctor_name = self.name(parts[1].sym).to_string();
                    }
                }
            }
        }
        let Ty::Iface { class, args, .. } = &t else {
            return (Js::prim("null"), Ty::Dynamic);
        };
        let explicit_args = !self.explicit_type_args(ty).is_empty();
        let class_args = if explicit_args { args.clone() } else { vec![] };
        let saved = self.in_const;
        if is_const {
            self.in_const = true;
        }
        let r = self.emit_constructor_call(*class, class_args, explicit_args, &ctor_name, arguments, expected, is_const);
        self.in_const = saved;
        r
    }

    fn explicit_type_args(&self, t: ast::TypeId) -> Vec<ast::TypeId> {
        match &self.ast().ty(t).kind {
            ast::TypeKind::Named { args, .. } => args.to_vec(),
            _ => vec![],
        }
    }

    /// Chamada de construtor `C<args>.name(...)`; infere argumentos de tipo quando ausentes.
    pub fn emit_constructor_call(
        &mut self,
        class: ClassId,
        class_args: Vec<Ty>,
        explicit_args: bool,
        ctor_name: &str,
        arguments: &ast::Arguments,
        expected: Option<&Ty>,
        is_const: bool,
    ) -> (Js, Ty) {
        // `bool/int/String.fromEnvironment` e `bool.hasEnvironment` (construtores
        // `const factory external`): constantes de ambiente, avaliadas na
        // compilação — é o que a CFE faz e o que o `dart_sdk.js` exige, pois os
        // construtores lançam `UnsupportedError` em tempo de execução.
        if ctor_name == "fromEnvironment" {
            if let Some(r) = self.constante_de_ambiente(class, arguments) {
                return r;
            }
        }
        if ctor_name == "hasEnvironment" && Some(class) == self.ctx.bool_ {
            // Sem `-D` na linha de comando, nenhuma variável está declarada.
            return (Js::prim("false"), self.ctx.t_bool());
        }
        let cls = self.ctx.program.class(class);
        let key = if ctor_name.is_empty() { self.ctx.empty_sym } else { self.ctx.sym(ctor_name) };
        let fid = key.and_then(|k| cls.constructors.get(&k).copied());
        let params = &self.ctx.class_params[class.0 as usize];
        // Tipo função do construtor com os parâmetros da classe como genéricos.
        let ctor_fn = match fid {
            Some(f) => self.ctx.fn_ty(f),
            None => Ty::Fn { type_params: vec![], ret: Box::new(Ty::Dynamic), pos: vec![], opt: vec![], named: vec![], nullable: false },
        };
        let (ctor_tps, ctor_fn) = match ctor_fn {
            Ty::Fn { type_params, ret, pos, opt, named, nullable } => (type_params, Ty::Fn { type_params: vec![], ret, pos, opt, named, nullable }),
            t => (vec![], t),
        };
        let _ = ctor_tps;
        let mut subst: HashMap<u32, Ty> = HashMap::new();
        let mut targs: Vec<Ty> = Vec::new();
        if explicit_args || params.is_empty() {
            for (p, a) in params.iter().zip(class_args.iter()) {
                subst.insert(p.id, a.clone());
                targs.push(a.clone());
            }
        } else {
            // Inferência: contexto esperado e argumentos.
            let free: Vec<u32> = params.iter().map(|p| p.id).collect();
            if let Some(e) = expected {
                if let Some(ec) = e.non_null().class() {
                    // `C<T..>` visto como o supertipo esperado, casado com o esperado.
                    let this_ty = self.ctx.this_ty(class);
                    if let Some(sup) = self.ctx.as_super(&this_ty, ec) {
                        let mut tmp = HashMap::new();
                        self.match_type(&sup, &e.non_null(), &free, &mut tmp);
                        for (k, v) in tmp {
                            if !v.is_dynamic() && !v.mentions_params() {
                                subst.insert(k, v);
                            }
                        }
                    }
                }
            }
            let saved_interop = self.interop_args;
            self.interop_args = fid.is_some_and(|f| self.ctx.is_js_member(f));
            let (arg_js, _arg_tys) = self.emit_args_infer(&ctor_fn, arguments, &free, &mut subst, expected);
            self.interop_args = saved_interop;
            for p in params {
                let t = subst.get(&p.id).cloned().unwrap_or_else(|| self.default_type_arg(&p.bound));
                targs.push(t.clone());
                subst.insert(p.id, t);
            }
            return self.finish_ctor_call(class, targs, fid, ctor_name, arg_js, is_const);
        }
        let saved_interop = self.interop_args;
        self.interop_args = fid.is_some_and(|f| self.ctx.is_js_member(f));
        let (arg_js, _) = self.emit_args_infer(&ctor_fn.subst(&subst), arguments, &[], &mut HashMap::new(), expected);
        self.interop_args = saved_interop;
        self.finish_ctor_call(class, targs, fid, ctor_name, arg_js, is_const)
    }

    /// Valor de `const bool/int/String.fromEnvironment(nome, defaultValue: v)`.
    /// Sem `-D` na linha de comando, o valor é sempre o `defaultValue`
    /// (`false`, `0` e `""` quando omitido, como manda a especificação).
    pub fn constante_de_ambiente(&mut self, class: ClassId, arguments: &ast::Arguments) -> Option<(Js, Ty)> {
        let (padrao, ty) = if Some(class) == self.ctx.bool_ {
            ("false".to_string(), self.ctx.t_bool())
        } else if Some(class) == self.ctx.int_ {
            ("0".to_string(), self.ctx.t_int())
        } else if Some(class) == self.ctx.string_ {
            ("\"\"".to_string(), self.ctx.t_string())
        } else {
            return None;
        };
        let default_arg = arguments
            .args
            .iter()
            .find(|a| a.name.as_ref().is_some_and(|n| self.name(n.sym) == "defaultValue"))
            .map(|a| a.value);
        let Some(arg) = default_arg else {
            return Some((Js::prim(padrao), ty));
        };
        let (js, jty) = self.emit_expr(arg, Some(&ty));
        // Só dobra quando o padrão já é um literal; qualquer outra forma cai no
        // caminho normal (e o programa não é válido em Dart de qualquer modo).
        let literal = matches!(js.code.as_str(), "true" | "false")
            || js.code.starts_with('"')
            || js.code.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-');
        if literal && !js.code.is_empty() {
            return Some((js, jty));
        }
        None
    }

    fn finish_ctor_call(&mut self, class: ClassId, targs: Vec<Ty>, fid: Option<dartforge_elements::model::FunctionElementId>, ctor_name: &str, mut arg_js: Vec<String>, is_const: bool) -> (Js, Ty) {
        let ty = Ty::Iface { class, args: targs.clone(), nullable: false };
        let cls = self.ctx.program.class(class);
        let cls_ref = self.class_ref(class);
        let jsname = if ctor_name.is_empty() { "new".to_string() } else { static_member_name(ctor_name) };
        let is_factory = fid.is_some_and(|f| self.ctx.program.function(f).factory);
        let generic = self.ctx.requires_rti(class);
        // Interop JS (`visitConstructorInvocation`/`_emitJSInteropNew`): tipo
        // `@anonymous` vira literal de objeto; construtor `external` (ou
        // sintético) é `new dart.global.Nome(args)`, sem rti; factory não
        // `external` fica na classe emitida.
        if let Some(jc) = self.ctx.js_classes.get(&class) {
            let external = fid.is_none_or(|f| {
                let fun = self.ctx.program.function(f);
                fun.external || fun.kind == FunctionKind::SyntheticConstructor
            });
            if external {
                // Literal de objeto: `@anonymous` (package:js) e construtor
                // `external` de extension type só com parâmetros nomeados
                // (`dart:js_interop`, compiler.dart:6948) — `{a: 1, b: 2}`.
                let so_nomeados = fid.is_some_and(|f| {
                    let ps = &self.ctx.outline.functions[f.0 as usize].parameters;
                    !ps.is_empty() && ps.iter().all(|p| p.kind == ast::ParameterKind::Named)
                });
                if jc.anonymous || so_nomeados {
                    let obj = arg_js.last().filter(|a| a.starts_with('{')).cloned().unwrap_or_else(|| "{}".to_string());
                    return (Js::prim(obj), ty);
                }
                return (Js::prim(format!("new {}({})", self.ctx.js_class_global(class), arg_js.join(", "))), ty);
            }
        }
        // Redirecionamento de factory `= Outra`: resolve o alvo.
        if is_factory {
            if let Some(target) = self.factory_redirect_target(fid.unwrap()) {
                let (tclass, tname, is_target_factory) = target;
                let tref = self.class_ref(tclass);
                let tgeneric = self.ctx.requires_rti(tclass);
                if tgeneric {
                    let tt = if self.ctx.class_params[tclass.0 as usize].len() == targs.len() { Some(Ty::Iface { class: tclass, args: targs.clone(), nullable: false }) } else { None };
                    arg_js.insert(0, self.rti(&tt.unwrap_or(ty.clone())));
                }
                let tjs = if tname.is_empty() { "new".to_string() } else { static_member_name(&tname) };
                let call = if is_target_factory {
                    format!("{tref}.{tjs}({})", arg_js.join(", "))
                } else {
                    format!("new {tref}.{tjs}({})", arg_js.join(", "))
                };
                let call = if is_const { format!("dart.const({call})") } else { call };
                self.m.note_class(class);
                return (Js::prim(call), ty);
            }
        }
        if generic {
            arg_js.insert(0, self.rti(&ty));
        }
        self.m.note_class(class);
        let _ = cls;
        let call = if is_factory {
            format!("{cls_ref}.{jsname}({})", arg_js.join(", "))
        } else {
            format!("new {cls_ref}.{jsname}({})", arg_js.join(", "))
        };
        let call = if is_const { format!("dart.const({call})") } else { call };
        (Js::prim(call), ty)
    }

    /// Alvo de `factory C.x(...) = D.y;`.
    pub fn factory_redirect_target(&self, fid: dartforge_elements::model::FunctionElementId) -> Option<(ClassId, String, bool)> {
        let fid = self.ctx.program.function(fid).patched_by.unwrap_or(fid);
        let f = self.ctx.program.function(fid);
        let dartforge_elements::model::FunctionRef::Constructor { unit, member } = f.node else { return None };
        let m = self.ctx.program.unit(unit).ast.member(member);
        let ast::MemberKind::Constructor(ctor) = &m.kind else { return None };
        let redirect = ctor.redirect.as_ref()?;
        // Resolve o tipo alvo no contexto da unidade do construtor.
        let sub = FnEmitter::new(self.ctx, self.m, unit, f.class, true);
        let mut t = sub.resolve_type(redirect.ty);
        let mut name = redirect.constructor.map(|n| self.name(n.sym).to_string()).unwrap_or_default();
        // `= Classe.nome` chega como tipo com duas partes (`prefixo.Tipo`): se a
        // primeira parte é uma classe, a segunda é o nome do construtor.
        if !matches!(t, Ty::Iface { .. }) && redirect.constructor.is_none() {
            if let ast::TypeKind::Named { name: parts, .. } = &sub.ast().ty(redirect.ty).kind {
                if parts.len() == 2 {
                    if let Some(b) = self.ctx.program.lookup(sub.lib, parts[0].sym) {
                        if let Some(Element::Class(c)) = b.getter {
                            t = Ty::iface(c);
                            name = self.name(parts[1].sym).to_string();
                        }
                    }
                }
            }
        }
        let Ty::Iface { class, .. } = t else { return None };
        let key = if name.is_empty() { self.ctx.empty_sym } else { self.ctx.sym(&name) };
        let tfid = key.and_then(|k| self.ctx.program.class(class).constructors.get(&k).copied());
        let is_factory = tfid.is_some_and(|x| self.ctx.program.function(x).factory);
        // Se o alvo também redireciona, segue.
        if let Some(tf) = tfid {
            if is_factory {
                if let Some(next) = self.factory_redirect_target(tf) {
                    return Some(next);
                }
            }
        }
        Some((class, name, is_factory))
    }
}

/// Nome Dart do operador binário.
pub fn binop_name(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::TruncDiv => "~/",
        BinaryOp::Rem => "%",
        BinaryOp::Shl => "<<",
        BinaryOp::Shr => ">>",
        BinaryOp::UShr => ">>>",
        BinaryOp::BitAnd => "&",
        BinaryOp::BitOr => "|",
        BinaryOp::BitXor => "^",
        BinaryOp::Eq => "==",
        BinaryOp::NotEq => "!=",
        BinaryOp::Lt => "<",
        BinaryOp::Gt => ">",
        BinaryOp::LtEq => "<=",
        BinaryOp::GtEq => ">=",
        BinaryOp::And => "&&",
        BinaryOp::Or => "||",
        BinaryOp::IfNull => "??",
    }
}

fn parse_int(text: &str) -> String {
    if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        match u64::from_str_radix(hex, 16) {
            Ok(v) => v.to_string(),
            Err(_) => text.to_string(),
        }
    } else {
        let t = text.trim_start_matches('0');
        if t.is_empty() { "0".to_string() } else { t.to_string() }
    }
}

pub fn is_simple(code: &str) -> bool {
    code == "this" || code.chars().all(|c| c.is_alphanumeric() || c == '$' || c == '_')
}

impl Ty {
    pub fn non_null_if(&self, cond: bool) -> Ty {
        if cond { self.non_null() } else { self.clone() }
    }
}

#[allow(dead_code)]
fn _unused(_: ClassKind, _: LibraryId) {}
