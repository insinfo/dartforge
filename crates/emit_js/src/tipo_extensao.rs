//! Erased extension types (Dart 3.3): the ones that are not JS interop.
//!
//! At run time a value of an extension type **is** the value of the
//! representation (the type disappears from the rti recipes, see `Ctx::ty_of`).
//! Members are resolved statically, like those of an extension, so the emitter
//! cannot look them up on the (erased) type of the receiver: the shared
//! inference (`BodyTypes`) says, at each access, that the member belongs to an
//! extension type (`Resolved::Member { class }`), and it is what decides the route.
//!
//! Each extension type becomes a support object `L.E = class E { … }` with static
//! functions only (the same role DDC gives to the library's `E|member`
//! functions):
//!
//! * static member `m` → `static m(…)`, and the lazy static fields
//!   (`dart.defineLazy`), exactly as in a class — the emitter's static access
//!   paths work unchanged;
//! * instance member → `static ["#m"](T…, M…, $this, …)` (methods and
//!   operators), `["get#g"]` and `["set#s"]`, with the type arguments of the
//!   extension type (`T…`) and of the method (`M…`) before the receiver;
//! * constructor `E.n` (the primary one included) → `static n(T…, …)`, which
//!   returns the representation value; the generative one keeps it in `$this`.
//!
//! A call to the primary constructor is the argument itself (no call), as in DDC.

use crate::body::{static_member_name, AsyncKind, FnEmitter};
use crate::ctx::Ctx;
use crate::js::{self, Js, Writer, P_ASSIGN, P_PRIMARY};
use crate::module::{finish_body, flush_stmts, function_text, indent, ModState};
use crate::ty::Ty;
use dartforge_elements::model::{ClassId, ClassKind, FunctionElementId, FunctionKind, FunctionRef, VariableRef};
use dartforge_frontend::ast::{self, DeclKind, ExprId, MemberKind};
use dartforge_types::resolved::{MemberRef, Resolved};
use dartforge_types::table::Type;
use std::collections::HashMap;

/// `c` is an erased extension type (not JS interop).
pub fn e_tipo_extensao_apagado(ctx: &Ctx, c: ClassId) -> bool {
    ctx.program.class(c).kind == ClassKind::ExtensionType && !ctx.interop_ext_types.contains(&c) && !ctx.is_js_class(c)
}

/// Key, in the support object, of an instance member of an extension type.
fn instance_key(kind: FunctionKind, name: &str) -> String {
    match kind {
        FunctionKind::Getter => format!("get#{name}"),
        FunctionKind::Setter => format!("set#{name}"),
        _ => format!("#{name}"),
    }
}

/// JS name (in the support object) of constructor `name` (`""` is the unnamed one).
fn constructor_js_name(name: &str) -> String {
    if name.is_empty() {
        "new".to_string()
    } else {
        static_member_name(name)
    }
}

/// `c` has constructor `name` (`""` is the unnamed one), the primary one included.
pub fn tem_construtor(ctx: &Ctx, c: ClassId, name: &str) -> bool {
    if nome_do_primario(ctx, c).as_deref() == Some(name) {
        return true;
    }
    let key = if name.is_empty() { ctx.empty_sym } else { ctx.sym(name) };
    key.is_some_and(|k| ctx.program.class(c).constructors.contains_key(&k))
}

/// Name of the primary constructor (the representation) of `c`: `""` for the unnamed one.
pub(crate) fn nome_do_primario(ctx: &Ctx, c: ClassId) -> Option<String> {
    let decl = ctx.program.class(c).decl?;
    match &ctx.program.unit(decl.unit).ast.decl(decl.decl).kind {
        DeclKind::ExtensionType(ed) => Some(ed.constructor.map(|n| ctx.name(n.sym).to_string()).unwrap_or_default()),
        _ => None,
    }
}

impl<'m, 'a> FnEmitter<'m, 'a> {
    /// The instance member of an erased extension type that the shared
    /// inference resolved for `e` (read, call, write or operator): the type that
    /// declares it and the member. Statics are left out (the support object has
    /// them the way a class has them).
    pub fn membro_de_tipo_extensao(&self, e: ExprId) -> Option<(ClassId, MemberRef)> {
        let Resolved::Member { class, member, via_super: false } = self.resolucao_comum(e)? else { return None };
        if !e_tipo_extensao_apagado(self.ctx, *class) {
            return None;
        }
        let is_static = match member {
            MemberRef::Function(f) => self.ctx.program.function(*f).static_,
            MemberRef::Variable(v) => self.ctx.program.variable(*v).static_,
        };
        (!is_static).then_some((*class, *member))
    }

    /// Type arguments of `owner` as seen on the receiver: from the static type
    /// (shared inference) of `recv`, or the parameters themselves when the
    /// receiver is the implicit `this` of a member. Returns the recipes and the
    /// substitution of `owner`'s parameters.
    fn receiver_type_args(&self, recv: Option<ExprId>, owner: ClassId) -> (Vec<String>, HashMap<u32, Ty>) {
        let params = self.ctx.class_params[owner.0 as usize].clone();
        if params.is_empty() {
            return (Vec::new(), HashMap::new());
        }
        let args: Vec<Ty> = match recv {
            None => params.iter().map(|p| Ty::Param { id: p.id, name: p.name.clone(), nullable: false }).collect(),
            Some(r) => {
                let t = self.ctx.bodies.units.get(self.unit.0 as usize).and_then(|u| u.get_type(r));
                match t.map(|t| self.ctx.table.get(t)) {
                    Some(Type::ExtensionType { decl, args, .. }) if *decl == owner && args.len() == params.len() => {
                        args.iter().map(|a| self.ctx.ty_of(*a)).collect()
                    }
                    _ => params.iter().map(|_| Ty::Dynamic).collect(),
                }
            }
        };
        let subst: HashMap<u32, Ty> = params.iter().zip(args.iter()).map(|(p, a)| (p.id, a.clone())).collect();
        (args.iter().map(|a| self.rti(a)).collect(), subst)
    }

    /// (Erased) type of the representation of `c` with the given substitution.
    fn representation(&self, c: ClassId, subst: &HashMap<u32, Ty>) -> Ty {
        let params: Vec<Ty> = self.ctx.class_params[c.0 as usize].iter().map(|p| subst.get(&p.id).cloned().unwrap_or(Ty::Dynamic)).collect();
        self.ctx.erase_ext(c, &params).unwrap_or(Ty::Dynamic)
    }

    /// `recv.member` (getter, representation or method tear-off).
    pub fn tipo_ext_ler(&mut self, recv: &Js, recv_e: Option<ExprId>, owner: ClassId, member: MemberRef) -> (Js, Ty) {
        let (targs, subst) = self.receiver_type_args(recv_e, owner);
        let fid = match member {
            MemberRef::Variable(_) => return (recv.clone(), self.representation(owner, &subst)),
            MemberRef::Function(f) => f,
        };
        let mut f = self.ctx.program.function(fid);
        let mut fid = fid;
        // In a compound write (`x.s += 1`) the shared resolution of the target is
        // the setter; the read goes through the getter with the same name.
        if f.kind == FunctionKind::Setter {
            let name = self.name(f.name).trim_end_matches("_=").to_string();
            match self.ext_type_getter(owner, &name) {
                Some(g) => {
                    fid = g;
                    f = self.ctx.program.function(g);
                }
                None => return (Js::prim("null"), Ty::Dynamic),
            }
        }
        let name = self.name(f.name).to_string();
        let support = self.class_ref(owner);
        match f.kind {
            FunctionKind::Getter => {
                let ret = self.ctx.ty_of(self.ctx.outline.functions[fid.0 as usize].return_type).subst_prop(&subst);
                let mut args = targs;
                args.push(recv.code.clone());
                (Js::prim(format!("{support}[{}]({})", js::string_literal(&instance_key(f.kind, &name)), args.join(", "))), ret)
            }
            _ => self.ext_type_tearoff(recv, &support, &instance_key(f.kind, &name), targs, fid, &subst),
        }
    }

    /// Tear-off of an extension type instance method: a closure that fixes the
    /// receiver (evaluated once) and forwards the arguments to the support function.
    fn ext_type_tearoff(&mut self, recv: &Js, support: &str, key: &str, targs: Vec<String>, fid: FunctionElementId, subst: &HashMap<u32, Ty>) -> (Js, Ty) {
        let ty = self.ctx.fn_ty(fid).subst_prop(subst);
        let Ty::Fn { type_params, pos, opt, named, .. } = &ty else { return (Js::prim("null"), Ty::Dynamic) };
        let mut ps: Vec<String> = type_params.iter().map(|p| self.nome_js_parametro_de_tipo(&p.name)).collect();
        let tps = ps.clone();
        ps.extend((0..pos.len() + opt.len()).map(|i| format!("a{i}")));
        if !named.is_empty() {
            ps.push("opts".into());
        }
        let r = self.temp();
        let mut args = targs;
        args.extend(tps.iter().cloned());
        args.push(r.clone());
        args.extend(ps[tps.len()..].iter().cloned());
        let call = format!("{support}[{}]({})", js::string_literal(key), args.join(", "));
        let rti = self.rti(&ty);
        let value = if type_params.is_empty() {
            format!("dart.fn(({}) => {call}, {rti})", ps.join(", "))
        } else {
            let defaults: Vec<String> = type_params.iter().map(|p| self.rti(&self.default_type_arg(&p.bound))).collect();
            format!(
                "dart.gFn(({}) => {call}, {rti}, dart.constList(dart_rti._Universe.eval(dart_rti._theUniverse(), \"@\", true), [{}]))",
                ps.join(", "),
                defaults.join(", ")
            )
        };
        (Js::new(format!("({r} = {}, {value})", recv.at(P_ASSIGN)), P_PRIMARY), ty)
    }

    /// `recv.member(args)`: method or operator through the support function; a
    /// getter or a representation of function type calls the value read.
    pub fn tipo_ext_chamar(&mut self, recv: &Js, recv_e: Option<ExprId>, owner: ClassId, member: MemberRef, arguments: &ast::Arguments, expected: Option<&Ty>) -> (Js, Ty) {
        let fid = match member {
            MemberRef::Function(f) if !matches!(self.ctx.program.function(f).kind, FunctionKind::Getter) => f,
            _ => {
                let (g, gty) = self.tipo_ext_ler(recv, recv_e, owner, member);
                return self.emit_fn_value_call(&g, &gty, arguments, expected);
            }
        };
        let (targs, subst) = self.receiver_type_args(recv_e, owner);
        let f = self.ctx.program.function(fid);
        let key = instance_key(f.kind, self.name(f.name));
        let support = self.class_ref(owner);
        let fty = self.ctx.fn_ty(fid).subst_prop(&subst);
        let (args, ret, mtargs) = self.emit_args_for(&fty, arguments, expected);
        let mut all = targs;
        all.extend(mtargs);
        all.push(recv.code.clone());
        all.extend(args);
        (Js::prim(format!("{support}[{}]({})", js::string_literal(&key), all.join(", "))), ret)
    }

    /// Instance operator (`a + b`, `-a`, `a[i]`) with already emitted operands.
    pub fn tipo_ext_operador(&mut self, recv: &Js, recv_e: Option<ExprId>, owner: ClassId, fid: FunctionElementId, operands: &[Js]) -> (Js, Ty) {
        let (targs, subst) = self.receiver_type_args(recv_e, owner);
        let f = self.ctx.program.function(fid);
        let key = instance_key(f.kind, self.name(f.name));
        let support = self.class_ref(owner);
        let ret = match self.ctx.fn_ty(fid).subst_prop(&subst) {
            Ty::Fn { ret, .. } => *ret,
            _ => Ty::Dynamic,
        };
        let mut all = targs;
        all.push(recv.code.clone());
        all.extend(operands.iter().map(|o| o.code.clone()));
        (Js::prim(format!("{support}[{}]({})", js::string_literal(&key), all.join(", "))), ret)
    }

    /// Type of parameter `i` of an extension type member (context of an
    /// operator's operand), with the receiver's type arguments.
    pub fn tipo_ext_parametro(&self, recv_e: Option<ExprId>, owner: ClassId, fid: FunctionElementId, i: usize) -> Option<Ty> {
        let (_, subst) = self.receiver_type_args(recv_e, owner);
        match self.ctx.fn_ty(fid).subst_prop(&subst) {
            Ty::Fn { pos, opt, .. } => pos.iter().chain(opt.iter()).nth(i).cloned(),
            _ => None,
        }
    }

    /// `recv.name = v` through an extension type setter. The value of the
    /// expression is `v` (the setter's support function does not return it), kept
    /// in a temporary after the receiver is evaluated.
    pub fn tipo_ext_escrever(&mut self, recv: &Js, recv_e: Option<ExprId>, owner: ClassId, fid: FunctionElementId, v: &Js, vty: &Ty) -> (Js, Ty) {
        let (targs, _) = self.receiver_type_args(recv_e, owner);
        let f = self.ctx.program.function(fid);
        let key = instance_key(FunctionKind::Setter, self.name(f.name).trim_end_matches("_="));
        let support = self.class_ref(owner);
        let t = self.temp();
        let mut all = targs;
        all.push(recv.code.clone());
        all.push(format!("{t} = {}", v.at(P_ASSIGN)));
        (Js::new(format!("({support}[{}]({}), {t})", js::string_literal(&key), all.join(", ")), P_PRIMARY), vty.clone())
    }

    /// The instance getter `name` of `owner`.
    pub fn ext_type_getter(&self, owner: ClassId, name: &str) -> Option<FunctionElementId> {
        let f = *self.ctx.program.class(owner).instance_members.get(&self.ctx.sym(name)?)?;
        (self.ctx.program.function(f).kind == FunctionKind::Getter).then_some(f)
    }

    /// The instance operator `op` (`[]`, `[]=`) of `owner`.
    pub fn operador_de_tipo_extensao(&self, owner: ClassId, op: &str) -> Option<FunctionElementId> {
        self.ctx.program.class(owner).instance_members.get(&self.ctx.sym(op)?).copied()
    }

    /// The instance setter `name` of `owner` (or of an extension type it
    /// implements), for a write whose shared resolution points to the getter with
    /// the same name.
    pub fn setter_de_tipo_extensao(&self, owner: ClassId, name: &str) -> Option<FunctionElementId> {
        let sym = self.ctx.sym(&format!("{name}_="))?;
        let f = *self.ctx.program.class(owner).instance_members.get(&sym)?;
        (self.ctx.program.function(f).kind == FunctionKind::Setter).then_some(f)
    }

    /// Construction `E.name(args)` (or `E(args)`) of an erased extension type.
    /// The primary constructor is the value of the argument; the others are the
    /// support function, with the type arguments (explicit or inferred from the
    /// arguments) in front.
    pub fn tipo_ext_construir(&mut self, c: ClassId, class_args: Vec<Ty>, explicit_args: bool, ctor_name: &str, arguments: &ast::Arguments, expected: Option<&Ty>) -> (Js, Ty) {
        let params = self.ctx.class_params[c.0 as usize].clone();
        let explicit: HashMap<u32, Ty> = if explicit_args { params.iter().zip(class_args.iter()).map(|(p, a)| (p.id, a.clone())).collect() } else { HashMap::new() };
        if nome_do_primario(self.ctx, c).as_deref() == Some(ctor_name) {
            // The argument's context is the representation only when it does not
            // depend on type arguments still to be inferred.
            let rep = if explicit_args || params.is_empty() { Some(self.representation(c, &explicit)) } else { None };
            let Some(a) = arguments.args.first() else { return (Js::prim("null"), Ty::Dynamic) };
            let (js, ty) = self.emit_expr(a.value, rep.as_ref());
            let ty = rep.unwrap_or(ty);
            return (js, ty);
        }
        let key = if ctor_name.is_empty() { self.ctx.empty_sym } else { self.ctx.sym(ctor_name) };
        let Some(fid) = key.and_then(|k| self.ctx.program.class(c).constructors.get(&k).copied()) else {
            return (Js::prim("null"), Ty::Dynamic);
        };
        let ctor_fn = match self.ctx.fn_ty(fid) {
            Ty::Fn { ret, pos, opt, named, nullable, .. } => Ty::Fn { type_params: vec![], ret, pos, opt, named, nullable },
            t => t,
        };
        let free: Vec<u32> = params.iter().map(|p| p.id).collect();
        let mut subst = explicit;
        let arg_js = if explicit_args || params.is_empty() {
            self.emit_args_infer(&ctor_fn.subst(&subst), arguments, &[], &mut HashMap::new(), expected).0
        } else {
            let (a, _) = self.emit_args_infer(&ctor_fn, arguments, &free, &mut subst, None);
            self.restringir_pelos_limites(&params, &free, &mut subst);
            a
        };
        let mut all: Vec<String> = Vec::new();
        for p in &params {
            let t = subst.get(&p.id).cloned().unwrap_or_else(|| self.default_type_arg(&p.bound));
            subst.insert(p.id, t.clone());
            all.push(self.rti(&t));
        }
        all.extend(arg_js);
        let support = self.class_ref(c);
        let rep = self.representation(c, &subst);
        (Js::prim(format!("{support}.{}({})", constructor_js_name(ctor_name), all.join(", "))), rep)
    }

    /// Tear-off `E.name` / `E.new` of an erased extension type constructor.
    pub fn tipo_ext_tearoff_construtor(&mut self, c: ClassId, targs: Vec<Ty>, name: &str) -> Option<(Js, Ty)> {
        let ctor_name = if name == "new" { "" } else { name };
        let params = self.ctx.class_params[c.0 as usize].clone();
        let subst: HashMap<u32, Ty> = params.iter().zip(targs.iter()).map(|(p, a)| (p.id, a.clone())).collect();
        let rep = self.representation(c, &subst);
        let ty = if nome_do_primario(self.ctx, c).as_deref() == Some(ctor_name) {
            Ty::Fn { type_params: vec![], ret: Box::new(rep.clone()), pos: vec![rep], opt: vec![], named: vec![], nullable: false }
        } else {
            let key = if ctor_name.is_empty() { self.ctx.empty_sym } else { self.ctx.sym(ctor_name) };
            let fid = key.and_then(|k| self.ctx.program.class(c).constructors.get(&k).copied())?;
            match self.ctx.fn_ty(fid).subst_prop(&subst) {
                Ty::Fn { pos, opt, named, nullable, .. } => Ty::Fn { type_params: vec![], ret: Box::new(rep), pos, opt, named, nullable },
                t => t,
            }
        };
        let Ty::Fn { pos, opt, named, .. } = &ty else { return None };
        let mut ps: Vec<String> = (0..pos.len() + opt.len()).map(|i| format!("a{i}")).collect();
        if !named.is_empty() {
            ps.push("opts".into());
        }
        let mut args: Vec<String> = params.iter().map(|p| self.rti(subst.get(&p.id).unwrap_or(&Ty::Dynamic))).collect();
        args.extend(ps.iter().cloned());
        let support = self.class_ref(c);
        let rti = self.rti(&ty);
        Some((Js::prim(format!("dart.fn(({}) => {support}.{}({}), {rti})", ps.join(", "), constructor_js_name(ctor_name), args.join(", "))), ty))
    }
}

/// Emits the support object of an erased extension type (see the top of the
/// module).
pub(crate) fn emit_extension_type(ctx: &Ctx, m: &ModState, c: ClassId, w: &mut Writer) {
    let class = ctx.program.class(c);
    let Some(decl) = class.decl else { return };
    let unit = decl.unit;
    let DeclKind::ExtensionType(ed) = &ctx.program.unit(unit).ast.decl(decl.decl).kind else { return };
    let lvar = ctx.libs[class.library.0 as usize].js_var.clone();
    let cname = ctx.class_name(c).to_string();
    let cref = format!("{lvar}.{cname}");
    let ast = &ctx.program.unit(unit).ast;
    let mut cw = Writer { indent: 1, ..Default::default() };
    let live = |f: FunctionElementId| ctx.estado_fn(f) != crate::filtro::Estado::Morta;
    let pruned = |f: FunctionElementId| ctx.estado_fn(f) == crate::filtro::Estado::Stub;
    let mut static_fields = Vec::new();
    // The primary constructor: identity.
    let tps: Vec<String> = ctx.class_params[c.0 as usize].iter().map(|p| js::ident(&p.name)).collect();
    {
        let primary = ed.constructor.map(|n| ctx.name(n.sym).to_string()).unwrap_or_default();
        let mut ps = tps.clone();
        ps.push("v".into());
        cw.line(&format!("static {}({}) {{ return v; }}", js::prop_key(&constructor_js_name(&primary)), ps.join(", ")));
    }
    for &mid in &ed.members {
        let mem = ast.member(mid);
        match &mem.kind {
            MemberKind::Method(afid) => {
                let af = ast.function(*afid);
                if af.external {
                    continue;
                }
                let Some(fname) = af.name else { continue };
                let mut name = ctx.name(fname.sym).to_string();
                if af.kind == ast::FunctionKind::Operator && name == "-" && af.parameters.as_ref().is_some_and(|p| p.is_empty()) {
                    name = "unary-".to_string();
                }
                let key = if af.kind == ast::FunctionKind::Setter { format!("{name}_=") } else { name.clone() };
                let map = if af.static_ { &class.static_members } else { &class.instance_members };
                let Some(feid) = ctx.sym(&key).and_then(|s| map.get(&s).copied()) else { continue };
                if !live(feid) {
                    continue;
                }
                let f = ctx.program.function(feid);
                if f.abstract_ {
                    continue;
                }
                let head = if af.static_ {
                    let sk = js::prop_key(&static_member_name(&name));
                    match af.kind {
                        ast::FunctionKind::Getter => format!("static get {sk}"),
                        ast::FunctionKind::Setter => format!("static set {sk}"),
                        _ => format!("static {sk}"),
                    }
                } else {
                    format!("static [{}]", js::string_literal(&instance_key(f.kind, &name)))
                };
                if pruned(feid) {
                    let r = js::string_literal(&ctx.rotulo_podado(class.library, Some(c), &name));
                    let text = match af.kind {
                        ast::FunctionKind::Getter if af.static_ => format!("{head}() {{ return dart_podado({r}); }}"),
                        ast::FunctionKind::Setter if af.static_ => format!("{head}(v) {{ dart_podado({r}); }}"),
                        _ => format!("{head}(...a) {{ return dart_podado({r}); }}"),
                    };
                    cw.line(&text);
                    continue;
                }
                let text = if af.static_ {
                    function_text(ctx, m, feid, Some(&head), Some(c), true, None).0
                } else {
                    member_text(ctx, m, c, feid, &head)
                };
                for line in text.lines() {
                    cw.line(line);
                }
            }
            MemberKind::Constructor(ctor) => {
                if ctor.external {
                    continue;
                }
                let sym = ctor.name.map(|n| n.sym).or(ctx.empty_sym);
                let Some(fid) = sym.and_then(|s| class.constructors.get(&s).copied()) else { continue };
                if !live(fid) {
                    continue;
                }
                let text = constructor_text(ctx, m, c, unit, ctor, fid);
                for line in text.lines() {
                    cw.line(line);
                }
            }
            MemberKind::Field(_) => {}
        }
    }
    for &vid in &class.fields {
        let v = ctx.program.variable(vid);
        if v.static_ && ctx.estado_var(vid) != crate::filtro::Estado::Morta {
            static_fields.push(vid);
        }
    }
    crate::linha!(w, "{cref} = class {cname} {{");
    w.push_raw(&cw.out);
    w.line("};");
    // Static fields: lazy, like those of a class.
    let mut lazy: Vec<String> = Vec::new();
    for vid in static_fields {
        let v = ctx.program.variable(vid);
        let name = static_member_name(ctx.name(v.name));
        if ctx.estado_var(vid) == crate::filtro::Estado::Stub {
            let r = js::string_literal(&ctx.rotulo_podado(class.library, Some(c), &name));
            lazy.push(format!("get {}() {{ return dart_podado({r}); }}", js::prop_key(&name)));
            continue;
        }
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
            None if v.late => {
                m.use_sdk("_internal");
                format!("dart.throw(new _internal.LateError.fieldNI({}));", js::string_literal(&name))
            }
            None => "return null;".to_string(),
        };
        let mut entry = format!("get {}() {{\n{}\n}}", js::prop_key(&name), indent(&init));
        if !v.final_ && !v.const_ || v.late && var.initializer.is_none() {
            entry.push_str(&format!(",\nset {}(value) {{}}", js::prop_key(&name)));
        }
        lazy.push(entry);
    }
    if !lazy.is_empty() {
        crate::linha!(w, "dart.defineLazy({cref}, {{");
        w.push_raw(&indent(&lazy.join(",\n")));
        w.push_raw("\n});\n");
    }
}

/// Emitter for a member (instance member or constructor) of `c`: the type
/// parameters of `c` in scope as function parameters, and the statics of `c`
/// reachable without qualification.
fn member_emitter<'m, 'a>(ctx: &'m Ctx<'a>, m: &'m ModState, c: ClassId, unit: dartforge_elements::model::UnitId) -> (FnEmitter<'m, 'a>, Vec<String>, Ty) {
    let mut e = FnEmitter::new(ctx, m, unit, None, true);
    e.tipo_extensao = Some(c);
    let mut tps = Vec::new();
    let mut params = Vec::new();
    for p in ctx.class_params[c.0 as usize].iter() {
        let jsn = e.nome_js_parametro_de_tipo(&p.name);
        e.fn_type_params.push((p.id, jsn.clone()));
        tps.push(jsn);
        params.push(Ty::Param { id: p.id, name: p.name.clone(), nullable: false });
    }
    let rep = ctx.erase_ext(c, &params).unwrap_or(Ty::Dynamic);
    (e, tps, rep)
}

fn async_kind_of(m: ast::AsyncModifier) -> AsyncKind {
    match m {
        ast::AsyncModifier::None => AsyncKind::None,
        ast::AsyncModifier::Async => AsyncKind::Async,
        ast::AsyncModifier::AsyncStar => AsyncKind::AsyncStar,
        ast::AsyncModifier::SyncStar => AsyncKind::SyncStar,
    }
}

/// `static ["#m"](T…, M…, $this, params) { body }` of an instance member.
fn member_text(ctx: &Ctx, m: &ModState, c: ClassId, fid: FunctionElementId, head: &str) -> String {
    let f = ctx.program.function(fid);
    let FunctionRef::Function { unit, function } = f.node else { return String::new() };
    let af = ctx.program.unit(unit).ast.function(function);
    let (mut e, mut ps, rep) = member_emitter(ctx, m, c, unit);
    let data = &ctx.outline.functions[fid.0 as usize];
    for &pid in data.type_params.iter() {
        let p = ctx.ty_param_of(pid);
        let jsn = e.nome_js_parametro_de_tipo(&p.name);
        e.fn_type_params.insert(0, (p.id, jsn.clone()));
        ps.push(jsn);
    }
    let kind = async_kind_of(af.modifier);
    let ret_ty = ctx.ty_of(data.return_type);
    e.async_kind = kind;
    e.ret_ty = ret_ty.clone();
    ps.push("$this".into());
    e.extension_this = Some(rep.clone());
    if let Some(sym) = ctx.sym("this") {
        e.declare_js(sym, "$this".into(), rep);
    }
    let sig = ctx.fn_ty(fid);
    let params: &[ast::Parameter] = af.parameters.as_deref().unwrap_or(&[]);
    let (pjs, prologue) = e.declare_params(params, Some(&sig));
    if !pjs.is_empty() {
        ps.push(pjs);
    }
    e.emit_body(&af.body);
    let body = finish_body(&mut e);
    let head = format!("{head}({}) {{", ps.join(", "));
    e.wrap_async_head(kind, &head, &prologue, &body, &ret_ty)
}

/// `static n(T…, params) { … }` of a constructor. The generative one computes the
/// representation into `$this` (initializing formal, initializer list or
/// `this(…)` redirection) and runs the body, which sees `this`; the factory is a
/// plain function that returns the value.
pub(crate) fn constructor_text(ctx: &Ctx, m: &ModState, c: ClassId, unit: dartforge_elements::model::UnitId, ctor: &ast::Constructor, fid: FunctionElementId) -> String {
    let (mut e, mut ps, rep) = member_emitter(ctx, m, c, unit);
    let name = constructor_js_name(ctor.name.map(|n| ctx.name(n.sym)).unwrap_or(""));
    let sig = ctx.fn_ty(fid);
    let (pjs, prologue, names) = e.declare_params_nomes(&ctor.parameters, Some(&sig));
    if !pjs.is_empty() {
        ps.push(pjs);
    }
    let head = format!("static {}({}) {{", js::prop_key(&name), ps.join(", "));
    if let Some(redir) = &ctor.redirect {
        // `factory E.f(…) = E2.g;`: forwards the parameters to the target.
        let target = redirect_target(&e, redir);
        let body = match target {
            Some((tc, tname)) => {
                let mut args: Vec<String> = ctx.class_params[tc.0 as usize].iter().map(|_| e.rti(&Ty::Dynamic)).collect();
                if tc == c {
                    args = e.fn_type_params.iter().map(|(_, j)| j.clone()).collect();
                }
                let mut named = Vec::new();
                for (k, dart, jsn) in &names {
                    match k {
                        ast::ParameterKind::Named => named.push(format!("{}: {jsn}", js::prop_key(&ctx.nomeado(dart)))),
                        _ => args.push(jsn.clone()),
                    }
                }
                if !named.is_empty() {
                    args.push(format!("{{{}}}", named.join(", ")));
                }
                let tref = e.class_ref(tc);
                if e_tipo_extensao_apagado(ctx, tc) {
                    format!("return {tref}.{}({});", constructor_js_name(&tname), args.join(", "))
                } else {
                    String::new()
                }
            }
            None => String::new(),
        };
        return format!("{head}\n{}{}\n}}", indent(&prologue), indent(&body));
    }
    if ctor.factory {
        e.ret_ty = rep.clone();
        e.emit_body(&ctor.body);
        let body = finish_body(&mut e);
        return e.wrap_async_head(AsyncKind::None, &head, &prologue, &body, &rep);
    }
    // Generative. In an interop type (`package:web`), `this` is the extension
    // type itself: its `external` members, read and written through the
    // implicit `this` in the body, are property accesses on `$this`.
    let this_ty = if ctx.is_js_class(c) {
        Ty::Iface { class: c, args: Vec::new(), nullable: false }
    } else {
        rep.clone()
    };
    e.extension_this = Some(this_ty.clone());
    if let Some(sym) = ctx.sym("this") {
        e.declare_js(sym, "$this".into(), this_ty);
    }
    let mut body = Writer::default();
    body.line("let $this;");
    let rep_name = ctx.program.class(c).representation.map(|v| ctx.program.variable(v).name);
    for p in ctor.parameters.iter() {
        if p.this_ && p.name.map(|n| n.sym) == rep_name {
            if let Some(l) = p.name.and_then(|n| e.lookup_local(n.sym)) {
                crate::linha!(body, "$this = {};", l.js);
            }
        }
    }
    for init in ctor.initializers.iter() {
        match init {
            ast::Initializer::Field { value, .. } => {
                let (js, _) = e.emit_expr(*value, Some(&rep));
                flush_stmts(&mut e, &mut body);
                crate::linha!(body, "$this = {};", js.code);
            }
            ast::Initializer::Redirect { constructor, arguments, .. } => {
                let n = constructor.map(|n| ctx.name(n.sym).to_string()).unwrap_or_default();
                let args: Vec<Ty> = ctx.class_params[c.0 as usize].iter().map(|p| Ty::Param { id: p.id, name: p.name.clone(), nullable: false }).collect();
                let (js, _) = e.tipo_ext_construir(c, args, true, &n, arguments, None);
                flush_stmts(&mut e, &mut body);
                crate::linha!(body, "$this = {};", js.code);
            }
            ast::Initializer::Assert { .. } if e.ctx.filtro.is_some_and(|f| !f.manter_asserts()) => {}
            ast::Initializer::Assert { condition, message, .. } => {
                let (cjs, _) = e.emit_cond(*condition);
                let msg = message.map(|mm| e.emit_expr(mm, None).0.code).unwrap_or("null".into());
                flush_stmts(&mut e, &mut body);
                crate::linha!(body, "if (!({cjs})) dart.assertFailed({msg}, null, 0, 0, \"\");");
            }
            ast::Initializer::Super { .. } => {}
        }
    }
    // The body may contain `return;`: it runs in an arrow function, which sees `$this`.
    let has_body = match &ctor.body {
        ast::FunctionBody::Block(b) => !is_empty_block(ctx, unit, *b),
        ast::FunctionBody::Expression(_) => true,
        _ => false,
    };
    if has_body {
        e.emit_body(&ctor.body);
        let text = std::mem::take(&mut e.w.out);
        crate::linha!(body, "(() => {{");
        body.push_raw(&indent(&text));
        body.push_raw("\n");
        body.line("})();");
    }
    body.line("return $this;");
    let temps = if e.temps.is_empty() { String::new() } else { format!("let {};\n", e.temps.join(", ")) };
    format!("{head}\n{}{}{}}}", indent(&prologue), indent(&temps), indent(&body.out))
}

/// The block body has no statements.
fn is_empty_block(ctx: &Ctx, unit: dartforge_elements::model::UnitId, b: ast::StmtId) -> bool {
    matches!(&ctx.program.unit(unit).ast.stmt(b).kind, ast::StmtKind::Block(s) if s.is_empty())
}

/// Class and name of the target constructor of `= Target.name` (the written type
/// is named; `Target.name` may arrive as a qualified name).
fn redirect_target(e: &FnEmitter, redir: &ast::RedirectTarget) -> Option<(ClassId, String)> {
    let ast::TypeKind::Named { name: parts, .. } = &e.ast().ty(redir.ty).kind else { return None };
    let class = |sym| match e.ctx.program.lookup(e.lib, sym).and_then(|b| b.getter) {
        Some(dartforge_elements::model::Element::Class(c)) => Some(c),
        _ => None,
    };
    let name = redir.constructor.map(|n| e.name(n.sym).to_string());
    match (parts.len(), name) {
        (1, n) => Some((class(parts[0].sym)?, n.unwrap_or_default())),
        (2, None) => class(parts[0].sym).map(|c| (c, e.name(parts[1].sym).to_string())),
        _ => None,
    }
}
