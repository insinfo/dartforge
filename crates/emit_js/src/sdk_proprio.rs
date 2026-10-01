//! Modo SDK do emissor: as bibliotecas `dart:` compiladas pela nossa trilha
//! (`docs/JS-PRODUCAO-SDK-PROPRIO.md` §5).
//!
//! O código do usuário nunca exercita o que o DDC faz de especial ao compilar
//! o SDK: os intrínsecos de `dart:_foreign_helper` (`JS('', '#.x', o)`,
//! `JS_GET_FLAG`, `TYPE_REF<T>()`…), as classes nativas (membros com símbolo
//! `dartx`, `registerExtension`), a biblioteca do runtime (`dart:_runtime`, que
//! **é** o objeto `dart`) e o módulo do SDK com o seu *bootstrap*. Este módulo
//! junta tudo isso; o resto do emissor só o consulta por ganchos pequenos,
//! todos atrás de `Ctx::sdk`.
//!
//! Sem `Ctx::sdk` nenhum gancho dispara: o perfil de desenvolvimento e o
//! caminho de produção que liga contra o `dart_sdk.js` emitem byte a byte o
//! de antes.
//!
//! As referências `compiler.dart:N` são do DDC **3.6.2**.

use crate::body::{js_member_name, FnEmitter};
use crate::ctx::Ctx;
use crate::js::{self, Js, P_ADD, P_PRIMARY, P_REL};
use crate::module::ModState;
use crate::ty::Ty;
use dartforge_elements::model::{ClassId, Element, FunctionElementId, FunctionKind, FunctionRef, LibraryId, UnitId, VariableId, VariableRef};
use dartforge_frontend::ast::{self, ExprId, ExprKind, StringPart};
use std::collections::{BTreeSet, HashMap, HashSet};

/// Uma chamada que o DDC resolve na compilação, sem chamar função nenhuma.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intrinseco {
    /// `JS(tipo, 'template', a…)` (`compiler.dart:6421-6479`).
    Js,
    GetFlag,
    GetName,
    EmbeddedGlobal,
    ClassRef,
    TypeRef,
    LegacyTypeRef,
    RawFunctionRef,
    StringConcat,
    Builtin,
    RawException,
    RtiParameter,
    RuntimeLibrary,
    GetInterceptor,
    /// `extensionSymbol('x')` do runtime → o símbolo `dartx.x`.
    ExtensionSymbol,
    /// `_jsInstanceOf(x, T)` do runtime → `x instanceof T`.
    InstanceOf,
    GetPrototypeOf,
    SetPrototypeOf,
    /// `staticInteropGlobalContext` (getter de `dart:_js_helper`) → `dart.global`.
    GlobalContext,
    UnsafeCast,
    ExtractTypeArguments,
    Spread,
}

/// O que o modo SDK sabe do programa, calculado uma vez (`ModoSdk::novo`).
pub struct ModoSdk {
    /// Declarações `@patch class X` de cada classe do SDK: os membros delas
    /// (os que substituem os `external` e os novos) entram na classe.
    pub patches: HashMap<ClassId, Vec<(UnitId, ast::DeclId)>>,
    /// `dart:_runtime`: a biblioteca cujo objeto **é** `dart`.
    pub runtime: Option<LibraryId>,
    /// `dart:_rti`, para os símbolos `_as`/`_is` de `JS_GET_NAME`.
    pub rti: Option<LibraryId>,
    pub intrinsecos: HashMap<FunctionElementId, Intrinseco>,
    /// `@JSExportName('x')` de funções (de topo e membros).
    pub nome_exportado: HashMap<FunctionElementId, String>,
    /// `@JSExportName('x')` de variáveis de topo (`global_` → `dart.global`).
    pub nome_exportado_var: HashMap<VariableId, String>,
    /// Tags de `@Native('A,B')`/`@JsPeerInterface(name: 'A')` por classe
    /// (sem as que começam com `!`, `native_types.dart:143-155`).
    pub pares: HashMap<ClassId, Vec<String>>,
    /// Bibliotecas com `@ReifyFunctionTypes(false)` na diretiva `library`.
    pub sem_reificar_lib: HashSet<LibraryId>,
    /// Funções com `@ReifyFunctionTypes(false)`.
    pub sem_reificar_fn: HashSet<FunctionElementId>,
    /// Funções com `@NoReifyGeneric()`: sem parâmetros de tipo no JS
    /// (`compiler.dart:3596-3598, 6394`).
    pub sem_generico: HashSet<FunctionElementId>,
    /// Parâmetros com `@nullCheck`, pelo início do nome na fonte (unidade, byte).
    pub verificar_nulo: HashSet<(u32, u32)>,
    /// Classes primitivas no JS (`JSBool`, `JSNumber`, `JSString`): ganham
    /// `definePrimitiveHashCode` (`compiler.dart:1114-1117`).
    pub primitivas: HashSet<ClassId>,
    /// Os `assert` do SDK não são emitidos (o dart2js também não emite os
    /// do SDK; os do programa ficam, como no perfil de desenvolvimento).
    pub sem_asserts_do_sdk: bool,
    /// Nomes de membro declarados por descendentes de cada classe (pela
    /// cadeia de superclasses e mixins) e as classes usadas como mixin:
    /// [`campo_nao_virtual`], calculado na primeira consulta.
    pub virtualidade: std::cell::OnceCell<(HashMap<ClassId, HashSet<dartforge_intern::SymbolId>>, HashSet<ClassId>)>,
}

/// Campo público de instância que nenhuma classe do programa sobrescreve nem
/// que sobrescreve um membro concreto herdado: no perfil de produção (mundo
/// fechado) ele vira propriedade comum do objeto, sem o par `get`/`set`
/// sobre um símbolo de armazenamento que o DDC escreve para todo campo
/// público porque, modular, não sabe se algum módulo o sobrescreverá
/// (`docs/JS-PRODUCAO-TAMANHO.md` §3.4). Classes nativas, mixins e classes
/// usadas como mixin ficam como antes.
pub fn campo_nao_virtual(ctx: &Ctx, c: ClassId, nome: dartforge_intern::SymbolId) -> bool {
    let Some(s) = ctx.sdk.as_ref() else { return false };
    let p = ctx.program;
    // Só no código do programa: as classes do SDK têm acessos que o emissor
    // não vê (`JS()`, *patches*, o runtime lendo campos por nome).
    if classe_nativa(ctx, c) || ctx.libs[p.class(c).library.0 as usize].is_sdk || p.class(c).kind != dartforge_elements::model::ClassKind::Class {
        return false;
    }
    let (desc, mixins) = s.virtualidade.get_or_init(|| {
        let mut desc: HashMap<ClassId, HashSet<dartforge_intern::SymbolId>> = HashMap::new();
        let mut mixins: HashSet<ClassId> = HashSet::new();
        for (i, k) in p.classes.iter().enumerate() {
            mixins.extend(k.mixin_classes.iter().copied());
            // Os ancestrais de `k` pela superclasse e pelos mixins aplicados.
            let mut vistos: HashSet<ClassId> = HashSet::new();
            let mut pilha: Vec<ClassId> = k.supertype_class.into_iter().chain(k.mixin_classes.iter().copied()).collect();
            while let Some(a) = pilha.pop() {
                if a.0 as usize == i || !vistos.insert(a) {
                    continue;
                }
                desc.entry(a).or_default().extend(k.instance_members.keys().copied());
                let ka = p.class(a);
                pilha.extend(ka.supertype_class);
                pilha.extend(ka.mixin_classes.iter().copied());
            }
        }
        (desc, mixins)
    });
    if mixins.contains(&c) {
        return false;
    }
    let texto = ctx.interner.resolve(nome);
    let setter = ctx.interner.lookup(&format!("{texto}_="));
    let tem = |k: ClassId| {
        let m = &p.class(k).instance_members;
        m.contains_key(&nome) || setter.is_some_and(|s| m.contains_key(&s))
    };
    // Sobrescrito por um descendente.
    if desc.get(&c).is_some_and(|ns| ns.contains(&nome) || setter.is_some_and(|s| ns.contains(&s))) {
        return false;
    }
    // Sobrescreve um membro de um ancestral (superclasse ou mixin aplicado).
    let mut vistos: HashSet<ClassId> = HashSet::new();
    let k = p.class(c);
    let mut pilha: Vec<ClassId> = k.supertype_class.into_iter().chain(k.mixin_classes.iter().copied()).collect();
    while let Some(a) = pilha.pop() {
        if a == c || !vistos.insert(a) {
            continue;
        }
        if tem(a) {
            return false;
        }
        let ka = p.class(a);
        pilha.extend(ka.supertype_class);
        pilha.extend(ka.mixin_classes.iter().copied());
    }
    true
}

fn nome_da_anotacao<'a>(ctx: &'a Ctx, a: &ast::Annotation) -> &'a str {
    a.name.last().map(|n| ctx.interner.resolve(n.sym)).unwrap_or("")
}

/// Primeiro argumento posicional de uma anotação, quando é literal de string
/// (ou argumento nomeado `name:` em `@JsPeerInterface(name: 'X')`).
fn texto_da_anotacao(ctx: &Ctx, unit: UnitId, a: &ast::Annotation) -> Option<String> {
    let args = a.arguments.as_ref()?;
    for arg in args.args.iter() {
        if let Some(n) = arg.name {
            if ctx.interner.resolve(n.sym) != "name" {
                continue;
            }
        }
        if let ExprKind::String(lit) = &ctx.program.unit(unit).ast.expr(arg.value).kind {
            return lit.constant_value().map(|v| v.to_string_lossy());
        }
    }
    None
}

fn bool_da_anotacao(ctx: &Ctx, unit: UnitId, a: &ast::Annotation) -> Option<bool> {
    let args = a.arguments.as_ref()?;
    let first = args.args.first()?;
    match &ctx.program.unit(unit).ast.expr(first.value).kind {
        ExprKind::Bool(b) => Some(*b),
        _ => None,
    }
}

impl ModoSdk {
    /// Lê do programa o que o modo SDK precisa: intrínsecos, anotações do
    /// DDC, classes nativas.
    pub fn novo(ctx: &Ctx) -> ModoSdk {
        let p = ctx.program;
        let lib_por_uri = |uri: &str| p.libraries.iter().position(|l| l.uri == uri).map(|i| LibraryId(i as u32));
        let runtime = lib_por_uri("dart:_runtime");
        let rti = lib_por_uri("dart:_rti");
        let foreign = lib_por_uri("dart:_foreign_helper");
        let js_helper = lib_por_uri("dart:_js_helper");
        let internal = lib_por_uri("dart:_internal");
        let mut m = ModoSdk {
            runtime,
            rti,
            intrinsecos: HashMap::new(),
            nome_exportado: HashMap::new(),
            nome_exportado_var: HashMap::new(),
            pares: HashMap::new(),
            sem_reificar_lib: HashSet::new(),
            sem_reificar_fn: HashSet::new(),
            sem_generico: HashSet::new(),
            verificar_nulo: HashSet::new(),
            primitivas: [ctx.jsbool, ctx.jsnumber, ctx.jsstring].into_iter().flatten().collect(),
            sem_asserts_do_sdk: true,
            patches: HashMap::new(),
            virtualidade: std::cell::OnceCell::new(),
        };
        // Intrínsecos, por (biblioteca, nome).
        for (i, f) in p.functions.iter().enumerate() {
            let fid = FunctionElementId(i as u32);
            if f.class.is_some() || f.extension.is_some() {
                continue;
            }
            let n = ctx.interner.resolve(f.name);
            let lib = Some(f.library);
            let k = if lib == foreign {
                match n {
                    "JS" => Some(Intrinseco::Js),
                    "JS_GET_FLAG" => Some(Intrinseco::GetFlag),
                    "JS_GET_NAME" => Some(Intrinseco::GetName),
                    "JS_EMBEDDED_GLOBAL" => Some(Intrinseco::EmbeddedGlobal),
                    "JS_CLASS_REF" => Some(Intrinseco::ClassRef),
                    "TYPE_REF" => Some(Intrinseco::TypeRef),
                    "LEGACY_TYPE_REF" => Some(Intrinseco::LegacyTypeRef),
                    "RAW_DART_FUNCTION_REF" => Some(Intrinseco::RawFunctionRef),
                    "JS_STRING_CONCAT" => Some(Intrinseco::StringConcat),
                    "JS_BUILTIN" => Some(Intrinseco::Builtin),
                    "JS_RAW_EXCEPTION" => Some(Intrinseco::RawException),
                    "JS_RTI_PARAMETER" => Some(Intrinseco::RtiParameter),
                    "DART_RUNTIME_LIBRARY" => Some(Intrinseco::RuntimeLibrary),
                    "getInterceptor" => Some(Intrinseco::GetInterceptor),
                    "spread" => Some(Intrinseco::Spread),
                    _ => None,
                }
            } else if lib == runtime {
                match n {
                    "extensionSymbol" => Some(Intrinseco::ExtensionSymbol),
                    "_jsInstanceOf" => Some(Intrinseco::InstanceOf),
                    _ => None,
                }
            } else if lib == js_helper {
                match n {
                    "jsObjectGetPrototypeOf" => Some(Intrinseco::GetPrototypeOf),
                    "jsObjectSetPrototypeOf" => Some(Intrinseco::SetPrototypeOf),
                    "staticInteropGlobalContext" => Some(Intrinseco::GlobalContext),
                    "spread" => Some(Intrinseco::Spread),
                    _ => None,
                }
            } else if lib == internal {
                match n {
                    "unsafeCast" => Some(Intrinseco::UnsafeCast),
                    "extractTypeArguments" => Some(Intrinseco::ExtractTypeArguments),
                    _ => None,
                }
            } else {
                None
            };
            if let Some(k) = k {
                m.intrinsecos.insert(fid, k);
            }
        }
        // Anotações, varrendo as unidades do SDK.
        let mut fn_por_no: HashMap<(u32, u32), FunctionElementId> = HashMap::new();
        for (i, f) in p.functions.iter().enumerate() {
            if !p.library(f.library).is_sdk {
                continue;
            }
            if let FunctionRef::Function { unit, function } = f.node {
                fn_por_no.insert((unit.0, function.0), FunctionElementId(i as u32));
            }
        }
        let mut var_por_no: HashMap<(u32, u32, usize), VariableId> = HashMap::new();
        for (i, v) in p.variables.iter().enumerate() {
            if !p.library(v.library).is_sdk {
                continue;
            }
            if let VariableRef::TopLevel { unit, decl, index } = v.node {
                var_por_no.insert((unit.0, decl.0, index), VariableId(i as u32));
            }
        }
        let mut classe_por_decl: HashMap<(u32, u32), ClassId> = HashMap::new();
        for (i, c) in p.classes.iter().enumerate() {
            if let Some(d) = c.decl {
                if p.library(c.library).is_sdk {
                    classe_por_decl.insert((d.unit.0, d.decl.0), ClassId(i as u32));
                }
            }
        }
        for (li, lib) in p.libraries.iter().enumerate() {
            if !lib.is_sdk {
                continue;
            }
            let lid = LibraryId(li as u32);
            for (ui, &uid) in lib.units.iter().enumerate() {
                let unit = p.unit(uid);
                if ui == 0 {
                    for d in &unit.unit.directives {
                        if let ast::DirectiveKind::Library { .. } = d.kind {
                            for a in &d.metadata {
                                if nome_da_anotacao(ctx, a) == "ReifyFunctionTypes" && bool_da_anotacao(ctx, uid, a) == Some(false) {
                                    m.sem_reificar_lib.insert(lid);
                                }
                            }
                        }
                    }
                }
                let ast = &unit.ast;
                for (di, decl) in ast.decls.iter().enumerate() {
                    let did = di as u32;
                    match &decl.kind {
                        ast::DeclKind::Function(fid) => {
                            let Some(&fe) = fn_por_no.get(&(uid.0, fid.0)) else { continue };
                            m.anotacoes_de_funcao(ctx, uid, &decl.metadata, fe);
                            m.parametros(uid, ctx, ast.function(*fid));
                        }
                        ast::DeclKind::Variables(l) => {
                            for a in decl.metadata.iter() {
                                if nome_da_anotacao(ctx, a) == "JSExportName" {
                                    if let Some(n) = texto_da_anotacao(ctx, uid, a) {
                                        for idx in 0..l.variables.len() {
                                            if let Some(&v) = var_por_no.get(&(uid.0, did, idx)) {
                                                m.nome_exportado_var.insert(v, n.clone());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        ast::DeclKind::Class(cd) => {
                            // `@patch class X`: os membros vão para a classe original.
                            let e_patch = decl.metadata.iter().any(|a| nome_da_anotacao(ctx, a) == "patch");
                            if e_patch {
                                if let Some(Element::Class(orig)) = lib.declared.get(&cd.name.sym).and_then(|b| b.getter) {
                                    if p.class(orig).decl.is_some_and(|d| (d.unit, d.decl) != (uid, ast::DeclId(did))) {
                                        m.patches.entry(orig).or_default().push((uid, ast::DeclId(did)));
                                    }
                                }
                            }
                            if let Some(&c) = classe_por_decl.get(&(uid.0, did)) {
                                for a in decl.metadata.iter() {
                                    let n = nome_da_anotacao(ctx, a);
                                    if n == "Native" || n == "JsPeerInterface" {
                                        if let Some(t) = texto_da_anotacao(ctx, uid, a) {
                                            let tags: Vec<String> = t.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty() && !s.starts_with('!')).collect();
                                            m.pares.insert(c, tags);
                                        }
                                    }
                                }
                            }
                            for &mid in cd.members.iter() {
                                m.membro(ctx, uid, &fn_por_no, ast.member(mid));
                            }
                        }
                        ast::DeclKind::Mixin(md) => {
                            for &mid in md.members.iter() {
                                m.membro(ctx, uid, &fn_por_no, ast.member(mid));
                            }
                        }
                        ast::DeclKind::Extension(ed) => {
                            for &mid in ed.members.iter() {
                                m.membro(ctx, uid, &fn_por_no, ast.member(mid));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        m
    }

    fn membro(&mut self, ctx: &Ctx, uid: UnitId, fn_por_no: &HashMap<(u32, u32), FunctionElementId>, mem: &ast::Member) {
        match &mem.kind {
            ast::MemberKind::Method(fid) => {
                if let Some(&fe) = fn_por_no.get(&(uid.0, fid.0)) {
                    self.anotacoes_de_funcao(ctx, uid, &mem.metadata, fe);
                }
                self.parametros(uid, ctx, ctx.program.unit(uid).ast.function(*fid));
            }
            ast::MemberKind::Constructor(k) => {
                for p in k.parameters.iter() {
                    self.parametro(uid, ctx, p);
                }
            }
            ast::MemberKind::Field(_) => {}
        }
    }

    fn anotacoes_de_funcao(&mut self, ctx: &Ctx, uid: UnitId, metadata: &[ast::Annotation], fe: FunctionElementId) {
        for a in metadata {
            match nome_da_anotacao(ctx, a) {
                "JSExportName" => {
                    if let Some(n) = texto_da_anotacao(ctx, uid, a) {
                        self.nome_exportado.insert(fe, n);
                    }
                }
                "ReifyFunctionTypes" if bool_da_anotacao(ctx, uid, a) == Some(false) => {
                    self.sem_reificar_fn.insert(fe);
                }
                "NoReifyGeneric" => {
                    self.sem_generico.insert(fe);
                }
                _ => {}
            }
        }
    }

    fn parametros(&mut self, uid: UnitId, ctx: &Ctx, f: &ast::Function) {
        for p in f.parameters.iter().flat_map(|ps| ps.iter()) {
            self.parametro(uid, ctx, p);
        }
    }

    fn parametro(&mut self, uid: UnitId, ctx: &Ctx, p: &ast::Parameter) {
        if p.metadata.iter().any(|a| nome_da_anotacao(ctx, a) == "nullCheck") {
            if let Some(n) = p.name {
                self.verificar_nulo.insert((uid.0, n.span.start as u32));
            }
        }
    }

    /// Valor de `JS_GET_FLAG('X')` (`compiler.dart:6147-6177`), com null
    /// safety sólida e o DDC como compilador.
    pub fn flag(nome: &str) -> Option<bool> {
        match nome {
            "DEV_COMPILER" | "SOUND_NULL_SAFETY" | "VARIANCE" => Some(true),
            "MINIFIED" | "LEGACY" | "EXTRA_NULL_SAFETY_CHECKS" | "PRINT_LEGACY_STARS" => Some(false),
            _ => None,
        }
    }
}

// --------------------------------------------------------------------- consultas

/// Os membros de uma classe com os das declarações `@patch` dela (no modo
/// SDK); fora dele, os de sempre ([`Program::membros_da_classe`]).
pub fn membros_da_classe(ctx: &Ctx, c: ClassId) -> Vec<(UnitId, ast::MemberId)> {
    let mut out = ctx.program.membros_da_classe(c);
    if let Some(ps) = ctx.sdk.as_ref().and_then(|s| s.patches.get(&c)) {
        for &(u, d) in ps {
            if let ast::DeclKind::Class(cd) = &ctx.program.unit(u).ast.decl(d).kind {
                out.extend(cd.members.iter().map(|&m| (u, m)));
            }
        }
    }
    out
}

/// A classe é nativa no sentido do DDC (`isNativeClass`): `int`, `double`,
/// `bool`, `String` e as `@Native` das bibliotecas de interceptadores, dados
/// tipados e web. Só no modo SDK.
pub fn classe_nativa(ctx: &Ctx, c: ClassId) -> bool {
    ctx.sdk.is_some() && ctx.native_set.contains(&c)
}

/// O emissor está compilando código de uma biblioteca do SDK no modo SDK.
pub fn em_sdk(ctx: &Ctx, lib: LibraryId) -> bool {
    ctx.sdk.is_some() && ctx.libs[lib.0 as usize].is_sdk
}

/// Nome da propriedade de uma função de topo (ou estática) com
/// `@JSExportName`: `throw_` → `throw`.
pub fn nome_de_topo<'c>(ctx: &'c Ctx, fid: FunctionElementId) -> Option<&'c str> {
    ctx.sdk.as_ref()?.nome_exportado.get(&fid).map(String::as_str)
}

pub fn nome_de_topo_var<'c>(ctx: &'c Ctx, vid: VariableId) -> Option<&'c str> {
    ctx.sdk.as_ref()?.nome_exportado_var.get(&vid).map(String::as_str)
}

/// Funções desta biblioteca/função não ganham `dart.fn` (`@ReifyFunctionTypes(false)`,
/// `compiler.dart:7306-7338`).
pub fn sem_reificar(ctx: &Ctx, lib: LibraryId) -> bool {
    ctx.sdk.as_ref().is_some_and(|s| s.sem_reificar_lib.contains(&lib))
}

pub fn funcao_sem_reificar(ctx: &Ctx, fid: FunctionElementId) -> bool {
    ctx.sdk.as_ref().is_some_and(|s| s.sem_reificar_fn.contains(&fid) || s.sem_reificar_lib.contains(&ctx.program.function(fid).library))
}

/// A função tem `@NoReifyGeneric()`.
pub fn sem_generico(ctx: &Ctx, fid: FunctionElementId) -> bool {
    ctx.sdk.as_ref().is_some_and(|s| s.sem_generico.contains(&fid))
}

/// O `assert` não é emitido: modo SDK e a unidade é do SDK.
pub fn omitir_assert(ctx: &Ctx, lib: LibraryId) -> bool {
    // O único `assert` do `dart:_runtime` é o de `assertInterop` (função Dart
    // passada ao JS sem `allowInterop`), comportamento observável que o
    // `dart_sdk.js` do DDC mantém: fica.
    (ctx.sdk.as_ref().is_some_and(|s| s.sem_asserts_do_sdk && s.runtime != Some(lib)) && ctx.libs[lib.0 as usize].is_sdk)
        || ctx.filtro.is_some_and(|f| !f.manter_asserts())
}

/// Conferências de `@nullCheck` dos parâmetros já declarados (`compiler.dart:3855`):
/// `if (p == null) dart.argumentError(p);`.
pub fn verificacoes_de_nulo(e: &FnEmitter, params: &[ast::Parameter]) -> String {
    let Some(s) = e.ctx.sdk.as_ref() else { return String::new() };
    if s.verificar_nulo.is_empty() || !e.ctx.libs[e.lib.0 as usize].is_sdk {
        return String::new();
    }
    let mut out = String::new();
    for p in params {
        let Some(n) = p.name else { continue };
        if !s.verificar_nulo.contains(&(e.unit.0, n.span.start as u32)) {
            continue;
        }
        if let Some(l) = e.lookup_local(n.sym) {
            out.push_str(&format!("if ({0} == null) dart.argumentError({0});\n", l.js));
        }
    }
    out
}

/// Inicializador que o DDC dobra no ponto de uso (`kernel/constants.dart:105-120`,
/// `shouldInlineConstant`): literal numérico, booleano, nulo ou *string*
/// com menos de 32 unidades. O mesmo critério está em `crates/mundo`
/// (`constante_inlinavel`), que não mantém viva a variável assim lida.
pub fn literal_inlinavel(ast: &ast::Ast, x: ExprId) -> bool {
    match &ast.expr(x).kind {
        ExprKind::Int(_) | ExprKind::Double(_) | ExprKind::Bool(_) | ExprKind::Null => true,
        ExprKind::String(lit) => lit.constant_value().is_some_and(|v| v.utf16_len() < 32),
        ExprKind::Unary { op: ast::UnaryOp::Neg, operand } => matches!(ast.expr(*operand).kind, ExprKind::Int(_) | ExprKind::Double(_)),
        ExprKind::Parenthesized(i) => literal_inlinavel(ast, *i),
        _ => false,
    }
}

/// A leitura de uma constante primitiva vira o próprio valor (modo SDK, que é
/// o perfil de produção): `dart_rti.interfaceTypeRecipePropertyName` sai
/// `"$interfaceRecipe"`, e a variável deixa de existir no arquivo.
pub fn constante_inline(e: &FnEmitter, vid: VariableId) -> Option<Js> {
    e.ctx.sdk.as_ref()?;
    let v = e.ctx.program.variable(vid);
    if !v.const_ || v.extension.is_some() {
        return None;
    }
    let (unit, init) = match v.node {
        VariableRef::TopLevel { unit, decl, index } => match &e.ctx.program.unit(unit).ast.decl(decl).kind {
            ast::DeclKind::Variables(l) => (unit, l.variables.get(index)?.initializer?),
            _ => return None,
        },
        VariableRef::Field { unit, member, index } => match &e.ctx.program.unit(unit).ast.member(member).kind {
            ast::MemberKind::Field(l) => (unit, l.variables.get(index)?.initializer?),
            _ => return None,
        },
        _ => return None,
    };
    if !literal_inlinavel(&e.ctx.program.unit(unit).ast, init) {
        return None;
    }
    let mut sub = FnEmitter::new(e.ctx, e.m, unit, None, true);
    let ty = e.ctx.var_ty(vid);
    let (js, _) = sub.emit_expr(init, Some(&ty));
    Some(js)
}

/// Conferências de covariância dos parâmetros de um método de instância de
/// classe genérica do SDK (`compiler.dart:3848-3856`): parâmetro cujo tipo
/// cita um parâmetro de tipo da classe (ou marcado `covariant`) é conferido
/// na entrada — `List<num> l = <int>[]; l.add(2.5)` lança `TypeError` no
/// `JSArray.add`.
pub fn verificacoes_de_covariancia(e: &FnEmitter, params: &[ast::Parameter]) -> String {
    let Some(c) = e.class else { return String::new() };
    if e.ctx.sdk.is_none() || e.is_static || !e.ctx.libs[e.lib.0 as usize].is_sdk {
        return String::new();
    }
    let da_classe: HashSet<u32> = e.ctx.class_params[c.0 as usize].iter().map(|p| p.id).collect();
    let mut out = String::new();
    for p in params {
        let Some(n) = p.name else { continue };
        let Some(l) = e.lookup_local(n.sym) else { continue };
        // Só a ocorrência **covariante** do parâmetro de tipo pede conferência
        // (`void Function(T)` é contravariante em `T`: sempre seguro).
        let cita = ocorre_covariante(&l.ty, &da_classe, true);
        if !(cita || p.covariant) || matches!(l.ty, Ty::Dynamic) {
            continue;
        }
        // `null` passa: o tipo local pode ter perdido o `?` de um parâmetro-função
        // (`E Function()? orElse`), e a chamada tipada já provou a nulidade.
        out.push_str(&format!("if ({0} != null) {1}[_as]({0});\n", l.js, e.rti(&l.ty)));
    }
    out
}

/// Um parâmetro de tipo de `ids` ocorre em posição covariante de `t`
/// (`positivo` é a polaridade corrente; parâmetro de função a inverte).
fn ocorre_covariante(t: &Ty, ids: &HashSet<u32>, positivo: bool) -> bool {
    match t {
        Ty::Param { id, .. } => positivo && ids.contains(id),
        Ty::Iface { args, .. } => args.iter().any(|a| ocorre_covariante(a, ids, positivo)),
        Ty::FutureOr { arg, .. } => ocorre_covariante(arg, ids, positivo),
        Ty::Fn { ret, pos, opt, named, .. } => {
            ocorre_covariante(ret, ids, positivo)
                || pos.iter().chain(opt.iter()).any(|p| ocorre_covariante(p, ids, !positivo))
                || named.iter().any(|(_, p, _)| ocorre_covariante(p, ids, !positivo))
        }
        Ty::Record { pos, named, .. } => pos.iter().any(|p| ocorre_covariante(p, ids, positivo)) || named.iter().any(|(_, p)| ocorre_covariante(p, ids, positivo)),
        _ => false,
    }
}

/// `@rest` no último parâmetro posicional (`compiler.dart:3722-3726`): o
/// parâmetro JS vira `...nome` — `gbind(f, @rest typeArgs)` recebe todos os
/// argumentos seguintes num array.
pub fn com_rest(e: &FnEmitter, params: &[ast::Parameter], js: String) -> String {
    if e.ctx.sdk.is_none() || !e.ctx.libs[e.lib.0 as usize].is_sdk {
        return js;
    }
    let Some(ultimo) = params.iter().rev().find(|p| p.kind != ast::ParameterKind::Named) else { return js };
    if !ultimo.metadata.iter().any(|a| nome_da_anotacao(e.ctx, a) == "rest") {
        return js;
    }
    let Some(n) = ultimo.name else { return js };
    let Some(l) = e.lookup_local(n.sym) else { return js };
    let nome = l.js.clone();
    // O nome é o último item da lista de parâmetros posicionais.
    match js.rfind(nome.as_str()) {
        Some(p) if js[p..].trim_end() == nome => format!("{}...{}", &js[..p], &js[p..]),
        _ => js,
    }
}

// --------------------------------------------------------------------- intrínsecos

/// O intrínseco que `fid` é, se for.
pub fn intrinseco_de(ctx: &Ctx, fid: FunctionElementId) -> Option<Intrinseco> {
    ctx.sdk.as_ref()?.intrinsecos.get(&fid).copied()
}

/// Valor de string constante de uma expressão: literal sem interpolação ou
/// identificador de constante de topo com inicializador literal (o
/// `JS_EMBEDDED_GLOBAL('', RTI_UNIVERSE)` do `rti.dart`).
fn string_constante(e: &FnEmitter, x: ExprId) -> Option<String> {
    match &e.expr(x).kind {
        ExprKind::String(lit) => lit.constant_value().map(|v| v.to_string_lossy()),
        ExprKind::Identifier(id) => {
            if let crate::expr::IdentTarget::Element(Element::Variable(v)) = e.alvo_do_identificador(id.sym, x) {
                return string_de_variavel(e.ctx, v);
            }
            None
        }
        ExprKind::Property { target, name, .. } => {
            // `prefixo.CONST`
            if let ExprKind::Identifier(id) = &e.expr(*target).kind {
                if let crate::expr::IdentTarget::Prefix(pf) = e.alvo_do_identificador(id.sym, *target) {
                    let b = e.ctx.program.lookup_prefixed_na_unidade(e.unit, pf, name.sym)?;
                    if let Some(Element::Variable(v)) = b.getter {
                        return string_de_variavel(e.ctx, v);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

fn string_de_variavel(ctx: &Ctx, v: VariableId) -> Option<String> {
    let var = ctx.program.variable(v);
    if !var.const_ {
        return None;
    }
    let VariableRef::TopLevel { unit, decl, index } = var.node else { return None };
    let ast = &ctx.program.unit(unit).ast;
    let ast::DeclKind::Variables(l) = &ast.decl(decl).kind else { return None };
    let init = l.variables.get(index)?.initializer?;
    match &ast.expr(init).kind {
        ExprKind::String(lit) => lit.constant_value().map(|v| v.to_string_lossy()),
        _ => None,
    }
}

/// Nome do membro de enum citado (`JsGetName.RTI_NAME` → `RTI_NAME`).
fn nome_de_enum(e: &FnEmitter, x: ExprId) -> Option<String> {
    match &e.expr(x).kind {
        ExprKind::Property { name, .. } => Some(e.name(name.sym).to_string()),
        ExprKind::Identifier(id) => Some(e.name(id.sym).to_string()),
        _ => None,
    }
}

/// A classe de um literal de tipo (`Foo` ou `prefixo.Foo`).
fn classe_do_literal(e: &FnEmitter, x: ExprId) -> Option<ClassId> {
    match &e.expr(x).kind {
        ExprKind::Identifier(id) => match e.alvo_do_identificador(id.sym, x) {
            crate::expr::IdentTarget::Element(Element::Class(c)) => Some(c),
            crate::expr::IdentTarget::Element(Element::Typedef(td)) => match e.ctx.ty_of(e.ctx.outline.typedefs[td.0 as usize].target_type) {
                Ty::Iface { class, .. } => Some(class),
                _ => None,
            },
            _ => None,
        },
        ExprKind::Property { target, name, .. } => {
            let ExprKind::Identifier(id) = &e.expr(*target).kind else { return None };
            let crate::expr::IdentTarget::Prefix(pf) = e.alvo_do_identificador(id.sym, *target) else { return None };
            let b = e.ctx.program.lookup_prefixed_na_unidade(e.unit, pf, name.sym)?;
            match b.getter {
                Some(Element::Class(c)) => Some(c),
                _ => None,
            }
        }
        ExprKind::TypeArguments { target, .. } => classe_do_literal(e, *target),
        _ => None,
    }
}

/// Tipo estático de uma chamada a um intrínseco genérico (`JS<T>`,
/// `unsafeCast<T>`): o argumento de tipo explícito, senão o do contexto
/// (inferência descendente, como o CFE faz), senão `dynamic`.
fn tipo_do_generico(e: &FnEmitter, args: &ast::Arguments, expected: Option<&Ty>) -> Ty {
    if let Some(t) = args.type_args.first() {
        return e.resolve_type(*t);
    }
    match expected {
        Some(t) if !matches!(t, Ty::Void) => t.clone(),
        _ => Ty::Dynamic,
    }
}

/// Emite um argumento de `JS()` (`_isInForeignJS`, `compiler.dart:6460-6463`).
fn arg_estrangeiro(e: &mut FnEmitter, x: ExprId) -> Js {
    let salvo = std::mem::replace(&mut e.em_js_estrangeiro, true);
    let (js, _) = e.emit_expr(x, None);
    e.em_js_estrangeiro = salvo;
    js
}

/// Uma peça do *template*: texto JS ou um buraco.
enum Peca {
    Texto(String),
    Buraco(ExprId),
}

/// As peças do *template* de `JS()`: literal com `#` (buracos dos argumentos
/// seguintes) ou com interpolações Dart (cada `$x` vira um buraco, na ordem).
fn pecas_do_template(e: &FnEmitter, tpl: ExprId, resto: &[ExprId]) -> Option<Vec<Peca>> {
    let ExprKind::String(lit) = &e.expr(tpl).kind else { return None };
    let mut texto = String::new();
    let mut interp: Vec<ExprId> = Vec::new();
    for part in lit.parts.iter() {
        match part {
            StringPart::Text(t) => texto.push_str(&t.to_string_lossy()),
            StringPart::Interpolation(x) => {
                // Interpolação de constante de string: o DDC dobra a
                // constante e o valor entra como texto JS (`compiler.dart:6435`).
                if let Some(s) = string_constante(e, *x) {
                    texto.push_str(&s);
                } else {
                    texto.push('#');
                    interp.push(*x);
                }
            }
        }
    }
    let buracos: Vec<ExprId> = if interp.is_empty() { resto.to_vec() } else { interp };
    let mut out = Vec::new();
    let mut i = 0usize;
    let b = texto.as_bytes();
    let mut ini = 0usize;
    let mut n = 0usize;
    while i < b.len() {
        match b[i] {
            b'\'' | b'"' | b'`' => {
                let q = b[i];
                i += 1;
                while i < b.len() && b[i] != q {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
                i += 1;
            }
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
            }
            b'#' => {
                out.push(Peca::Texto(texto[ini..i].to_string()));
                let x = *buracos.get(n)?;
                n += 1;
                out.push(Peca::Buraco(x));
                i += 1;
                ini = i;
            }
            _ => i += 1,
        }
    }
    out.push(Peca::Texto(texto[ini.min(texto.len())..].to_string()));
    Some(out)
}

fn e_ident_js(s: &str) -> bool {
    js::is_js_ident(s)
}

/// Monta o texto de `JS(tipo, 'template', a…)`. `None` se a chamada não tem a
/// forma esperada (então é emitida como chamada comum, e o erro aparece).
pub fn texto_js(e: &mut FnEmitter, args: &ast::Arguments) -> Option<String> {
    let lista: Vec<ExprId> = args.args.iter().map(|a| a.value).collect();
    if lista.len() < 2 {
        return None;
    }
    let pecas = pecas_do_template(e, lista[1], &lista[2..])?;
    // Os argumentos são emitidos na ordem em que aparecem (efeitos e temps).
    let mut emitidos: HashMap<u32, (Js, Option<String>)> = HashMap::new();
    for p in &pecas {
        if let Peca::Buraco(x) = p {
            // Literal de string que é identificador: pode virar nome de
            // propriedade (`#.#` com `'foo'` dá `.foo`, `compiler.dart` via
            // o impressor do `js_ast`).
            let lit = match &e.expr(*x).kind {
                ExprKind::String(l) => l.constant_value().map(|v| v.to_string_lossy()),
                _ => None,
            };
            let js = arg_estrangeiro(e, *x);
            emitidos.insert(x.0, (js, lit));
        }
    }
    let mut out = String::new();
    for p in &pecas {
        match p {
            Peca::Texto(t) => out.push_str(t),
            Peca::Buraco(x) => {
                let (js, lit) = &emitidos[&x.0];
                let antes = out.trim_end();
                // Só o ponto na mesma linha: o de fim de comentário não conta.
                let colado = out.trim_end_matches([' ', '\t']);
                let depois_ponto = colado.ends_with('.') && !colado.ends_with("..");
                if depois_ponto {
                    match lit {
                        Some(s) if e_ident_js(s) => out.push_str(s),
                        _ => {
                            // `a.#` com expressão: `a[expr]`.
                            let corte = colado.len() - 1;
                            out.truncate(corte);
                            out.push('[');
                            out.push_str(&js.code);
                            out.push(']');
                        }
                    }
                    continue;
                }
                let e_chave = (antes.ends_with('{') || antes.ends_with(',')) && proximo_nao_branco_e(&pecas, x, ':');
                if e_chave {
                    match lit {
                        Some(s) if e_ident_js(s) => out.push_str(s),
                        _ => {
                            out.push('[');
                            out.push_str(&js.code);
                            out.push(']');
                        }
                    }
                    continue;
                }
                let depois_new = antes.ends_with("new") && antes.len() >= 3 && !antes[..antes.len() - 3].ends_with(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '$');
                if depois_new {
                    out.push('(');
                    out.push_str(&js.code);
                    out.push(')');
                } else {
                    out.push_str(&js.at(P_PRIMARY));
                }
            }
        }
    }
    Some(out)
}

/// O texto que segue o buraco `x` começa com `c` (depois de espaços).
fn proximo_nao_branco_e(pecas: &[Peca], x: &ExprId, c: char) -> bool {
    let mut achou = false;
    for p in pecas {
        match p {
            Peca::Buraco(y) if y.0 == x.0 => achou = true,
            Peca::Texto(t) if achou => return t.trim_start().starts_with(c),
            Peca::Buraco(_) if achou => return false,
            _ => {}
        }
    }
    false
}

/// `JS()` cujo *template* é uma instrução (`throw …`, `compiler.dart` via
/// `parseForeignJS`): o texto, para ser escrito em posição de instrução.
pub fn instrucao_js(e: &mut FnEmitter, x: ExprId) -> Option<String> {
    e.ctx.sdk.as_ref()?;
    let ExprKind::Call { target, arguments } = &e.expr(x).kind else { return None };
    let fid = funcao_chamada(e, *target)?;
    if intrinseco_de(e.ctx, fid) != Some(Intrinseco::Js) {
        return None;
    }
    let tpl = arguments.args.get(1)?.value;
    let ExprKind::String(lit) = &e.expr(tpl).kind else { return None };
    let comeca = match lit.parts.first() {
        Some(StringPart::Text(t)) => t.to_string_lossy().trim_start().starts_with("throw "),
        _ => false,
    };
    if !comeca {
        return None;
    }
    texto_js(e, arguments)
}

/// A função de topo chamada por `alvo(...)` (`f` ou `prefixo.f`).
fn funcao_chamada(e: &FnEmitter, alvo: ExprId) -> Option<FunctionElementId> {
    match &e.expr(alvo).kind {
        ExprKind::Identifier(id) => match e.alvo_do_identificador(id.sym, alvo) {
            crate::expr::IdentTarget::Element(Element::Function(f)) => Some(f),
            _ => None,
        },
        ExprKind::Property { target, name, .. } => {
            let ExprKind::Identifier(id) = &e.expr(*target).kind else { return None };
            let crate::expr::IdentTarget::Prefix(pf) = e.alvo_do_identificador(id.sym, *target) else { return None };
            match e.ctx.program.lookup_prefixed_na_unidade(e.unit, pf, name.sym)?.getter {
                Some(Element::Function(f)) => Some(f),
                _ => None,
            }
        }
        ExprKind::TypeArguments { target, .. } => funcao_chamada(e, *target),
        _ => None,
    }
}

/// Emite a chamada a um intrínseco. `None`: não é intrínseco (ou a forma não
/// casa) — o chamador emite a chamada comum.
pub fn emitir_intrinseco(e: &mut FnEmitter, fid: FunctionElementId, args: &ast::Arguments, expected: Option<&Ty>) -> Option<(Js, Ty)> {
    let k = intrinseco_de(e.ctx, fid)?;
    let pos: Vec<ExprId> = args.args.iter().filter(|a| a.name.is_none()).map(|a| a.value).collect();
    let ret_declarado = || match e.ctx.fn_ty(fid) {
        Ty::Fn { ret, .. } if !ret.mentions_params() => (*ret).clone(),
        _ => Ty::Dynamic,
    };
    match k {
        Intrinseco::Js => {
            let ty = tipo_do_generico(e, args, expected);
            let tpl_instrucao = pos.get(1).is_some_and(|t| match &e.expr(*t).kind {
                ExprKind::String(lit) => matches!(lit.parts.first(), Some(StringPart::Text(x)) if x.to_string_lossy().trim_start().starts_with("throw ")),
                _ => false,
            });
            let texto = texto_js(e, args)?;
            if tpl_instrucao {
                // Instrução em posição de expressão: numa função-flecha.
                return Some((Js::prim(format!("(() => {{ {texto}; }})()")), ty));
            }
            // Entre parênteses: o *template* é JS livre, e nem todo ponto do
            // emissor confere a precedência (`!JS('', '# || #', a, b)`).
            let simples = texto.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'$' | b'.'));
            Some((if simples { Js::prim(texto) } else { Js::prim(format!("({texto})")) }, ty))
        }
        Intrinseco::GetFlag => {
            let nome = string_constante(e, *pos.first()?)?;
            let v = ModoSdk::flag(&nome)?;
            Some((Js::prim(if v { "true" } else { "false" }), e.ctx.t_bool()))
        }
        Intrinseco::GetName => {
            let n = nome_de_enum(e, *pos.first()?)?;
            let js = match n.as_str() {
                "OPERATOR_IS_PREFIX" => "\"$is_\"".to_string(),
                "SIGNATURE_NAME" => "dart._functionRti".to_string(),
                "RTI_NAME" => "\"$ti\"".to_string(),
                "FUTURE_CLASS_TYPE_NAME" => "\"async|Future\"".to_string(),
                "LIST_CLASS_TYPE_NAME" => "\"core|List\"".to_string(),
                "RTI_FIELD_AS" | "RTI_FIELD_IS" => {
                    let rti = e.ctx.sdk.as_ref()?.rti?;
                    e.private_sym(rti, if n == "RTI_FIELD_AS" { "_as" } else { "_is" })
                }
                _ => return None,
            };
            Some((Js::prim(js), ret_declarado()))
        }
        Intrinseco::EmbeddedGlobal => {
            let nome = string_constante(e, *pos.get(1)?)?;
            let js = if nome == "arrayRti" { e.dartx("arrayRti") } else { format!("dart{}", js::prop_access(&nome)) };
            Some((Js::prim(js), tipo_do_generico(e, args, expected)))
        }
        Intrinseco::ClassRef => {
            let x = *pos.first()?;
            if let ExprKind::Identifier(id) = &e.expr(x).kind {
                if e.name(id.sym) == "Null" {
                    return Some((Js::prim("core.Null"), ret_declarado()));
                }
            }
            let c = classe_do_literal(e, x)?;
            Some((Js::prim(e.class_ref(c)), ret_declarado()))
        }
        Intrinseco::TypeRef | Intrinseco::LegacyTypeRef => {
            let t = e.resolve_type(*args.type_args.first()?);
            let mut rti = e.rti(&t);
            if k == Intrinseco::LegacyTypeRef {
                // `T*`: a receita fechada ganha a estrela.
                let pre = "dart_rti._Universe.eval(dart_rti._theUniverse(), \"";
                if let Some(r) = rti.strip_prefix(pre).and_then(|r| r.strip_suffix("\", true)")) {
                    rti = format!("{pre}{r}*\", true)");
                }
            }
            Some((Js::prim(rti), ret_declarado()))
        }
        Intrinseco::RawFunctionRef => {
            let js = arg_estrangeiro(e, *pos.first()?);
            Some((js, ret_declarado()))
        }
        Intrinseco::StringConcat => {
            let a = arg_estrangeiro(e, *pos.first()?);
            let b = arg_estrangeiro(e, *pos.get(1)?);
            Some((Js::new(format!("{} + {}", a.at(P_ADD), b.at(P_ADD + 1)), P_ADD), e.ctx.t_string()))
        }
        Intrinseco::Builtin => {
            let n = nome_de_enum(e, *pos.get(1)?)?;
            let js = match n.as_str() {
                "dartClosureConstructor" => "Function",
                "dartObjectConstructor" => "core.Object",
                _ => return None,
            };
            Some((Js::prim(js), tipo_do_generico(e, args, expected)))
        }
        Intrinseco::RawException => {
            let v = e.rethrow_var.last().cloned()?;
            Some((Js::prim(v), Ty::Dynamic))
        }
        Intrinseco::RtiParameter => Some((Js::prim("_ti"), Ty::Dynamic)),
        Intrinseco::RuntimeLibrary => Some((Js::prim("dart"), Ty::Dynamic)),
        Intrinseco::GetInterceptor => {
            let a = arg_estrangeiro(e, *pos.first()?);
            Some((Js::prim(format!("dart.getInterceptorForRti({})", a.code)), Ty::Dynamic))
        }
        Intrinseco::ExtensionSymbol => {
            let x = *pos.first()?;
            let ExprKind::String(lit) = &e.expr(x).kind else { return None };
            let n = lit.constant_value()?.to_string_lossy();
            Some((Js::prim(e.dartx(&js_member_name(&n))), Ty::Dynamic))
        }
        Intrinseco::InstanceOf => {
            let c = classe_do_literal(e, *pos.get(1)?)?;
            if !e.ctx.class_params[c.0 as usize].is_empty() {
                return None;
            }
            let a = arg_estrangeiro(e, *pos.first()?);
            Some((Js::new(format!("{} instanceof {}", a.at(P_REL), e.class_ref(c)), P_REL), e.ctx.t_bool()))
        }
        Intrinseco::GetPrototypeOf => {
            let a = arg_estrangeiro(e, *pos.first()?);
            Some((Js::prim(format!("Object.getPrototypeOf({})", a.code)), Ty::Dynamic))
        }
        Intrinseco::SetPrototypeOf => {
            let a = arg_estrangeiro(e, *pos.first()?);
            let b = arg_estrangeiro(e, *pos.get(1)?);
            Some((Js::prim(format!("Object.setPrototypeOf({}, {})", a.code, b.code)), Ty::Dynamic))
        }
        Intrinseco::GlobalContext => Some((Js::prim("dart.global"), Ty::Dynamic)),
        Intrinseco::UnsafeCast => {
            let ty = tipo_do_generico(e, args, expected);
            let (js, _) = e.emit_expr(*pos.first()?, None);
            Some((js, ty))
        }
        Intrinseco::ExtractTypeArguments => {
            let t = e.resolve_type(*args.type_args.first()?);
            let Ty::Iface { class, .. } = t else { return None };
            let cname = e.ctx.class_name(class).to_string();
            let params: Vec<String> = e.ctx.class_params[class.0 as usize].iter().map(|p| p.name.clone()).collect();
            let (inst, _) = e.emit_expr(*pos.first()?, None);
            let (f, _) = e.emit_expr(*pos.get(1)?, None);
            let tmp = e.temp();
            let extraidos: Vec<String> = params.iter().map(|p| format!("dart_rti.evalInInstance({tmp}, {})", js::string_literal(&format!("{cname}.{p}")))).collect();
            Some((Js::prim(format!("({tmp} = {}, dart.dgcall({}, [{}], []))", inst.code, f.code, extraidos.join(", "))), Ty::Dynamic))
        }
        Intrinseco::Spread => {
            let a = arg_estrangeiro(e, *pos.first()?);
            Some((Js::prim(format!("...{}", a.at(P_PRIMARY))), Ty::Dynamic))
        }
    }
}

// --------------------------------------------------------------------- classes nativas

/// Chave de declaração de um membro de instância público de classe nativa:
/// o símbolo `dartx` (`compiler.dart:2766-2773`).
pub fn chave_nativa(m: &ModState, nome: &str) -> String {
    format!("[{}]", m.dartx(&js_member_name(nome)))
}

/// Corpo de um membro `external` (ou `native`) de instância de classe nativa
/// (`_emitNativeFunctionBody`, `compiler.dart:2322-2343`): a propriedade JS
/// de mesmo nome (ou a do `@JSName`). Em biblioteca web, retorno não anulável
/// passa por `dart.checkNativeNonNull`.
pub fn membro_externo_nativo(ctx: &Ctx, head: &str, fid: FunctionElementId, nome_dart: &str, kind: ast::FunctionKind) -> String {
    let f = ctx.program.function(fid);
    let jsn = ctx.js_names.get(&fid.0).cloned().unwrap_or_else(|| nome_dart.to_string());
    let acesso = format!("this{}", js::prop_access(&jsn));
    let web = ctx.is_web_library(f.library);
    let ret = ctx.ty_of(ctx.outline.functions[fid.0 as usize].return_type);
    let checar = |x: String| if web && !ret.is_nullable() && !matches!(ret, Ty::Void | Ty::Dynamic) { format!("dart.checkNativeNonNull({x})") } else { x };
    match kind {
        ast::FunctionKind::Getter => format!("{head}() {{\n  return {};\n}}", checar(acesso)),
        ast::FunctionKind::Setter => format!("{head}(v) {{\n  {acesso} = v;\n}}"),
        _ => format!("{head}(...args) {{\n  return {};\n}}", checar(format!("{acesso}.apply(this, args)"))),
    }
}

/// Linhas que fecham a declaração de uma classe do SDK (`compiler.dart:1108-1117`):
/// `Object` instala a igualdade por identidade; cada *peer* nativo é registrado.
pub fn epilogo_de_classe(ctx: &Ctx, c: ClassId, cref: &str) -> String {
    let Some(s) = ctx.sdk.as_ref() else { return String::new() };
    let mut out = String::new();
    if Some(c) == ctx.object {
        out.push_str("dart._installIdentityEquals();\n");
        return out;
    }
    if let Some(tags) = s.pares.get(&c) {
        if !tags.is_empty() && s.primitivas.contains(&c) {
            out.push_str(&format!("dart.definePrimitiveHashCode({cref}.prototype);\n"));
        }
        for t in tags {
            out.push_str(&format!("dart.registerExtension({}, {cref});\n", js::string_literal(t)));
        }
    }
    out
}

/// Corpo extra de classes especiais (`compiler.dart:2163-2183`).
pub fn construtor_especial(ctx: &Ctx, c: ClassId) -> Option<&'static str> {
    ctx.sdk.as_ref()?;
    if Some(c) == ctx.object {
        return Some("constructor() {\n  throw Error(\"use `new \" + dart.typeName(dart.getReifiedType(this)) + \".new(...)` to create a Dart object\");\n}");
    }
    if Some(c) == ctx.js_array {
        return Some("constructor() {\n  return [];\n}");
    }
    None
}

// --------------------------------------------------------------------- o módulo do SDK

/// Bibliotecas do SDK com fonte, na ordem do programa.
pub fn bibliotecas_do_sdk(ctx: &Ctx) -> Vec<LibraryId> {
    (0..ctx.program.libraries.len() as u32)
        .map(LibraryId)
        .filter(|l| ctx.libs[l.0 as usize].is_sdk && !ctx.program.library(*l).units.is_empty())
        .collect()
}

/// O *bootstrap* (`compiler.dart:7934-7989`): o objeto `dart`, um namespace
/// por biblioteca, o `dartx`, o `dart.privateName` e os símbolos `dartx`
/// usados por qualquer módulo do arquivo — **um** `Symbol()` por nome.
pub fn bootstrap(ctx: &Ctx, dartx: &BTreeSet<String>, campos_tardios: bool) -> String {
    let mut out = String::new();
    out.push_str("const _library = Object.create(null);\nconst dart = Object.create(_library);\ndart.library = _library;\n");
    for l in bibliotecas_do_sdk(ctx) {
        let v = &ctx.libs[l.0 as usize].js_var;
        if v != "dart" {
            out.push_str(&format!("var {v} = Object.create(dart.library);\n"));
        }
    }
    out.push_str("var dartx = Object.create(dart.library);\n");
    out.push_str(
        "const _privateNames = Symbol(\"_privateNames\");\ndart.privateName = function(library, name) {\n  let names = library[_privateNames];\n  if (names == null) names = library[_privateNames] = new Map();\n  let symbol = names.get(name);\n  if (symbol == null) names.set(name, symbol = Symbol(name));\n  return symbol;\n};\n",
    );
    if campos_tardios {
        // `late` sem inicializador (`module.rs`, `tardios`): o mesmo par
        // `get`/`set` que a classe escreveria, definido uma vez por campo.
        out.push_str("dart.lateField = function(p, k, s, n, f) {
  Object.defineProperty(p, k, {
    get() { let t = this[s]; return t == null ? dart.throw(new _internal.LateError.fieldNI(n)) : t; },
    set: f ? function(v) { if (this[s] != null) dart.throw(new _internal.LateError.fieldAI(n)); this[s] = v; } : function(v) { this[s] = v; },
    configurable: true
  });
};
");
    }
    for n in dartx {
        let chave = if js::is_js_ident(n) { format!(".{n}") } else { format!("[{}]", js::string_literal(n)) };
        out.push_str(&format!("dartx{chave} = Symbol({});\n", js::string_literal(&format!("dartx.{n}"))));
    }
    out
}

/// Campo de topo do runtime emitido no lugar (`compiler.dart:2635-2681`):
/// inicializador nulo, literal, `JS()` ou construção de classe do runtime.
fn campo_ansioso(e: &FnEmitter, init: Option<ExprId>) -> bool {
    let Some(x) = init else { return true };
    match &e.expr(x).kind {
        ExprKind::Null | ExprKind::Bool(_) | ExprKind::Int(_) | ExprKind::Double(_) | ExprKind::String(_) => true,
        ExprKind::Call { target, .. } => funcao_chamada(e, *target).is_some_and(|f| intrinseco_de(e.ctx, f) == Some(Intrinseco::Js)),
        _ => false,
    }
}

/// O módulo do SDK: tudo o que o filtro deixar vivo das bibliotecas `dart:`,
/// na ordem que o carregamento exige (`docs/JS-PRODUCAO-SDK-PROPRIO.md` §5.6):
/// funções e campos ansiosos do runtime → classes (superclasse antes) →
/// resto de cada biblioteca → regras rti. Devolve o texto e os nomes `dartx`
/// que ele usa (para o *bootstrap*).
pub fn emitir_modulo_sdk(ctx: &Ctx) -> (String, BTreeSet<String>) {
    let libs = bibliotecas_do_sdk(ctx);
    let Some(&primeira) = libs.first() else { return (String::new(), BTreeSet::new()) };
    let mut m = ModState::new(primeira);
    m.group = libs.clone();
    let runtime = ctx.sdk.as_ref().and_then(|s| s.runtime);
    let mut corpo = crate::js::Writer::default();

    // 1. O runtime: o universo de tipos, as funções e os campos ansiosos.
    let mut preguicosos_do_runtime: Vec<VariableId> = Vec::new();
    if let Some(rt) = runtime {
        corpo.line("dart.typeUniverse = {eC: new Map(), tR: {}, eT: {}, tPV: {}, sEA: []};");
        let mut acessores: Vec<(String, String)> = Vec::new();
        for (i, f) in ctx.program.functions.iter().enumerate() {
            let fid = FunctionElementId(i as u32);
            if f.library != rt || f.class.is_some() || f.extension.is_some() || f.kind == FunctionKind::ImplicitAccessor {
                continue;
            }
            if ctx.estado_fn(fid) == crate::filtro::Estado::Morta || externa_sem_corpo(ctx, fid) {
                continue;
            }
            crate::module::emit_top_function(ctx, &m, fid, &mut corpo, &mut acessores);
        }
        if !acessores.is_empty() {
            corpo.line("dart.copyProperties(dart, {");
            let textos: Vec<String> = acessores.iter().map(|(_, t)| crate::module::indent(t)).collect();
            corpo.push_raw(&textos.join(",\n"));
            corpo.push_raw("\n});\n");
        }
        for (i, v) in ctx.program.variables.iter().enumerate() {
            let vid = VariableId(i as u32);
            if v.library != rt || v.class.is_some() || v.extension.is_some() || ctx.estado_var(vid) == crate::filtro::Estado::Morta {
                continue;
            }
            let VariableRef::TopLevel { unit, decl, index } = v.node else { continue };
            let d = ctx.program.unit(unit).ast.decl(decl);
            let ast::DeclKind::Variables(list) = &d.kind else { continue };
            let var = &list.variables[index];
            let mut e = FnEmitter::new(ctx, &m, unit, None, true);
            if v.late || !campo_ansioso(&e, var.initializer) {
                preguicosos_do_runtime.push(vid);
                continue;
            }
            let nome = nome_de_topo_var(ctx, vid).map(str::to_string).unwrap_or_else(|| ctx.name(v.name).to_string());
            let ty = ctx.var_ty(vid);
            let valor = match var.initializer {
                Some(x) => {
                    let (js, _) = e.emit_expr(x, Some(&ty));
                    let pre = crate::module::finish_body(&mut e);
                    corpo.push_raw(&pre);
                    js.code
                }
                None => "null".to_string(),
            };
            corpo.line(&format!("dart{} = {valor};", js::prop_access(&nome)));
        }
    }

    // 1b. Os campos preguiçosos do runtime.
    if let Some(rt) = runtime {
        if !preguicosos_do_runtime.is_empty() {
            crate::module::emit_top_variables(ctx, &m, rt, &preguicosos_do_runtime, &mut corpo);
        }
    }

    // 2. Funções e variáveis de topo de todas as bibliotecas (só definições:
    // as variáveis são preguiçosas): a definição de uma classe executa código
    // (`addRtiResources`, `registerExtension`) que pode lê-las.
    for &l in &libs {
        if Some(l) != runtime {
            crate::module::emit_library_rest_partes(ctx, &m, l, &mut corpo, true, true);
        }
    }

    // 3. Classes de todas as bibliotecas, superclasse antes.
    let mut classes: Vec<ClassId> = Vec::new();
    for (i, c) in ctx.program.classes.iter().enumerate() {
        let cid = ClassId(i as u32);
        if libs.contains(&c.library) && c.decl.is_some() && ctx.nivel_classe(cid) != crate::filtro::Nivel::Morta {
            classes.push(cid);
        }
    }
    // `core.Object` primeiro: todas as outras a estendem, inclusive as do runtime.
    let ordenadas = crate::module::order_classes(ctx, &classes);
    let rastro = std::env::var("DARTFORGE_SDK_RASTRO").is_ok();
    for c in ordenadas {
        if rastro {
            eprintln!("[sdk] classe {}::{}", ctx.program.library(ctx.program.class(c).library).uri, ctx.class_name(c));
        }
        m.note_class(c);
        crate::module::emit_class(ctx, &m, c, &mut corpo);
    }

    // 5. Regras rti das classes do SDK.
    let regras = crate::module::emit_rules_com(ctx, &m, true);
    corpo.push_raw(&regras);

    // Prelúdio do módulo: aliases `dartx`, nomes privados, os símbolos do rti
    // e do async que o emissor cita sem qualificar, caches de constantes.
    let mut out = String::new();
    let dartx_usados: BTreeSet<String> = m.dartx_used.borrow().values().cloned().collect();
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
    for &l in &libs {
        let lvar = &ctx.libs[l.0 as usize].js_var;
        out.push_str(&format!("{lvar}.$constCache = new Map();\n{lvar}.$C = function(k, f) {{ let v = {lvar}.$constCache.get(k); if (v === void 0) {{ v = f(); {lvar}.$constCache.set(k, v); }} return v; }};\n"));
    }
    out.push_str(&corpo.out);
    out.push_str("dart_rti.findType(\"core|Object*\");\ndart_rti.findType(\"core|Object?\");\ndart_rti.findType(\"0&*\");\n");
    (out, dartx_usados)
}

/// `external` sem corpo nem *patch*: nunca emitida (`compiler.dart:3096-3099`).
pub fn externa_sem_corpo(ctx: &Ctx, fid: FunctionElementId) -> bool {
    let f = ctx.program.function(fid);
    if !f.external {
        return false;
    }
    match f.node {
        FunctionRef::Function { unit, function } => matches!(ctx.program.unit(unit).ast.function(function).body, ast::FunctionBody::Empty),
        _ => true,
    }
}
