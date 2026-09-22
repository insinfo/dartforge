//! Chamadas, argumentos, inferência de argumentos de tipo e closures.

use crate::body::{static_member_name, AsyncKind, FnEmitter};
use crate::ctx::MemberKind;
use crate::expr::{Guard, IdentTarget};
use crate::js::{self, Js, P_ASSIGN, P_PRIMARY};
use crate::ty::{Ty, TyParam};
use dartforge_elements::model::{ClassId, Element, FunctionKind};
use dartforge_frontend::ast::{self, ExprId, ExprKind};
use std::collections::HashMap;

impl<'m, 'a> FnEmitter<'m, 'a> {
    /// `f(args)`, `a.m(args)`, `C.named(args)`, `p.f(args)`…
    pub fn emit_call(&mut self, target: ExprId, arguments: &ast::Arguments, expected: Option<&Ty>) -> (Js, Ty, Vec<Guard>) {
        let t = self.expr(target);
        match &t.kind {
            ExprKind::Property { target: recv, name, null_aware } => {
                let n = self.name(name.sym).to_string();
                if let Some(r) = self.try_static_call(*recv, &n, arguments, expected) {
                    return (r.0, r.1, vec![]);
                }
                if let ExprKind::Super = self.expr(*recv).kind {
                    let sup_ty = self.super_ty();
                    let (js, ty) = self.emit_method_call(&Js::prim(self.super_ref()), &sup_ty, &n, arguments, expected, true);
                    return (js, ty, vec![]);
                }
                let (rjs, rty, mut guards) = self.emit_target(*recv);
                let (recv, recv_ty) = if *null_aware {
                    let tmp = self.temp();
                    guards.push(Guard { temp: tmp.clone(), init: rjs.code });
                    (Js::prim(tmp), rty.non_null())
                } else {
                    (rjs, rty)
                };
                let (js, ty) = self.emit_method_call(&recv, &recv_ty, &n, arguments, expected, false);
                let ty = if *null_aware { ty.with_nullable(true) } else { ty };
                (js, ty, guards)
            }
            ExprKind::Identifier(id) => {
                let n = self.name(id.sym).to_string();
                match self.resolve_ident(id.sym) {
                    IdentTarget::Local(js, ty) => {
                        let (r, rt) = self.emit_fn_value_call(&Js::prim(js), &ty, arguments, expected);
                        (r, rt, vec![])
                    }
                    IdentTarget::ThisMember(_) => {
                        let this_ty = self.class.map(|c| self.ctx.this_ty(c)).unwrap_or(Ty::Dynamic);
                        let (js, ty) = self.emit_method_call(&Js::prim("this"), &this_ty, &n, arguments, expected, false);
                        (js, ty, vec![])
                    }
                    IdentTarget::Static(c, mk) => {
                        let (js, ty) = self.emit_static_call(c, &n, mk, arguments, expected);
                        (js, ty, vec![])
                    }
                    IdentTarget::Element(el) => {
                        let (js, ty) = self.emit_element_call(el, &n, arguments, expected);
                        (js, ty, vec![])
                    }
                    IdentTarget::ExtThisMember(_) => {
                        let t = self.extension_this.clone().unwrap_or(Ty::Dynamic);
                        let (js, ty) = self.emit_method_call(&Js::prim("$this"), &t, &n, arguments, expected, false);
                        (js, ty, vec![])
                    }
                    IdentTarget::ThisExt => {
                        let t = self.class.map(|c| self.ctx.this_ty(c)).unwrap_or(Ty::Dynamic);
                        let (js, ty) = self.emit_method_call(&Js::prim("this"), &t, &n, arguments, expected, false);
                        (js, ty, vec![])
                    }
                    IdentTarget::ExtMember(ext, fid) => {
                        let t = self.extension_this.clone().unwrap_or(Ty::Dynamic);
                        let _ = (ext, fid);
                        let (js, ty) = self.emit_method_call(&Js::prim("$this"), &t, &n, arguments, expected, false);
                        (js, ty, vec![])
                    }
                    IdentTarget::ExtStatic(ext, _) => {
                        let (js, ty) = self.static_extension_call(ext, &n, arguments, expected).unwrap_or((Js::prim("null"), Ty::Dynamic));
                        (js, ty, vec![])
                    }
                    IdentTarget::ExtField(ext) => {
                        let e = self.ctx.program.extension(ext);
                        let lib_var = self.lib_var(e.library);
                        let ext_name = self.extension_js_name(ext);
                        let ty = e.fields.iter().find(|v| self.ctx.program.variable(**v).name == id.sym).map(|v| self.ctx.var_ty(*v)).unwrap_or(Ty::Dynamic);
                        let f = Js::prim(format!("{lib_var}[{}]", js::string_literal(&format!("{ext_name}|{n}"))));
                        let (r, rt) = self.emit_fn_value_call(&f, &ty, arguments, expected);
                        (r, rt, vec![])
                    }
                    IdentTarget::TypeParam(_) | IdentTarget::Prefix(_) | IdentTarget::Unknown => {
                        let (args, _, _) = self.emit_args_plain(arguments);
                        (Js::prim(format!("dart.dcall({}, [{}])", js::ident(&n), args.join(", "))), Ty::Dynamic, vec![])
                    }
                }
            }
            _ => {
                let (fjs, fty) = self.emit_expr(target, None);
                let (r, rt) = self.emit_fn_value_call(&fjs, &fty, arguments, expected);
                (r, rt, vec![])
            }
        }
    }

    /// Chamada de um valor de função.
    pub fn emit_fn_value_call(&mut self, f: &Js, fty: &Ty, arguments: &ast::Arguments, expected: Option<&Ty>) -> (Js, Ty) {
        match fty {
            Ty::Fn { nullable: false, .. } => {
                let (args, ret, targs) = self.emit_args_for(fty, arguments, expected);
                let mut all = targs;
                all.extend(args);
                (Js::prim(format!("{}({})", f.at(P_PRIMARY), all.join(", "))), ret)
            }
            _ => {
                let (args, named, _) = self.emit_args_plain(arguments);
                let named_js = named.map(|n| format!(", {n}")).unwrap_or_default();
                if !arguments.type_args.is_empty() {
                    let targs: Vec<String> = arguments.type_args.iter().map(|t| self.rti(&self.resolve_type(*t))).collect();
                    return (Js::prim(format!("dart.dgcall({}, [{}], [{}]{named_js})", f.code, targs.join(", "), args.join(", "))), Ty::Dynamic);
                }
                (Js::prim(format!("dart.dcall({}, [{}]{named_js})", f.code, args.join(", "))), Ty::Dynamic)
            }
        }
    }

    /// Argumentos sem tipagem (chamada dinâmica): (posicionais, objeto nomeado, tipos).
    pub fn emit_args_plain(&mut self, arguments: &ast::Arguments) -> (Vec<String>, Option<String>, Vec<Ty>) {
        let mut pos = Vec::new();
        let mut named = Vec::new();
        let mut tys = Vec::new();
        for a in arguments.args.iter() {
            let (js, ty) = self.emit_expr(a.value, None);
            tys.push(ty);
            match a.name {
                Some(n) => named.push(format!("{}: {}", js::prop_key(self.name(n.sym)), js.at(P_ASSIGN))),
                None => pos.push(js.into_at(P_ASSIGN)),
            }
        }
        let named_obj = if named.is_empty() { None } else { Some(format!("{{{}}}", named.join(", "))) };
        (pos, named_obj, tys)
    }

    /// Emite argumentos para um tipo de função conhecido; devolve (args JS, tipo de retorno substituído, rtis dos argumentos de tipo).
    pub fn emit_args_for(&mut self, fty: &Ty, arguments: &ast::Arguments, expected: Option<&Ty>) -> (Vec<String>, Ty, Vec<String>) {
        let Ty::Fn { type_params, ret, .. } = fty else {
            let (args, named, _) = self.emit_args_plain(arguments);
            let mut all = args;
            if let Some(n) = named {
                all.push(n);
            }
            return (all, Ty::Dynamic, vec![]);
        };
        let free: Vec<u32> = type_params.iter().map(|p| p.id).collect();
        let mut subst = HashMap::new();
        let inner = match fty {
            Ty::Fn { ret, pos, opt, named, nullable, .. } => Ty::Fn { type_params: vec![], ret: ret.clone(), pos: pos.clone(), opt: opt.clone(), named: named.clone(), nullable: *nullable },
            t => t.clone(),
        };
        let (args, _) = self.emit_args_infer(&inner, arguments, &free, &mut subst, expected);
        let mut targs = Vec::new();
        for p in type_params {
            let t = subst.get(&p.id).cloned().unwrap_or_else(|| self.default_type_arg(&p.bound));
            subst.insert(p.id, t.clone());
            targs.push(self.rti(&t));
        }
        (args, ret.subst(&subst), targs)
    }

    /// Núcleo: emite argumentos casando com a assinatura `fty` (sem parâmetros
    /// de tipo próprios), inferindo os parâmetros livres `free` em `subst`.
    pub fn emit_args_infer(
        &mut self,
        fty: &Ty,
        arguments: &ast::Arguments,
        free: &[u32],
        subst: &mut HashMap<u32, Ty>,
        expected_ret: Option<&Ty>,
    ) -> (Vec<String>, Vec<Ty>) {
        let (pos_p, opt_p, named_p, ret) = match fty {
            Ty::Fn { pos, opt, named, ret, .. } => (pos.clone(), opt.clone(), named.clone(), (**ret).clone()),
            _ => (vec![], vec![], vec![], Ty::Dynamic),
        };
        // Só os argumentos diretos desta chamada passam por `assertInterop`.
        let interop = std::mem::replace(&mut self.interop_args, false);
        // Argumentos de tipo explícitos.
        if !arguments.type_args.is_empty() {
            for (p, t) in free.iter().zip(arguments.type_args.iter()) {
                let ty = self.resolve_type(*t);
                subst.insert(*p, ty);
            }
        }
        let n = arguments.args.len();
        let mut out: Vec<Option<String>> = vec![None; n];
        let mut tys: Vec<Ty> = vec![Ty::Dynamic; n];
        let mut param_of: Vec<Option<Ty>> = vec![None; n];
        let mut pi = 0;
        for (i, a) in arguments.args.iter().enumerate() {
            match a.name {
                Some(nm) => {
                    let name = self.name(nm.sym);
                    param_of[i] = named_p.iter().find(|(x, _, _)| x == name).map(|(_, t, _)| t.clone());
                }
                None => {
                    param_of[i] = if pi < pos_p.len() {
                        Some(pos_p[pi].clone())
                    } else {
                        opt_p.get(pi - pos_p.len()).cloned()
                    };
                    pi += 1;
                }
            }
        }
        let is_closure = |s: &Self, e: ExprId| matches!(s.expr(e).kind, ExprKind::FunctionExpression(_));
        // Fase 1: argumentos que não são closures.
        for (i, a) in arguments.args.iter().enumerate() {
            if is_closure(self, a.value) {
                continue;
            }
            let is_literal = matches!(
                self.expr(a.value).kind,
                ExprKind::List { .. } | ExprKind::SetOrMap { .. } | ExprKind::Record { .. } | ExprKind::Int(_) | ExprKind::Double(_)
            );
            let expected = param_of[i].as_ref().map(|p| p.subst(subst)).and_then(|p| {
                if !mentions_any(&p, free) {
                    Some(p)
                } else if is_literal {
                    None
                } else {
                    // Parâmetros ainda livres viram dynamic (só para coerções de função).
                    let mut m = HashMap::new();
                    for f in free {
                        m.insert(*f, Ty::Dynamic);
                    }
                    Some(p.subst(&m))
                }
            });
            let (js, ty) = self.emit_expr(a.value, expected.as_ref());
            if let Some(p) = &param_of[i] {
                if !free.is_empty() {
                    self.match_type(p, &ty, free, subst);
                }
            }
            let js = if interop { self.assert_interop(js, &ty) } else { js };
            out[i] = Some(js.into_at(P_ASSIGN));
            tys[i] = ty;
        }
        // Fase 2: closures.
        for (i, a) in arguments.args.iter().enumerate() {
            if !is_closure(self, a.value) {
                continue;
            }
            let expected = param_of[i].as_ref().map(|p| {
                // Parâmetros livres ainda não ligados viram dynamic nos tipos dos parâmetros da closure.
                let mut m = subst.clone();
                for f in free {
                    m.entry(*f).or_insert(Ty::Dynamic);
                }
                let p2 = p.subst(subst);
                // Mantém o retorno com parâmetros livres para casar depois.
                match (&p2, p.subst(&m)) {
                    (Ty::Fn { ret, type_params, .. }, Ty::Fn { pos, opt, named, nullable, .. }) => {
                        Ty::Fn { type_params: type_params.clone(), ret: ret.clone(), pos, opt, named, nullable }
                    }
                    (_, other) => other,
                }
            });
            let (js, ty) = self.emit_expr(a.value, expected.as_ref());
            if let Some(p) = &param_of[i] {
                if !free.is_empty() {
                    self.match_type(p, &ty, free, subst);
                }
            }
            let js = if interop { self.assert_interop(js, &ty) } else { js };
            out[i] = Some(js.into_at(P_ASSIGN));
            tys[i] = ty;
        }
        // Retorno esperado (só para o que ainda falta).
        if let Some(e) = expected_ret {
            if !free.is_empty() {
                let mut tmp = subst.clone();
                self.match_type(&ret, e, free, &mut tmp);
                for f in free {
                    if !subst.contains_key(f) {
                        if let Some(t) = tmp.get(f) {
                            if !t.mentions_params() && !matches!(t, Ty::Dynamic) {
                                subst.insert(*f, t.clone());
                            }
                        }
                    }
                }
            }
        }
        // Monta a lista: posicionais na ordem, nomeados no objeto final.
        let mut pos_js = Vec::new();
        let mut named_js = Vec::new();
        for (i, a) in arguments.args.iter().enumerate() {
            let js = out[i].clone().unwrap_or_else(|| "null".into());
            match a.name {
                Some(nm) => named_js.push(format!("{}: {}", js::prop_key(self.name(nm.sym)), js)),
                None => pos_js.push(js),
            }
        }
        if !named_js.is_empty() {
            // Preenche opcionais posicionais omitidos entre os fornecidos e o objeto? Não: o
            // objeto `opts` é o parâmetro seguinte aos posicionais declarados.
            let declared_pos = pos_p.len() + opt_p.len();
            while pos_js.len() < declared_pos {
                pos_js.push("void 0".into());
            }
            pos_js.push(format!("{{{}}}", named_js.join(", ")));
        }
        self.interop_args = interop;
        (pos_js, tys)
    }

    /// Chamada de método de instância `recv.name(args)`.
    pub fn emit_method_call(&mut self, recv: &Js, recv_ty: &Ty, name: &str, arguments: &ast::Arguments, expected: Option<&Ty>, is_super: bool) -> (Js, Ty) {
        let recv_nn = recv_ty.non_null();
        // Membros de Object com helpers.
        let user = self.is_user_class_ty(&recv_nn) || is_super;
        match name {
            "toString" if !user && arguments.args.is_empty() => {
                return (Js::prim(format!("dart.toString({})", recv.code)), self.ctx.t_string());
            }
            "noSuchMethod" if !user && arguments.args.len() == 1 => {
                let (a, _) = self.emit_expr(arguments.args[0].value, None);
                return (Js::prim(format!("dart.noSuchMethod({}, {})", recv.code, a.code)), Ty::Dynamic);
            }
            _ => {}
        }
        if self.forced_ext.is_some() {
            if let Some(r) = self.try_extension_call(recv, &recv_nn, name, arguments, expected) {
                return r;
            }
            self.forced_ext = None;
        }
        if recv_ty.is_dynamic() {
            return self.emit_dsend(recv, name, arguments);
        }
        // Tipo função: `.call`.
        if let Ty::Fn { .. } = &recv_nn {
            if name == "call" {
                return self.emit_fn_value_call(recv, &recv_nn, arguments, expected);
            }
        }
        match self.ctx.lookup_member(&recv_nn, name, false) {
            Some(m) => {
                let mty = self.ctx.member_ty(&m);
                let access = self.member_access(&recv_nn, name, false);
                match m.kind {
                    MemberKind::Method(_) if self.ctx.is_js_member_kind(&m.kind) => {
                        // Interop: sem argumentos de tipo, `assertInterop` nas funções,
                        // `jsInteropNullCheck` no retorno não anulável.
                        let saved = self.interop_args;
                        self.interop_args = true;
                        let (args, ret, _) = self.emit_args_for(&mty, arguments, expected);
                        self.interop_args = saved;
                        (self.js_null_check(Js::prim(format!("{}{access}({})", recv.at(P_PRIMARY), args.join(", "))), &m.kind), ret)
                    }
                    MemberKind::Method(_) => {
                        let (args, ret, targs) = self.emit_args_for(&mty, arguments, expected);
                        let mut all = targs;
                        all.extend(args);
                        if is_super && self.async_kind != AsyncKind::None {
                            return (Js::prim(format!("{}{access}.call(this{})", recv.at(P_PRIMARY), all.iter().map(|a| format!(", {a}")).collect::<String>())), ret);
                        }
                        (Js::prim(format!("{}{access}({})", recv.at(P_PRIMARY), all.join(", "))), ret)
                    }
                    _ => {
                        // Getter/campo devolvendo função.
                        let getter = Js::prim(format!("{}{access}", recv.at(P_PRIMARY)));
                        self.emit_fn_value_call(&getter, &mty, arguments, expected)
                    }
                }
            }
            None => {
                if let Some(r) = self.try_extension_call(recv, &recv_nn, name, arguments, expected) {
                    return r;
                }
                if let Ty::Record { .. } = &recv_nn {
                    let (g, gty) = self.emit_member_get(recv, &recv_nn, name, None);
                    return self.emit_fn_value_call(&g, &gty, arguments, expected);
                }
                self.emit_dsend(recv, name, arguments)
            }
        }
    }

    pub fn is_user_class_ty(&self, t: &Ty) -> bool {
        match t {
            Ty::Iface { class, .. } => !self.ctx.libs[self.ctx.lib_of_class(*class).0 as usize].is_sdk,
            _ => false,
        }
    }

    pub fn emit_dsend(&mut self, recv: &Js, name: &str, arguments: &ast::Arguments) -> (Js, Ty) {
        let (args, named, _) = self.emit_args_plain(arguments);
        let named_js = named.map(|n| format!(", {n}")).unwrap_or_default();
        let jsname = crate::body::js_member_name(name);
        if !arguments.type_args.is_empty() {
            let targs: Vec<String> = arguments.type_args.iter().map(|t| self.rti(&self.resolve_type(*t))).collect();
            return (Js::prim(format!("dart.dgsend({}, [{}], {}, [{}]{named_js})", recv.code, targs.join(", "), js::string_literal(&jsname), args.join(", "))), Ty::Dynamic);
        }
        (Js::prim(format!("dart.dsend({}, {}, [{}]{named_js})", recv.code, js::string_literal(&jsname), args.join(", "))), Ty::Dynamic)
    }

    fn try_extension_call(&mut self, recv: &Js, recv_ty: &Ty, name: &str, arguments: &ast::Arguments, expected: Option<&Ty>) -> Option<(Js, Ty)> {
        let found = self.find_extension_member(recv_ty, name, false);
        self.forced_ext = None;
        let (ext, fid, subst) = found?;
        let f = self.ctx.program.function(fid);
        let e = self.ctx.program.extension(ext);
        let ext_name = self.extension_js_name(ext);
        let lib_var = self.lib_var(e.library);
        let ext_tps: Vec<u32> = self.ctx.outline.extensions[ext.0 as usize].type_params.iter().map(|p| p.0).collect();
        let fty = self.ctx.fn_ty(fid);
        match f.kind {
            FunctionKind::Function | FunctionKind::Operator => {
                // Parâmetros livres: os da extensão (parcialmente ligados) + os do método.
                let (mtps, inner) = match &fty {
                    Ty::Fn { type_params, ret, pos, opt, named, nullable } => (
                        type_params.clone(),
                        Ty::Fn { type_params: vec![], ret: ret.clone(), pos: pos.clone(), opt: opt.clone(), named: named.clone(), nullable: *nullable },
                    ),
                    t => (vec![], t.clone()),
                };
                let mut free: Vec<u32> = ext_tps.clone();
                free.extend(mtps.iter().map(|p| p.id));
                let mut s = subst.clone();
                let (args, _) = self.emit_args_infer(&inner, arguments, &free, &mut s, expected);
                let mut all: Vec<String> = ext_tps.iter().map(|p| self.rti(s.get(p).unwrap_or(&Ty::Dynamic))).collect();
                for p in &mtps {
                    let t = s.get(&p.id).cloned().unwrap_or_else(|| self.default_type_arg(&p.bound));
                    s.insert(p.id, t.clone());
                    all.push(self.rti(&t));
                }
                all.push(recv.code.clone());
                all.extend(args);
                let ret = match &inner {
                    Ty::Fn { ret, .. } => ret.subst(&s),
                    _ => Ty::Dynamic,
                };
                Some((Js::prim(format!("{lib_var}[{}]({})", js::string_literal(&format!("{ext_name}|{name}")), all.join(", "))), ret))
            }
            FunctionKind::Getter => {
                let (g, gty) = self.try_extension_get(recv, recv_ty, name)?;
                Some(self.emit_fn_value_call(&g, &gty, arguments, expected))
            }
            _ => None,
        }
    }

    /// `C.m(args)`, `C.named(args)`, `p.f(args)`, `p.C.m(args)`, `E.method(...)` (extensão estática).
    fn try_static_call(&mut self, recv: ExprId, name: &str, arguments: &ast::Arguments, expected: Option<&Ty>) -> Option<(Js, Ty)> {
        let t = self.expr(recv);
        match &t.kind {
            ExprKind::Identifier(id) => match self.resolve_ident(id.sym) {
                IdentTarget::Prefix(p) => {
                    if name == "loadLibrary" {
                        self.m.use_sdk("async");
                        let t = self.ctx.t_future(Ty::Void);
                        let rti = self.rti(&t);
                        return Some((Js::prim(format!("async.Future.value({rti}, null)")), t));
                    }
                    let sym = self.ctx.sym(name)?;
                    let b = self.ctx.program.lookup_prefixed(self.lib, p, sym)?;
                    let el = b.getter?;
                    Some(self.emit_element_call(el, name, arguments, expected))
                }
                IdentTarget::Element(Element::Class(c)) => self.static_call_on_class(c, name, arguments, expected),
                IdentTarget::Element(Element::Extension(ext)) => self.static_extension_call(ext, name, arguments, expected),
                IdentTarget::Element(Element::Typedef(td)) => {
                    let data = &self.ctx.outline.typedefs[td.0 as usize];
                    if let Ty::Iface { class, .. } = self.ctx.ty_of(data.target_type) {
                        return self.static_call_on_class(class, name, arguments, expected);
                    }
                    None
                }
                _ => None,
            },
            ExprKind::Property { target: t2, name: n2, .. } => {
                if let ExprKind::Identifier(id) = &self.expr(*t2).kind {
                    if let IdentTarget::Prefix(p) = self.resolve_ident(id.sym) {
                        let sym = self.ctx.sym(self.name(n2.sym))?;
                        let b = self.ctx.program.lookup_prefixed(self.lib, p, sym)?;
                        if let Some(Element::Class(c)) = b.getter {
                            return self.static_call_on_class(c, name, arguments, expected);
                        }
                    }
                }
                None
            }
            ExprKind::TypeArguments { target: t2, type_args } if matches!(&self.expr(*t2).kind, ExprKind::Property { .. }) => {
                // `p.C<int>.named(args)`
                if let ExprKind::Property { target: t3, name: n3, .. } = &self.expr(*t2).kind {
                    if let ExprKind::Identifier(id) = &self.expr(*t3).kind {
                        if let IdentTarget::Prefix(p) = self.resolve_ident(id.sym) {
                            let sym = self.ctx.sym(self.name(n3.sym))?;
                            let b = self.ctx.program.lookup_prefixed(self.lib, p, sym)?;
                            if let Some(Element::Class(c)) = b.getter {
                                let targs: Vec<Ty> = type_args.iter().map(|t| self.resolve_type(*t)).collect();
                                let cname = if name == "new" { "" } else { name };
                                let is_const = self.in_const;
                                return Some(self.emit_constructor_call(c, targs, true, cname, arguments, expected, is_const));
                            }
                        }
                    }
                }
                None
            }
            ExprKind::TypeArguments { target: t2, type_args } => {
                // `C<int>.named(args)`
                if let ExprKind::Identifier(id) = &self.expr(*t2).kind {
                    if let IdentTarget::Element(Element::Class(c)) = self.resolve_ident(id.sym) {
                        let targs: Vec<Ty> = type_args.iter().map(|t| self.resolve_type(*t)).collect();
                        let cname = if name == "new" { "" } else { name };
                        let is_const = self.in_const;
                        return Some(self.emit_constructor_call(c, targs, true, cname, arguments, expected, is_const));
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn static_call_on_class(&mut self, c: ClassId, name: &str, arguments: &ast::Arguments, expected: Option<&Ty>) -> Option<(Js, Ty)> {
        let mut cur = Some(c);
        while let Some(k) = cur {
            if let Some(mk) = self.ctx.declared_static(k, name, false) {
                return Some(self.emit_static_call(k, name, mk, arguments, expected));
            }
            cur = self.ctx.superclass_of(k);
        }
        // Construtor nomeado.
        let cname = if name == "new" { "" } else { name };
        let key = if cname.is_empty() { self.ctx.empty_sym } else { self.ctx.sym(cname) };
        if key.is_some_and(|k| self.ctx.program.class(c).constructors.contains_key(&k)) {
            let is_const = self.in_const;
            return Some(self.emit_constructor_call(c, vec![], false, cname, arguments, expected, is_const));
        }
        None
    }

    fn static_extension_call(&mut self, ext: dartforge_elements::model::ExtensionId, name: &str, arguments: &ast::Arguments, expected: Option<&Ty>) -> Option<(Js, Ty)> {
        let e = self.ctx.program.extension(ext);
        let sym = self.ctx.sym(name)?;
        let ext_name = self.extension_js_name(ext);
        let lib_var = self.lib_var(e.library);
        if let Some(&fid) = e.static_members.get(&sym) {
            let fty = self.ctx.fn_ty(fid);
            let (args, ret, targs) = self.emit_args_for(&fty, arguments, expected);
            let mut all = targs;
            all.extend(args);
            return Some((Js::prim(format!("{lib_var}[{}]({})", js::string_literal(&format!("{ext_name}|{name}")), all.join(", "))), ret));
        }
        // `Ext(x).m()` é tratado como chamada em `Ext(x)`; aqui só estáticos.
        None
    }

    pub fn emit_static_call(&mut self, c: ClassId, name: &str, mk: MemberKind, arguments: &ast::Arguments, expected: Option<&Ty>) -> (Js, Ty) {
        // Constante de ambiente chamada como estático (`X.fromEnvironment(...)`
        // sem `const`): o valor é o mesmo, avaliado na compilação.
        if name == "fromEnvironment" {
            if let Some(r) = self.constante_de_ambiente(c, arguments) {
                return r;
            }
        }
        if self.ctx.is_js_class(c) && self.ctx.is_js_member_kind(&mk) {
            let js = self.ctx.js_static_ref(Some(c), self.ctx.lib_of_class(c), &mk, name);
            return match mk {
                MemberKind::Method(fid) => {
                    let fty = self.ctx.fn_ty(fid);
                    let saved = self.interop_args;
                    self.interop_args = true;
                    let (args, ret, _) = self.emit_args_for(&fty, arguments, expected);
                    self.interop_args = saved;
                    (self.js_null_check(Js::prim(format!("{js}({})", args.join(", "))), &mk), ret)
                }
                MemberKind::Field(vid) => {
                    let ty = self.ctx.var_ty(vid);
                    self.emit_fn_value_call(&Js::prim(js), &ty, arguments, expected)
                }
                MemberKind::Getter(fid) => {
                    let ty = self.ctx.ty_of(self.ctx.outline.functions[fid.0 as usize].return_type);
                    self.emit_fn_value_call(&Js::prim(js), &ty, arguments, expected)
                }
                MemberKind::Setter(_) => (Js::prim("null"), Ty::Dynamic),
            };
        }
        let cls = self.class_ref(c);
        let js = format!("{cls}{}", js::prop_access(&static_member_name(name)));
        match mk {
            MemberKind::Method(fid) => {
                let fty = self.ctx.fn_ty(fid);
                let (args, ret, targs) = self.emit_args_for(&fty, arguments, expected);
                let mut all = targs;
                all.extend(args);
                (Js::prim(format!("{js}({})", all.join(", "))), ret)
            }
            MemberKind::Field(vid) => {
                let ty = self.ctx.var_ty(vid);
                self.emit_fn_value_call(&Js::prim(js), &ty, arguments, expected)
            }
            MemberKind::Getter(fid) => {
                let ty = self.ctx.ty_of(self.ctx.outline.functions[fid.0 as usize].return_type);
                self.emit_fn_value_call(&Js::prim(js), &ty, arguments, expected)
            }
            MemberKind::Setter(_) => (Js::prim("null"), Ty::Dynamic),
        }
    }

    pub fn emit_element_call(&mut self, el: Element, name: &str, arguments: &ast::Arguments, expected: Option<&Ty>) -> (Js, Ty) {
        match el {
            Element::Function(fid) => {
                let f = self.ctx.program.function(fid);
                if f.kind == FunctionKind::Getter {
                    let (g, gty) = self.element_ref(el).map(|(j, _)| (Js::prim(j), self.ctx.ty_of(self.ctx.outline.functions[fid.0 as usize].return_type))).expect("getter");
                    return self.emit_fn_value_call(&g, &gty, arguments, expected);
                }
                let (js, fty) = self.element_ref(el).expect("função");
                if self.ctx.is_js_member(fid) {
                    let saved = self.interop_args;
                    self.interop_args = true;
                    let (args, ret, _) = self.emit_args_for(&fty, arguments, expected);
                    self.interop_args = saved;
                    return (self.js_null_check(Js::prim(format!("{js}({})", args.join(", "))), &MemberKind::Method(fid)), ret);
                }
                let (args, ret, targs) = self.emit_args_for(&fty, arguments, expected);
                let mut all = targs;
                all.extend(args);
                (Js::prim(format!("{js}({})", all.join(", "))), ret)
            }
            Element::Variable(_) => {
                let (js, ty) = self.element_ref(el).expect("variável");
                self.emit_fn_value_call(&Js::prim(js), &ty, arguments, expected)
            }
            Element::Class(c) => {
                // Extension type: `E(x)` é o valor representado (nos de
                // interop, `X(...)` é o construtor JS).
                let class = self.ctx.program.class(c);
                if class.kind == dartforge_elements::model::ClassKind::ExtensionType && !self.ctx.is_js_class(c) {
                    if let Some(a) = arguments.args.first() {
                        let (js, _) = self.emit_expr(a.value, None);
                        return (js, self.ctx.this_ty(c));
                    }
                }
                let is_const = self.in_const;
                self.emit_constructor_call(c, vec![], false, "", arguments, expected, is_const)
            }
            Element::Extension(ext) => {
                // `Ext(x)` — valor para acesso explícito a membros da extensão.
                let _ = ext;
                if let Some(a) = arguments.args.first() {
                    let (js, ty) = self.emit_expr(a.value, None);
                    return (js, ty);
                }
                (Js::prim("null"), Ty::Dynamic)
            }
            Element::Typedef(td) => {
                let data = &self.ctx.outline.typedefs[td.0 as usize];
                if let Ty::Iface { class, args, .. } = self.ctx.ty_of(data.target_type) {
                    let is_const = self.in_const;
                    return self.emit_constructor_call(class, args.clone(), !args.is_empty(), "", arguments, expected, is_const);
                }
                (Js::prim("null"), Ty::Dynamic)
            }
            Element::Prefix(..) => (Js::prim("null"), Ty::Dynamic),
        }
    }

    // -----------------------------------------------------------------------
    // Closures
    // -----------------------------------------------------------------------

    /// Emite uma expressão de função (closure ou função local). Devolve
    /// `dart.fn(...)`/`dart.gFn(...)` e o tipo.
    pub fn emit_function_expr(&mut self, fid: ast::FunctionId, expected: Option<&Ty>, _closure: bool) -> (Js, Ty) {
        let f = self.ast().function(fid);
        // Parâmetros de tipo próprios.
        let mut tps: Vec<TyParam> = Vec::new();
        let mut scope_params = Vec::new();
        for tp in f.type_params.iter() {
            let p = self.ctx.fresh_param(self.name(tp.name.sym), self.ctx.t_object_q());
            scope_params.push((p.id, js::ident(&p.name)));
            tps.push(p);
        }
        let saved_tps = self.fn_type_params.clone();
        let mut new_tps = scope_params.clone();
        new_tps.extend(saved_tps.iter().cloned());
        self.fn_type_params = new_tps;
        for (tp, p) in f.type_params.iter().zip(tps.iter_mut()) {
            if let Some(b) = tp.bound {
                let bt = self.resolve_type(b);
                self.ctx.param_bounds.borrow_mut().insert(p.id, bt.clone());
                p.bound = Box::new(bt);
            }
        }
        let expected_fn = match expected {
            Some(Ty::Fn { .. }) => expected.cloned(),
            _ => None,
        };
        let declared_ret = f.return_type.map(|r| self.resolve_type(r));
        let kind = match f.modifier {
            ast::AsyncModifier::None => AsyncKind::None,
            ast::AsyncModifier::Async => AsyncKind::Async,
            ast::AsyncModifier::AsyncStar => AsyncKind::AsyncStar,
            ast::AsyncModifier::SyncStar => AsyncKind::SyncStar,
        };
        let expected_ret = match &expected_fn {
            Some(Ty::Fn { ret, .. }) => Some((**ret).clone()),
            _ => None,
        };
        let ret_for_body = declared_ret.clone().or(expected_ret.clone()).unwrap_or(Ty::Dynamic);
        let saved = self.enter_fn(kind, ret_for_body.clone(), self.is_static);
        self.is_closure_body = true;
        let params: &[ast::Parameter] = f.parameters.as_deref().unwrap_or(&[]);
        let (param_js, prologue) = self.declare_params(params, expected_fn.as_ref());
        // Tipo dos parâmetros já declarados: lê do escopo.
        let (pos, opt, named) = self.declared_param_tys(params);
        self.emit_body(&f.body);
        let (body, returns) = self.exit_fn(saved);
        self.is_closure_body = false;
        // Tipo de retorno: declarado, senão inferido do corpo (ou do contexto).
        let inferred = if returns.is_empty() {
            if matches!(f.body, ast::FunctionBody::Expression(_)) { None } else { Some(Ty::Void) }
        } else {
            let mut it = returns.into_iter();
            let first = it.next().unwrap();
            Some(it.fold(first, |a, b| self.ctx.lub(&a, &b)))
        };
        let inner_ret = match (&declared_ret, inferred) {
            (Some(d), _) => d.clone(),
            (None, Some(i)) => {
                let i = match kind {
                    AsyncKind::Async => match i {
                        Ty::FutureOr { arg, .. } => (*arg).clone(),
                        t => t,
                    },
                    _ => i,
                };
                match (&expected_ret, kind) {
                    (Some(e), AsyncKind::None) if !e.is_dynamic() && !e.mentions_params() && self.ctx.is_subtype(&i, e) && !matches!(i, Ty::Never) => {
                        if matches!(e, Ty::Void) { Ty::Void } else { i }
                    }
                    _ => i,
                }
            }
            (None, None) => expected_ret.clone().unwrap_or(Ty::Dynamic),
        };
        let ret_ty = match kind {
            AsyncKind::None => inner_ret,
            AsyncKind::Async => match &inner_ret {
                Ty::Iface { class, .. } if Some(*class) == self.ctx.future_ && declared_ret.is_some() => inner_ret.clone(),
                _ => self.ctx.t_future(self.flatten_future(&inner_ret)),
            },
            AsyncKind::AsyncStar => match &inner_ret {
                Ty::Iface { class, .. } if Some(*class) == self.ctx.stream_ && declared_ret.is_some() => inner_ret.clone(),
                _ => self.ctx.t_stream(Ty::Dynamic),
            },
            AsyncKind::SyncStar => match &inner_ret {
                Ty::Iface { class, .. } if Some(*class) == self.ctx.iterable_ && declared_ret.is_some() => inner_ret.clone(),
                _ => self.ctx.t_iterable(Ty::Dynamic),
            },
        };
        let fn_ty = Ty::Fn { type_params: tps.clone(), ret: Box::new(ret_ty.clone()), pos, opt, named, nullable: false };
        let mut all_params: Vec<String> = scope_params.iter().map(|(_, n)| n.clone()).collect();
        if !param_js.is_empty() {
            all_params.push(param_js);
        }
        let head = format!("({}) => {{", all_params.join(", "));
        let text = self.wrap_async_head(kind, &head, &prologue, &body, &ret_ty);
        self.fn_type_params = saved_tps;
        let rti = self.rti(&fn_ty);
        if tps.is_empty() {
            (Js::prim(format!("dart.fn({text}, {rti})")), fn_ty)
        } else {
            let defaults: Vec<String> = tps.iter().map(|p| self.rti(&self.default_type_arg(&p.bound))).collect();
            (
                Js::prim(format!("dart.gFn({text}, {rti}, dart.constList(dart_rti._Universe.eval(dart_rti._theUniverse(), \"@\", true), [{}]))", defaults.join(", "))),
                fn_ty,
            )
        }
    }

    pub fn flatten_future(&self, t: &Ty) -> Ty {
        match t {
            Ty::FutureOr { arg, .. } => (**arg).clone(),
            Ty::Iface { class, args, .. } if Some(*class) == self.ctx.future_ && !args.is_empty() => args[0].clone(),
            _ => t.clone(),
        }
    }

    /// Tipos dos parâmetros já declarados no escopo atual (após `declare_params`).
    pub fn declared_param_tys(&self, params: &[ast::Parameter]) -> (Vec<Ty>, Vec<Ty>, Vec<(String, Ty, bool)>) {
        let mut pos = Vec::new();
        let mut opt = Vec::new();
        let mut named = Vec::new();
        for p in params {
            let Some(n) = p.name else { continue };
            let t = self.lookup_local(n.sym).map(|l| l.ty.clone()).unwrap_or(Ty::Dynamic);
            match p.kind {
                ast::ParameterKind::Required => pos.push(t),
                ast::ParameterKind::Optional => opt.push(t),
                ast::ParameterKind::Named => named.push((self.name(n.sym).to_string(), t, p.required)),
            }
        }
        named.sort_by(|a, b| a.0.cmp(&b.0));
        (pos, opt, named)
    }
}

fn mentions_any(t: &Ty, free: &[u32]) -> bool {
    let mut v = Vec::new();
    t.collect_params(&mut v);
    v.iter().any(|x| free.contains(x))
}
