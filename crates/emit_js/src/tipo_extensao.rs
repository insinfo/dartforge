//! Tipos de extensão (Dart 3.3) apagados: os que não são de interop JS.
//!
//! Em tempo de execução um valor de tipo de extensão **é** o valor da
//! representação (o tipo some das receitas rti, ver `Ctx::ty_of`). Os membros
//! são resolvidos estaticamente, como os de uma extensão, e por isso o
//! emissor não pode procurá-los no tipo (apagado) do receptor: a inferência
//! comum (`BodyTypes`) diz, em cada acesso, que o membro é de um tipo de
//! extensão (`Resolved::Member { class }`), e é ela que decide a rota.
//!
//! Cada tipo de extensão vira um objeto de apoio `L.E = class E { … }` só com
//! funções estáticas (o mesmo papel que o DDC dá às funções `E|membro` da
//! biblioteca):
//!
//! * membro estático `m` → `static m(…)`, e os campos estáticos preguiçosos
//!   (`dart.defineLazy`), exatamente como numa classe — os caminhos de acesso
//!   estático do emissor servem sem mudança;
//! * membro de instância → `static ["#m"](T…, M…, $this, …)` (métodos e
//!   operadores), `["get#g"]` e `["set#s"]`, com os argumentos de tipo do
//!   tipo de extensão (`T…`) e do método (`M…`) antes do receptor;
//! * construtor `E.n` (o primário inclusive) → `static n(T…, …)`, que
//!   devolve o valor da representação; o generativo guarda-o em `$this`.
//!
//! A chamada do construtor primário é o próprio argumento (sem chamada),
//! como no DDC.

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

/// `c` é um tipo de extensão apagado (não é de interop JS).
pub fn e_tipo_extensao_apagado(ctx: &Ctx, c: ClassId) -> bool {
    ctx.program.class(c).kind == ClassKind::ExtensionType && !ctx.interop_ext_types.contains(&c) && !ctx.is_js_class(c)
}

/// Chave, no objeto de apoio, de um membro de instância de tipo de extensão.
fn chave_de_instancia(kind: FunctionKind, nome: &str) -> String {
    match kind {
        FunctionKind::Getter => format!("get#{nome}"),
        FunctionKind::Setter => format!("set#{nome}"),
        _ => format!("#{nome}"),
    }
}

/// Nome JS (no objeto de apoio) do construtor `nome` (`""` é o sem nome).
fn nome_de_construtor(nome: &str) -> String {
    if nome.is_empty() {
        "new".to_string()
    } else {
        static_member_name(nome)
    }
}

/// `c` tem o construtor `nome` (`""` é o sem nome), o primário inclusive.
pub fn tem_construtor(ctx: &Ctx, c: ClassId, nome: &str) -> bool {
    if nome_do_primario(ctx, c).as_deref() == Some(nome) {
        return true;
    }
    let key = if nome.is_empty() { ctx.empty_sym } else { ctx.sym(nome) };
    key.is_some_and(|k| ctx.program.class(c).constructors.contains_key(&k))
}

/// Nome do construtor primário (a representação) de `c`: `""` para o sem nome.
fn nome_do_primario(ctx: &Ctx, c: ClassId) -> Option<String> {
    let decl = ctx.program.class(c).decl?;
    match &ctx.program.unit(decl.unit).ast.decl(decl.decl).kind {
        DeclKind::ExtensionType(ed) => Some(ed.constructor.map(|n| ctx.name(n.sym).to_string()).unwrap_or_default()),
        _ => None,
    }
}

impl<'m, 'a> FnEmitter<'m, 'a> {
    /// O membro de instância de tipo de extensão apagado que a inferência
    /// comum resolveu para `e` (leitura, chamada, escrita ou operador): o
    /// tipo que o declara e o membro. Estáticos ficam de fora (o objeto de
    /// apoio os tem como uma classe os tem).
    pub fn membro_de_tipo_extensao(&self, e: ExprId) -> Option<(ClassId, MemberRef)> {
        let Resolved::Member { class, member, via_super: false } = self.resolucao_comum(e)? else { return None };
        if !e_tipo_extensao_apagado(self.ctx, *class) {
            return None;
        }
        let estatico = match member {
            MemberRef::Function(f) => self.ctx.program.function(*f).static_,
            MemberRef::Variable(v) => self.ctx.program.variable(*v).static_,
        };
        (!estatico).then_some((*class, *member))
    }

    /// Argumentos de tipo de `dono` vistos no receptor: do tipo estático
    /// (inferência comum) de `recv`, ou os próprios parâmetros quando o
    /// receptor é o `this` implícito de um membro. Devolve as receitas e a
    /// substituição dos parâmetros de `dono`.
    fn args_do_receptor(&self, recv: Option<ExprId>, dono: ClassId) -> (Vec<String>, HashMap<u32, Ty>) {
        let params = self.ctx.class_params[dono.0 as usize].clone();
        if params.is_empty() {
            return (Vec::new(), HashMap::new());
        }
        let args: Vec<Ty> = match recv {
            None => params.iter().map(|p| Ty::Param { id: p.id, name: p.name.clone(), nullable: false }).collect(),
            Some(r) => {
                let t = self.ctx.bodies.units.get(self.unit.0 as usize).and_then(|u| u.get_type(r));
                match t.map(|t| self.ctx.table.get(t)) {
                    Some(Type::ExtensionType { decl, args, .. }) if *decl == dono && args.len() == params.len() => {
                        args.iter().map(|a| self.ctx.ty_of(*a)).collect()
                    }
                    _ => params.iter().map(|_| Ty::Dynamic).collect(),
                }
            }
        };
        let subst: HashMap<u32, Ty> = params.iter().zip(args.iter()).map(|(p, a)| (p.id, a.clone())).collect();
        (args.iter().map(|a| self.rti(a)).collect(), subst)
    }

    /// Tipo (apagado) da representação de `c` com a substituição dada.
    fn representacao(&self, c: ClassId, subst: &HashMap<u32, Ty>) -> Ty {
        let params: Vec<Ty> = self.ctx.class_params[c.0 as usize].iter().map(|p| subst.get(&p.id).cloned().unwrap_or(Ty::Dynamic)).collect();
        self.ctx.erase_ext(c, &params).unwrap_or(Ty::Dynamic)
    }

    /// `recv.membro` (getter, representação ou tearoff de método).
    pub fn tipo_ext_ler(&mut self, recv: &Js, recv_e: Option<ExprId>, dono: ClassId, membro: MemberRef) -> (Js, Ty) {
        let (targs, subst) = self.args_do_receptor(recv_e, dono);
        let fid = match membro {
            MemberRef::Variable(_) => return (recv.clone(), self.representacao(dono, &subst)),
            MemberRef::Function(f) => f,
        };
        let mut f = self.ctx.program.function(fid);
        let mut fid = fid;
        // Numa escrita composta (`x.s += 1`) a resolução comum do alvo é o
        // setter; a leitura é pelo getter de mesmo nome.
        if f.kind == FunctionKind::Setter {
            let nome = self.name(f.name).trim_end_matches("_=").to_string();
            match self.getter_de_tipo_extensao(dono, &nome) {
                Some(g) => {
                    fid = g;
                    f = self.ctx.program.function(g);
                }
                None => return (Js::prim("null"), Ty::Dynamic),
            }
        }
        let nome = self.name(f.name).to_string();
        let apoio = self.class_ref(dono);
        match f.kind {
            FunctionKind::Getter => {
                let ret = self.ctx.ty_of(self.ctx.outline.functions[fid.0 as usize].return_type).subst_prop(&subst);
                let mut args = targs;
                args.push(recv.code.clone());
                (Js::prim(format!("{apoio}[{}]({})", js::string_literal(&chave_de_instancia(f.kind, &nome)), args.join(", "))), ret)
            }
            _ => self.tearoff_tipo_ext(recv, &apoio, &chave_de_instancia(f.kind, &nome), targs, fid, &subst),
        }
    }

    /// Tearoff de método de instância de tipo de extensão: closure que fixa o
    /// receptor (avaliado uma vez) e repassa os argumentos à função de apoio.
    fn tearoff_tipo_ext(&mut self, recv: &Js, apoio: &str, chave: &str, targs: Vec<String>, fid: FunctionElementId, subst: &HashMap<u32, Ty>) -> (Js, Ty) {
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
        let chamada = format!("{apoio}[{}]({})", js::string_literal(chave), args.join(", "));
        let rti = self.rti(&ty);
        let valor = if type_params.is_empty() {
            format!("dart.fn(({}) => {chamada}, {rti})", ps.join(", "))
        } else {
            let padroes: Vec<String> = type_params.iter().map(|p| self.rti(&self.default_type_arg(&p.bound))).collect();
            format!(
                "dart.gFn(({}) => {chamada}, {rti}, dart.constList(dart_rti._Universe.eval(dart_rti._theUniverse(), \"@\", true), [{}]))",
                ps.join(", "),
                padroes.join(", ")
            )
        };
        (Js::new(format!("({r} = {}, {valor})", recv.at(P_ASSIGN)), P_PRIMARY), ty)
    }

    /// `recv.membro(args)`: método ou operador pela função de apoio; getter
    /// ou representação de tipo função, chamando o valor lido.
    pub fn tipo_ext_chamar(&mut self, recv: &Js, recv_e: Option<ExprId>, dono: ClassId, membro: MemberRef, arguments: &ast::Arguments, expected: Option<&Ty>) -> (Js, Ty) {
        let fid = match membro {
            MemberRef::Function(f) if !matches!(self.ctx.program.function(f).kind, FunctionKind::Getter) => f,
            _ => {
                let (g, gty) = self.tipo_ext_ler(recv, recv_e, dono, membro);
                return self.emit_fn_value_call(&g, &gty, arguments, expected);
            }
        };
        let (targs, subst) = self.args_do_receptor(recv_e, dono);
        let f = self.ctx.program.function(fid);
        let chave = chave_de_instancia(f.kind, self.name(f.name));
        let apoio = self.class_ref(dono);
        let fty = self.ctx.fn_ty(fid).subst_prop(&subst);
        let (args, ret, mtargs) = self.emit_args_for(&fty, arguments, expected);
        let mut todos = targs;
        todos.extend(mtargs);
        todos.push(recv.code.clone());
        todos.extend(args);
        (Js::prim(format!("{apoio}[{}]({})", js::string_literal(&chave), todos.join(", "))), ret)
    }

    /// Operador de instância (`a + b`, `-a`, `a[i]`) com operandos já emitidos.
    pub fn tipo_ext_operador(&mut self, recv: &Js, recv_e: Option<ExprId>, dono: ClassId, fid: FunctionElementId, operandos: &[Js]) -> (Js, Ty) {
        let (targs, subst) = self.args_do_receptor(recv_e, dono);
        let f = self.ctx.program.function(fid);
        let chave = chave_de_instancia(f.kind, self.name(f.name));
        let apoio = self.class_ref(dono);
        let ret = match self.ctx.fn_ty(fid).subst_prop(&subst) {
            Ty::Fn { ret, .. } => *ret,
            _ => Ty::Dynamic,
        };
        let mut todos = targs;
        todos.push(recv.code.clone());
        todos.extend(operandos.iter().map(|o| o.code.clone()));
        (Js::prim(format!("{apoio}[{}]({})", js::string_literal(&chave), todos.join(", "))), ret)
    }

    /// Tipo do parâmetro `i` de um membro de tipo de extensão (contexto do
    /// operando de um operador), com os argumentos de tipo do receptor.
    pub fn tipo_ext_parametro(&self, recv_e: Option<ExprId>, dono: ClassId, fid: FunctionElementId, i: usize) -> Option<Ty> {
        let (_, subst) = self.args_do_receptor(recv_e, dono);
        match self.ctx.fn_ty(fid).subst_prop(&subst) {
            Ty::Fn { pos, opt, .. } => pos.iter().chain(opt.iter()).nth(i).cloned(),
            _ => None,
        }
    }

    /// `recv.nome = v` por um setter de tipo de extensão. O valor da
    /// expressão é `v` (a função de apoio do setter não o devolve), guardado
    /// num temporário depois de o receptor ser avaliado.
    pub fn tipo_ext_escrever(&mut self, recv: &Js, recv_e: Option<ExprId>, dono: ClassId, fid: FunctionElementId, v: &Js, vty: &Ty) -> (Js, Ty) {
        let (targs, _) = self.args_do_receptor(recv_e, dono);
        let f = self.ctx.program.function(fid);
        let chave = chave_de_instancia(FunctionKind::Setter, self.name(f.name).trim_end_matches("_="));
        let apoio = self.class_ref(dono);
        let t = self.temp();
        let mut todos = targs;
        todos.push(recv.code.clone());
        todos.push(format!("{t} = {}", v.at(P_ASSIGN)));
        (Js::new(format!("({apoio}[{}]({}), {t})", js::string_literal(&chave), todos.join(", ")), P_PRIMARY), vty.clone())
    }

    /// O getter `nome` de instância de `dono`.
    pub fn getter_de_tipo_extensao(&self, dono: ClassId, nome: &str) -> Option<FunctionElementId> {
        let f = *self.ctx.program.class(dono).instance_members.get(&self.ctx.sym(nome)?)?;
        (self.ctx.program.function(f).kind == FunctionKind::Getter).then_some(f)
    }

    /// O operador `op` (`[]`, `[]=`) de instância de `dono`.
    pub fn operador_de_tipo_extensao(&self, dono: ClassId, op: &str) -> Option<FunctionElementId> {
        self.ctx.program.class(dono).instance_members.get(&self.ctx.sym(op)?).copied()
    }

    /// O setter `nome` de instância de `dono` (ou de um tipo de extensão que
    /// ele implementa), para uma escrita cuja resolução comum aponta o getter
    /// de mesmo nome.
    pub fn setter_de_tipo_extensao(&self, dono: ClassId, nome: &str) -> Option<FunctionElementId> {
        let sym = self.ctx.sym(&format!("{nome}_="))?;
        let f = *self.ctx.program.class(dono).instance_members.get(&sym)?;
        (self.ctx.program.function(f).kind == FunctionKind::Setter).then_some(f)
    }

    /// Construção `E.nome(args)` (ou `E(args)`) de um tipo de extensão
    /// apagado. O construtor primário é o valor do argumento; os demais, a
    /// função de apoio, com os argumentos de tipo (explícitos ou inferidos
    /// dos argumentos) à frente.
    pub fn tipo_ext_construir(&mut self, c: ClassId, class_args: Vec<Ty>, explicit_args: bool, ctor_name: &str, arguments: &ast::Arguments, expected: Option<&Ty>) -> (Js, Ty) {
        let params = self.ctx.class_params[c.0 as usize].clone();
        let explicitos: HashMap<u32, Ty> = if explicit_args { params.iter().zip(class_args.iter()).map(|(p, a)| (p.id, a.clone())).collect() } else { HashMap::new() };
        if nome_do_primario(self.ctx, c).as_deref() == Some(ctor_name) {
            // O contexto do argumento é a representação só quando ela não
            // depende de argumentos de tipo ainda por inferir.
            let rep = if explicit_args || params.is_empty() { Some(self.representacao(c, &explicitos)) } else { None };
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
        let mut subst = explicitos;
        let arg_js = if explicit_args || params.is_empty() {
            self.emit_args_infer(&ctor_fn.subst(&subst), arguments, &[], &mut HashMap::new(), expected).0
        } else {
            let (a, _) = self.emit_args_infer(&ctor_fn, arguments, &free, &mut subst, None);
            self.restringir_pelos_limites(&params, &free, &mut subst);
            a
        };
        let mut todos: Vec<String> = Vec::new();
        for p in &params {
            let t = subst.get(&p.id).cloned().unwrap_or_else(|| self.default_type_arg(&p.bound));
            subst.insert(p.id, t.clone());
            todos.push(self.rti(&t));
        }
        todos.extend(arg_js);
        let apoio = self.class_ref(c);
        let rep = self.representacao(c, &subst);
        (Js::prim(format!("{apoio}.{}({})", nome_de_construtor(ctor_name), todos.join(", "))), rep)
    }

    /// Tearoff `E.nome` / `E.new` de construtor de tipo de extensão apagado.
    pub fn tipo_ext_tearoff_construtor(&mut self, c: ClassId, targs: Vec<Ty>, nome: &str) -> Option<(Js, Ty)> {
        let ctor_name = if nome == "new" { "" } else { nome };
        let params = self.ctx.class_params[c.0 as usize].clone();
        let subst: HashMap<u32, Ty> = params.iter().zip(targs.iter()).map(|(p, a)| (p.id, a.clone())).collect();
        let rep = self.representacao(c, &subst);
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
        let apoio = self.class_ref(c);
        let rti = self.rti(&ty);
        Some((Js::prim(format!("dart.fn(({}) => {apoio}.{}({}), {rti})", ps.join(", "), nome_de_construtor(ctor_name), args.join(", "))), ty))
    }
}

/// Emite o objeto de apoio de um tipo de extensão apagado (ver o topo do
/// módulo).
pub(crate) fn emit_tipo_extensao(ctx: &Ctx, m: &ModState, c: ClassId, w: &mut Writer) {
    let class = ctx.program.class(c);
    let Some(decl) = class.decl else { return };
    let unit = decl.unit;
    let DeclKind::ExtensionType(ed) = &ctx.program.unit(unit).ast.decl(decl.decl).kind else { return };
    let lvar = ctx.libs[class.library.0 as usize].js_var.clone();
    let cname = ctx.class_name(c).to_string();
    let cref = format!("{lvar}.{cname}");
    let ast = &ctx.program.unit(unit).ast;
    let mut cw = Writer { indent: 1, ..Default::default() };
    let vivo = |f: FunctionElementId| ctx.estado_fn(f) != crate::filtro::Estado::Morta;
    let podado = |f: FunctionElementId| ctx.estado_fn(f) == crate::filtro::Estado::Stub;
    let mut campos_estaticos = Vec::new();
    // O construtor primário: identidade.
    let tps: Vec<String> = ctx.class_params[c.0 as usize].iter().map(|p| js::ident(&p.name)).collect();
    {
        let primario = ed.constructor.map(|n| ctx.name(n.sym).to_string()).unwrap_or_default();
        let mut ps = tps.clone();
        ps.push("v".into());
        cw.line(&format!("static {}({}) {{ return v; }}", js::prop_key(&nome_de_construtor(&primario)), ps.join(", ")));
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
                let mapa = if af.static_ { &class.static_members } else { &class.instance_members };
                let Some(feid) = ctx.sym(&key).and_then(|s| mapa.get(&s).copied()) else { continue };
                if !vivo(feid) {
                    continue;
                }
                let f = ctx.program.function(feid);
                if f.abstract_ {
                    continue;
                }
                let cabeca = if af.static_ {
                    let sk = js::prop_key(&static_member_name(&name));
                    match af.kind {
                        ast::FunctionKind::Getter => format!("static get {sk}"),
                        ast::FunctionKind::Setter => format!("static set {sk}"),
                        _ => format!("static {sk}"),
                    }
                } else {
                    format!("static [{}]", js::string_literal(&chave_de_instancia(f.kind, &name)))
                };
                if podado(feid) {
                    let r = js::string_literal(&ctx.rotulo_podado(class.library, Some(c), &name));
                    let texto = match af.kind {
                        ast::FunctionKind::Getter if af.static_ => format!("{cabeca}() {{ return dart_podado({r}); }}"),
                        ast::FunctionKind::Setter if af.static_ => format!("{cabeca}(v) {{ dart_podado({r}); }}"),
                        _ => format!("{cabeca}(...a) {{ return dart_podado({r}); }}"),
                    };
                    cw.line(&texto);
                    continue;
                }
                let texto = if af.static_ {
                    function_text(ctx, m, feid, Some(&cabeca), Some(c), true, None).0
                } else {
                    texto_de_membro(ctx, m, c, feid, &cabeca)
                };
                for line in texto.lines() {
                    cw.line(line);
                }
            }
            MemberKind::Constructor(ctor) => {
                if ctor.external {
                    continue;
                }
                let sym = ctor.name.map(|n| n.sym).or(ctx.empty_sym);
                let Some(fid) = sym.and_then(|s| class.constructors.get(&s).copied()) else { continue };
                if !vivo(fid) {
                    continue;
                }
                let texto = texto_de_construtor(ctx, m, c, unit, ctor, fid);
                for line in texto.lines() {
                    cw.line(line);
                }
            }
            MemberKind::Field(_) => {}
        }
    }
    for &vid in &class.fields {
        let v = ctx.program.variable(vid);
        if v.static_ && ctx.estado_var(vid) != crate::filtro::Estado::Morta {
            campos_estaticos.push(vid);
        }
    }
    crate::linha!(w, "{cref} = class {cname} {{");
    w.push_raw(&cw.out);
    w.line("};");
    // Campos estáticos: preguiçosos, como os de uma classe.
    let mut lazy: Vec<String> = Vec::new();
    for vid in campos_estaticos {
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

/// Emissor de um membro (de instância ou construtor) de `c`: parâmetros de
/// tipo de `c` em escopo como parâmetros de função e os estáticos de `c`
/// alcançáveis sem qualificação.
fn emissor<'m, 'a>(ctx: &'m Ctx<'a>, m: &'m ModState, c: ClassId, unit: dartforge_elements::model::UnitId) -> (FnEmitter<'m, 'a>, Vec<String>, Ty) {
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

fn tipo_assincrono(m: ast::AsyncModifier) -> AsyncKind {
    match m {
        ast::AsyncModifier::None => AsyncKind::None,
        ast::AsyncModifier::Async => AsyncKind::Async,
        ast::AsyncModifier::AsyncStar => AsyncKind::AsyncStar,
        ast::AsyncModifier::SyncStar => AsyncKind::SyncStar,
    }
}

/// `static ["#m"](T…, M…, $this, params) { corpo }` de um membro de instância.
fn texto_de_membro(ctx: &Ctx, m: &ModState, c: ClassId, fid: FunctionElementId, cabeca: &str) -> String {
    let f = ctx.program.function(fid);
    let FunctionRef::Function { unit, function } = f.node else { return String::new() };
    let af = ctx.program.unit(unit).ast.function(function);
    let (mut e, mut ps, rep) = emissor(ctx, m, c, unit);
    let data = &ctx.outline.functions[fid.0 as usize];
    for &pid in data.type_params.iter() {
        let p = ctx.ty_param_of(pid);
        let jsn = e.nome_js_parametro_de_tipo(&p.name);
        e.fn_type_params.insert(0, (p.id, jsn.clone()));
        ps.push(jsn);
    }
    let kind = tipo_assincrono(af.modifier);
    let ret_ty = ctx.ty_of(data.return_type);
    e.async_kind = kind;
    e.ret_ty = ret_ty.clone();
    ps.push("$this".into());
    e.extension_this = Some(rep.clone());
    if let Some(sym) = ctx.sym("this") {
        e.declare_js(sym, "$this".into(), rep);
    }
    let sig = ctx.fn_ty(fid);
    let parametros: &[ast::Parameter] = af.parameters.as_deref().unwrap_or(&[]);
    let (pjs, prologo) = e.declare_params(parametros, Some(&sig));
    if !pjs.is_empty() {
        ps.push(pjs);
    }
    e.emit_body(&af.body);
    let corpo = finish_body(&mut e);
    let head = format!("{cabeca}({}) {{", ps.join(", "));
    e.wrap_async_head(kind, &head, &prologo, &corpo, &ret_ty)
}

/// `static n(T…, params) { … }` de um construtor. O generativo calcula a
/// representação em `$this` (parâmetro inicializador, lista de
/// inicializadores ou redirecionamento `this(…)`) e roda o corpo, que vê
/// `this`; o factory é uma função comum que devolve o valor.
pub(crate) fn texto_de_construtor(ctx: &Ctx, m: &ModState, c: ClassId, unit: dartforge_elements::model::UnitId, ctor: &ast::Constructor, fid: FunctionElementId) -> String {
    let (mut e, mut ps, rep) = emissor(ctx, m, c, unit);
    let nome = nome_de_construtor(ctor.name.map(|n| ctx.name(n.sym)).unwrap_or(""));
    let sig = ctx.fn_ty(fid);
    let (pjs, prologo, nomes) = e.declare_params_nomes(&ctor.parameters, Some(&sig));
    if !pjs.is_empty() {
        ps.push(pjs);
    }
    let head = format!("static {}({}) {{", js::prop_key(&nome), ps.join(", "));
    if let Some(redir) = &ctor.redirect {
        // `factory E.f(…) = E2.g;`: repassa os parâmetros ao alvo.
        let alvo = alvo_de_redirecionamento(&e, redir);
        let corpo = match alvo {
            Some((tc, tnome)) => {
                let mut args: Vec<String> = ctx.class_params[tc.0 as usize].iter().map(|_| e.rti(&Ty::Dynamic)).collect();
                if tc == c {
                    args = e.fn_type_params.iter().map(|(_, j)| j.clone()).collect();
                }
                let mut nomeados = Vec::new();
                for (k, dart, jsn) in &nomes {
                    match k {
                        ast::ParameterKind::Named => nomeados.push(format!("{}: {jsn}", js::prop_key(dart))),
                        _ => args.push(jsn.clone()),
                    }
                }
                if !nomeados.is_empty() {
                    args.push(format!("{{{}}}", nomeados.join(", ")));
                }
                let tref = e.class_ref(tc);
                if e_tipo_extensao_apagado(ctx, tc) {
                    format!("return {tref}.{}({});", nome_de_construtor(&tnome), args.join(", "))
                } else {
                    String::new()
                }
            }
            None => String::new(),
        };
        return format!("{head}\n{}{}\n}}", indent(&prologo), indent(&corpo));
    }
    if ctor.factory {
        e.ret_ty = rep.clone();
        e.emit_body(&ctor.body);
        let corpo = finish_body(&mut e);
        return e.wrap_async_head(AsyncKind::None, &head, &prologo, &corpo, &rep);
    }
    // Generativo. No tipo de interop (`package:web`), o `this` é o próprio
    // tipo de extensão: os membros `external` dele, lidos e escritos pelo
    // `this` implícito no corpo, são acessos de propriedade sobre `$this`.
    let this_ty = if ctx.is_js_class(c) {
        Ty::Iface { class: c, args: Vec::new(), nullable: false }
    } else {
        rep.clone()
    };
    e.extension_this = Some(this_ty.clone());
    if let Some(sym) = ctx.sym("this") {
        e.declare_js(sym, "$this".into(), this_ty);
    }
    let mut corpo = Writer::default();
    corpo.line("let $this;");
    let rep_nome = ctx.program.class(c).representation.map(|v| ctx.program.variable(v).name);
    for p in ctor.parameters.iter() {
        if p.this_ && p.name.map(|n| n.sym) == rep_nome {
            if let Some(l) = p.name.and_then(|n| e.lookup_local(n.sym)) {
                crate::linha!(corpo, "$this = {};", l.js);
            }
        }
    }
    for init in ctor.initializers.iter() {
        match init {
            ast::Initializer::Field { value, .. } => {
                let (js, _) = e.emit_expr(*value, Some(&rep));
                flush_stmts(&mut e, &mut corpo);
                crate::linha!(corpo, "$this = {};", js.code);
            }
            ast::Initializer::Redirect { constructor, arguments, .. } => {
                let n = constructor.map(|n| ctx.name(n.sym).to_string()).unwrap_or_default();
                let args: Vec<Ty> = ctx.class_params[c.0 as usize].iter().map(|p| Ty::Param { id: p.id, name: p.name.clone(), nullable: false }).collect();
                let (js, _) = e.tipo_ext_construir(c, args, true, &n, arguments, None);
                flush_stmts(&mut e, &mut corpo);
                crate::linha!(corpo, "$this = {};", js.code);
            }
            ast::Initializer::Assert { condition, message, .. } => {
                let (cjs, _) = e.emit_cond(*condition);
                let msg = message.map(|mm| e.emit_expr(mm, None).0.code).unwrap_or("null".into());
                flush_stmts(&mut e, &mut corpo);
                crate::linha!(corpo, "if (!({cjs})) dart.assertFailed({msg}, null, 0, 0, \"\");");
            }
            ast::Initializer::Super { .. } => {}
        }
    }
    // O corpo pode ter `return;`: roda numa função seta, que vê `$this`.
    let tem_corpo = match &ctor.body {
        ast::FunctionBody::Block(b) => !ast_bloco_vazio(ctx, unit, *b),
        ast::FunctionBody::Expression(_) => true,
        _ => false,
    };
    if tem_corpo {
        e.emit_body(&ctor.body);
        let texto = std::mem::take(&mut e.w.out);
        crate::linha!(corpo, "(() => {{");
        corpo.push_raw(&indent(&texto));
        corpo.push_raw("\n");
        corpo.line("})();");
    }
    corpo.line("return $this;");
    let temps = if e.temps.is_empty() { String::new() } else { format!("let {};\n", e.temps.join(", ")) };
    format!("{head}\n{}{}{}}}", indent(&prologo), indent(&temps), indent(&corpo.out))
}

/// O corpo em bloco não tem comandos.
fn ast_bloco_vazio(ctx: &Ctx, unit: dartforge_elements::model::UnitId, b: ast::StmtId) -> bool {
    matches!(&ctx.program.unit(unit).ast.stmt(b).kind, ast::StmtKind::Block(s) if s.is_empty())
}

/// Classe e nome do construtor alvo de `= Alvo.nome` (o tipo escrito é
/// nomeado; `Alvo.nome` pode chegar como nome qualificado).
fn alvo_de_redirecionamento(e: &FnEmitter, redir: &ast::RedirectTarget) -> Option<(ClassId, String)> {
    let ast::TypeKind::Named { name: partes, .. } = &e.ast().ty(redir.ty).kind else { return None };
    let classe = |sym| match e.ctx.program.lookup(e.lib, sym).and_then(|b| b.getter) {
        Some(dartforge_elements::model::Element::Class(c)) => Some(c),
        _ => None,
    };
    let nome = redir.constructor.map(|n| e.name(n.sym).to_string());
    match (partes.len(), nome) {
        (1, n) => Some((classe(partes[0].sym)?, n.unwrap_or_default())),
        (2, None) => classe(partes[0].sym).map(|c| (c, e.name(partes[1].sym).to_string())),
        _ => None,
    }
}
