//! O décimo sétimo lote de regras de lint (docs/ANALYZER-ESPECIFICACAO-INFRA.md
//! §8), escritas direto dos emissores da 3.6.2
//! (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//!
//! * `library_private_types_in_public_api` (`recommended`): o `Validator` do
//!   emissor nas declarações públicas de uma unidade cujo arquivo não é
//!   privado; o elemento de um tipo nomeado é o do nome escrito (parâmetro de
//!   tipo em escopo, ou o da unidade), e o tipo de registro não é visitado.
//! * `package_prefixed_library_names`: o emissor da 3.6.2 não relata nada
//!   (o `visitLibraryDirective` volta logo).
//! * `avoid_js_rounded_ints`: o literal inteiro que o `double` não guarda.
//! * `cancel_subscriptions` e `close_sinks`: o `LeakDetectorProcessors`
//!   (campos na unidade, locais no corpo da função) com os usos válidos do
//!   `_ValidUseVisitor`.
//! * `erase_dart_type_extension_types`: `is` com o `DartType` do
//!   `kernel.ast`.
//! * `conditional_uri_does_not_exist`: a URI de cada configuração que não
//!   aponta para um arquivo existente (relativa, `package:` pelo
//!   `package_config.json` do pacote, `dart:` pela lista do
//!   `libraries.dart` do SDK 3.6.2).
//! * `missing_code_block_language_in_doc_comment`: o bloco cercado sem
//!   linguagem (a linha de abertura).
//! * `prefer_mixin`: o `with` de uma classe que não é `mixin class`.
//! * `avoid_catching_errors`: o `on` que é ou implementa `Error`.
//! * `avoid_web_libraries_in_flutter`: o import de `dart:html`, `dart:js` ou
//!   `dart:js_util` num pacote que depende do `flutter` (e não é plugin web).
//! * `cast_nullable_to_non_nullable`: `as` de anulável para não anulável.
//!
//! Escrito sem compilar nem executar (2026-10-05).

use super::codigos_g as c;
use super::regras::RelatoDeLint;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, ClassKind, VariableId, VariableRef};
use dartforge_frontend::ast::{
    Ast, DeclKind, DirectiveKind, ExprId, ExprKind, FunctionBody, Initializer, MemberKind, Parameter, ParameterKind, StmtKind, TypeId, TypeKind,
    TypeParameter, TypedefKind, UnaryOp,
};
use dartforge_intern::{Interner, SymbolId};
use dartforge_types::resolved::{MemberRef, Resolved};
use dartforge_types::table::{Type, TypeTable};

/// As bibliotecas do `libraries.dart` do SDK 3.6.2.
const BIBLIOTECAS_DO_SDK: [&str; 46] = [
    "async", "collection", "concurrent", "convert", "core", "developer", "ffi", "html", "html_common", "indexed_db", "_http", "io", "isolate", "js",
    "_js", "js_interop", "js_interop_unsafe", "js_util", "math", "mirrors", "nativewrappers", "typed_data", "_native_typed_data", "cli", "svg",
    "web_audio", "web_gl", "_internal", "_js_helper", "_late_helper", "_rti", "_interceptors", "_foreign_helper", "_js_names", "_js_primitives",
    "_js_embedded_names", "_js_shared_embedded_names", "_js_types", "_async_status_codes", "_invocation_mirror_constants", "_recipe_syntax",
    "_load_library_priority", "_metadata", "_js_annotations", "_wasm", "_macros",
];

fn privado(s: &str) -> bool {
    s.starts_with('_')
}

fn anulavel(table: &TypeTable, t: dartforge_types::table::TypeId) -> bool {
    match table.get(t) {
        Type::Dynamic | Type::Void | Type::Null => true,
        Type::Intersection { bound, .. } => anulavel(table, *bound),
        Type::Interface { nullable, .. } | Type::TypeParameter { nullable, .. } | Type::ExtensionType { nullable, .. } => *nullable,
        Type::Function { nullable, .. } | Type::Record { nullable, .. } => *nullable,
        Type::FutureOr { arg, nullable } => *nullable || anulavel(table, *arg),
        Type::Never => false,
    }
}

fn nao_anulavel(table: &TypeTable, t: dartforge_types::table::TypeId) -> bool {
    match table.get(t) {
        Type::Dynamic | Type::Void | Type::Null => false,
        Type::Intersection { bound, .. } => nao_anulavel(table, *bound),
        Type::TypeParameter { param, nullable } => !*nullable && nao_anulavel(table, table.param(*param).bound),
        Type::FutureOr { arg, nullable } => !*nullable && nao_anulavel(table, *arg),
        Type::Interface { nullable, .. } | Type::ExtensionType { nullable, .. } => !*nullable,
        Type::Function { nullable, .. } | Type::Record { nullable, .. } => !*nullable,
        Type::Never => true,
    }
}

fn nome_da_biblioteca(s: &super::Semantica<'_>, interner: &Interner, l: dartforge_elements::model::LibraryId) -> Option<String> {
    s.program.library(l).name.as_ref().map(|n| n.iter().map(|x| interner.resolve(*x)).collect::<Vec<_>>().join("."))
}

/// `implementsInterface(nome, biblioteca)`.
fn implementa(s: &super::Semantica<'_>, interner: &Interner, t: dartforge_types::table::TypeId, nome: &str, biblioteca: &str) -> bool {
    let (Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. }) = s.table.get(t) else { return false };
    let e = |k: ClassId| {
        let x = s.program.class(k);
        interner.resolve(x.name) == nome && nome_da_biblioteca(s, interner, x.library).as_deref() == Some(biblioteca)
    };
    e(*class) || (s.program.class(*class).decl.is_some() && s.outline.hierarchy.get(*class).is_some_and(|d| d.supertypes.keys().any(|k| e(*k))))
}

/// O `Validator` do `library_private_types_in_public_api`.
struct Validador<'a, 'b> {
    a: &'a Ast,
    s: &'a super::Semantica<'b>,
    interner: &'a Interner,
    /// Os parâmetros de tipo em escopo (nomes).
    tps: Vec<SymbolId>,
    achados: Vec<Span>,
}

impl Validador<'_, '_> {
    fn nome(&self, s: SymbolId) -> &str {
        self.interner.resolve(s)
    }

    /// `visitNamedType` (e os outros tipos).
    fn tipo(&mut self, t: TypeId) {
        let a = self.a;
        match &a.ty(t).kind {
            TypeKind::Named { name, args } => {
                let ultimo = name[name.len() - 1];
                let existe = match &name[..] {
                    [n] => self.tps.contains(&n.sym) || self.s.program.lookup_na_unidade(self.s.unidade, n.sym).is_some(),
                    [p, n] => self.s.program.lookup_prefixed_na_unidade(self.s.unidade, p.sym, n.sym).is_some(),
                    _ => false,
                };
                if existe && privado(self.nome(ultimo.sym)) {
                    self.achados.push(ultimo.span);
                }
                for x in args.iter() {
                    self.tipo(*x);
                }
            }
            TypeKind::Function { return_type, type_params, parameters, .. } => {
                let antes = self.tps.len();
                self.tps.extend(type_params.iter().map(|p| p.name.sym));
                if let Some(r) = return_type {
                    self.tipo(*r);
                }
                self.parametros_de_tipo(type_params);
                self.parametros(parameters);
                self.tps.truncate(antes);
            }
            _ => {}
        }
    }

    fn parametros_de_tipo(&mut self, tps: &[TypeParameter]) {
        for p in tps {
            if let Some(b) = p.bound {
                self.tipo(b);
            }
        }
    }

    /// `visitFormalParameterList` com os parâmetros simples, de campo, de
    /// `super` e de função.
    fn parametros(&mut self, ps: &[Parameter]) {
        for p in ps {
            let nomeado_privado = p.kind == ParameterKind::Named && p.name.is_some_and(|n| privado(self.nome(n.sym)));
            if nomeado_privado {
                continue;
            }
            if let Some(internos) = &p.function_parameters {
                // `FunctionTypedFormalParameter`.
                let antes = self.tps.len();
                self.tps.extend(p.function_type_params.iter().map(|t| t.name.sym));
                if let Some(r) = p.ty {
                    self.tipo(r);
                }
                self.parametros_de_tipo(&p.function_type_params);
                self.parametros(internos);
                self.tps.truncate(antes);
                continue;
            }
            match p.ty {
                Some(t) => self.tipo(t),
                None if p.this_ || p.super_ => {
                    // O tipo do elemento: o do campo (ou do parâmetro do
                    // construtor da superclasse), de interface privada.
                    if let Some(n) = p.name
                        && self.tipo_implicito_privado(p, n.sym)
                    {
                        self.achados.push(n.span);
                    }
                }
                None => {}
            }
        }
    }

    /// O tipo do elemento de um `this.x`/`super.x` sem tipo escrito é de
    /// interface com nome privado: o do campo, ou o do parâmetro
    /// correspondente do construtor da superclasse (pelo nome, ou pela
    /// posição entre os `super.` posicionais), visto no supertipo.
    fn tipo_implicito_privado(&self, p: &Parameter, nome: SymbolId) -> bool {
        let s = self.s;
        let program = s.program;
        let table = s.table;
        let privada = |t: dartforge_types::table::TypeId| match table.get(t) {
            Type::Interface { class, .. } => privado(self.nome(program.class(*class).name)),
            _ => false,
        };
        if p.this_ {
            let v = program.variables.iter().position(|x| {
                x.name == nome && matches!(x.node, VariableRef::Field { unit, member, .. } if unit == s.unidade && self.mesma_classe(member, p.span))
            });
            let t = v.and_then(|v| s.outline.variables.get(v)).and_then(|d| d.declared_type.or(d.inferred));
            return t.is_some_and(privada);
        }
        // `super.x`: o construtor que tem o parâmetro e a classe dele.
        let a = self.a;
        let Some((decl_idx, k)) = a.decls.iter().enumerate().find_map(|(i, d)| {
            let membros = match &d.kind {
                DeclKind::Class(x) => &x.members,
                DeclKind::Enum(x) => &x.members,
                _ => return None,
            };
            membros.iter().find_map(|m| match &a.member(*m).kind {
                MemberKind::Constructor(k) if k.parameters.iter().any(|q| q.span == p.span) => Some((i, k)),
                _ => None,
            })
        }) else {
            return false;
        };
        let did = dartforge_frontend::ast::DeclId(decl_idx as u32);
        let Some(classe) = (0..program.classes.len()).map(|i| ClassId(i as u32)).find(|c| program.class(*c).decl.is_some_and(|r| r.unit == s.unidade && r.decl == did)) else {
            return false;
        };
        let Some(sup) = program.class(classe).supertype_class else { return false };
        let nome_do_super = k.initializers.iter().find_map(|i| match i {
            Initializer::Super { constructor, .. } => Some(*constructor),
            _ => None,
        });
        let chave = match nome_do_super.flatten() {
            Some(n) => Some(n.sym),
            None => self.interner.lookup(""),
        };
        let Some(&ksup) = chave.and_then(|ch| program.class(sup).constructors.get(&ch)) else { return false };
        let dados = match s.outline.functions.get(ksup.0 as usize) {
            Some(d) => d,
            None => return false,
        };
        let Type::Function { positional, optional, named, .. } = table.get(dados.signature) else { return false };
        let declarado = if p.kind == ParameterKind::Named {
            named.iter().find(|(n, _, _)| *n == nome).map(|(_, t, _)| *t)
        } else {
            let pos = k.parameters.iter().take_while(|q| q.span != p.span).filter(|q| q.super_ && q.kind != ParameterKind::Named).count();
            positional.iter().chain(optional.iter()).nth(pos).copied()
        };
        let Some(declarado) = declarado else { return false };
        // Um parâmetro de tipo da superclasse vira o argumento do supertipo.
        let visto = match table.get(declarado) {
            Type::TypeParameter { param, .. } => {
                let i = s.outline.classes.get(sup.0 as usize).and_then(|d| d.type_params.iter().position(|q| q == param));
                let molde = s.outline.hierarchy.get(classe).and_then(|d| d.supertypes.get(&sup).copied());
                match (i, molde.map(|m| table.get(m))) {
                    (Some(i), Some(Type::Interface { args, .. })) => args.get(i).copied(),
                    _ => None,
                }
            }
            _ => Some(declarado),
        };
        visto.is_some_and(privada)
    }

    /// O campo `member` é da mesma declaração que contém `pos`.
    fn mesma_classe(&self, member: dartforge_frontend::ast::MemberId, pos: Span) -> bool {
        self.a.decls.iter().any(|d| {
            let membros: &[dartforge_frontend::ast::MemberId] = match &d.kind {
                DeclKind::Class(x) => &x.members,
                DeclKind::Enum(x) => &x.members,
                DeclKind::Mixin(x) => &x.members,
                DeclKind::ExtensionType(x) => &x.members,
                _ => return false,
            };
            membros.contains(&member) && dentro(pos, d.span)
        })
    }

    /// Os membros de uma declaração pública.
    fn membros(&mut self, membros: &[dartforge_frontend::ast::MemberId], dono: Option<&DeclKind>, efetivamente_privado: bool) {
        let a = self.a;
        for &m in membros {
            match &a.member(m).kind {
                MemberKind::Field(l) => {
                    // `isInvalidExtensionTypeField`.
                    if !l.static_ && matches!(dono, Some(DeclKind::ExtensionType(_))) {
                        continue;
                    }
                    if l.variables.iter().any(|v| !privado(self.nome(v.name.sym)))
                        && let Some(t) = l.ty
                    {
                        self.tipo(t);
                    }
                }
                MemberKind::Method(f) => {
                    let f = a.function(*f);
                    let Some(n) = f.name else { continue };
                    if privado(self.nome(n.sym)) {
                        continue;
                    }
                    let antes = self.tps.len();
                    self.tps.extend(f.type_params.iter().map(|t| t.name.sym));
                    if let Some(r) = f.return_type {
                        self.tipo(r);
                    }
                    self.parametros_de_tipo(&f.type_params);
                    if let Some(ps) = &f.parameters {
                        self.parametros(ps);
                    }
                    self.tps.truncate(antes);
                }
                MemberKind::Constructor(k) => {
                    if k.name.is_some_and(|n| privado(self.nome(n.sym))) || matches!(dono, Some(DeclKind::Enum(_))) || efetivamente_privado {
                        continue;
                    }
                    self.parametros(&k.parameters);
                }
            }
        }
    }
}

fn dentro(a: Span, b: Span) -> bool {
    a.start >= b.start && a.end <= b.end
}

/// Roda as regras deste lote que estão ligadas (`ligada(nome)`).
pub fn executar(u: Unidade<'_>, interner: &Interner, ligada: &dyn Fn(&str) -> bool, sem: Option<&super::Semantica<'_>>) -> Vec<RelatoDeLint> {
    let mut out: Vec<RelatoDeLint> = Vec::new();
    let a = u.ast;
    let fonte = u.fonte;
    let mut relatar = |codigo: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(RelatoDeLint { codigo, span, args: args.iter().map(|x| x.to_string()).collect() });
    };

    // `package_prefixed_library_names`: o emissor da 3.6.2 volta sem relatar.
    let _ = ligada("package_prefixed_library_names");

    // `avoid_js_rounded_ints`: o `IntegerLiteral.value` (com o `-` do pai
    // unário, como o `AstBuilder` o lê) e o trecho do literal.
    if ligada("avoid_js_rounded_ints") {
        for e in a.exprs.iter() {
            let ExprKind::Int(sp) = &e.kind else { continue };
            let bruto = &fonte[sp.start..sp.end];
            let negativo = bruto.starts_with('-');
            let inicio = if negativo { sp.start + (bruto.len() - bruto[1..].trim_start().len()) } else { sp.start };
            let lexema: String = fonte[inicio..sp.end].chars().filter(|c| *c != '_').collect();
            let valor: Option<i64> = match lexema.strip_prefix("0x").or_else(|| lexema.strip_prefix("0X")) {
                Some(h) => u64::from_str_radix(h, 16).ok().map(|v| if negativo { (v as i64).wrapping_neg() } else { v as i64 }),
                None => {
                    if negativo {
                        format!("-{lexema}").parse::<i64>().ok()
                    } else {
                        lexema.parse::<i64>().ok()
                    }
                }
            };
            if let Some(v) = valor
                && ((v as f64) as i64) != v
            {
                relatar(&c::AVOID_JS_ROUNDED_INTS, Span { start: inicio, end: sp.end }, &[]);
            }
        }
    }

    // `missing_code_block_language_in_doc_comment`.
    if ligada("missing_code_block_language_in_doc_comment") {
        let comentarios = dartforge_frontend::comentarios::Comentarios::de(fonte);
        for doc in super::regras14::docs_da_unidade(u, &comentarios) {
            let linhas = super::regras14::linhas_do_doc(fonte, &doc);
            for b in super::regras14::blocos_de_codigo(&linhas) {
                if b.cercado && !b.tem_info {
                    let (pos, tam) = b.linhas[0];
                    relatar(&c::MISSING_CODE_BLOCK_LANGUAGE_IN_DOC_COMMENT, Span { start: pos, end: pos + tam }, &[]);
                }
            }
        }
    }

    let Some(s) = sem else { return out };
    let program = s.program;
    let table = s.table;

    // `library_private_types_in_public_api`.
    if ligada("library_private_types_in_public_api") {
        let arquivo_privado = program.unit(s.unidade).path.as_ref().and_then(|p| p.file_name()).is_some_and(|n| n.to_string_lossy().starts_with('_'));
        if !arquivo_privado {
            let mut v = Validador { a, s, interner, tps: Vec::new(), achados: Vec::new() };
            for &did in &u.unit.declarations {
                let d = a.decl(did);
                match &d.kind {
                    DeclKind::Class(x) => {
                        if privado(interner.resolve(x.name.sym)) {
                            continue;
                        }
                        v.tps = x.type_params.iter().map(|t| t.name.sym).collect();
                        if x.mixin_application {
                            // `ClassTypeAlias`: a superclasse e os parâmetros de tipo.
                            if let Some(sup) = x.extends {
                                v.tipo(sup);
                            }
                            v.parametros_de_tipo(&x.type_params);
                            continue;
                        }
                        v.parametros_de_tipo(&x.type_params);
                        // `isEffectivelyPrivate` do construtor: `@internal`,
                        // `sealed`, `abstract final`, `abstract interface`.
                        let interno = d.metadata.iter().any(|m| dartforge_types::anotacoes::e_getter_de(program, interner, s.unidade, m, "meta", "internal"));
                        let m = &x.modifiers;
                        let efetivamente = interno || m.sealed || (m.abstract_ && (m.final_ || m.interface));
                        v.membros(&x.members, Some(&d.kind), efetivamente);
                    }
                    DeclKind::Enum(x) => {
                        if privado(interner.resolve(x.name.sym)) {
                            continue;
                        }
                        v.tps = x.type_params.iter().map(|t| t.name.sym).collect();
                        v.parametros_de_tipo(&x.type_params);
                        v.membros(&x.members, Some(&d.kind), false);
                    }
                    DeclKind::Extension(x) => {
                        if x.name.is_none_or(|n| privado(interner.resolve(n.sym))) {
                            continue;
                        }
                        v.tps = x.type_params.iter().map(|t| t.name.sym).collect();
                        v.parametros_de_tipo(&x.type_params);
                        v.tipo(x.on);
                        let interno = d.metadata.iter().any(|m| dartforge_types::anotacoes::e_getter_de(program, interner, s.unidade, m, "meta", "internal"));
                        v.membros(&x.members, Some(&d.kind), interno);
                    }
                    DeclKind::ExtensionType(x) => {
                        if privado(interner.resolve(x.name.sym)) {
                            continue;
                        }
                        v.tps = x.type_params.iter().map(|t| t.name.sym).collect();
                        v.parametros_de_tipo(&x.type_params);
                        if !privado(interner.resolve(x.representation_name.sym)) {
                            v.tipo(x.representation_type);
                        }
                        let interno = d.metadata.iter().any(|m| dartforge_types::anotacoes::e_getter_de(program, interner, s.unidade, m, "meta", "internal"));
                        v.membros(&x.members, Some(&d.kind), interno);
                    }
                    DeclKind::Mixin(x) => {
                        if privado(interner.resolve(x.name.sym)) {
                            continue;
                        }
                        v.tps = x.type_params.iter().map(|t| t.name.sym).collect();
                        for t in x.on.iter() {
                            v.tipo(*t);
                        }
                        v.parametros_de_tipo(&x.type_params);
                        let interno = d.metadata.iter().any(|m| dartforge_types::anotacoes::e_getter_de(program, interner, s.unidade, m, "meta", "internal"));
                        v.membros(&x.members, Some(&d.kind), interno);
                    }
                    DeclKind::Function(f) => {
                        let f = a.function(*f);
                        let Some(n) = f.name else { continue };
                        if privado(interner.resolve(n.sym)) {
                            continue;
                        }
                        v.tps = f.type_params.iter().map(|t| t.name.sym).collect();
                        if let Some(r) = f.return_type {
                            v.tipo(r);
                        }
                        v.parametros_de_tipo(&f.type_params);
                        if let Some(ps) = &f.parameters {
                            v.parametros(ps);
                        }
                    }
                    DeclKind::Typedef(x) => {
                        if privado(interner.resolve(x.name.sym)) {
                            continue;
                        }
                        v.tps = x.type_params.iter().map(|t| t.name.sym).collect();
                        match &x.kind {
                            TypedefKind::Legacy { return_type, parameters } => {
                                if let Some(r) = return_type {
                                    v.tipo(*r);
                                }
                                v.parametros_de_tipo(&x.type_params);
                                v.parametros(parameters);
                            }
                            TypedefKind::Alias(t) => {
                                // `functionType?.accept`: só o alias de tipo de função.
                                v.parametros_de_tipo(&x.type_params);
                                if matches!(a.ty(*t).kind, TypeKind::Function { .. }) {
                                    v.tipo(*t);
                                }
                            }
                        }
                    }
                    DeclKind::Variables(l) => {
                        v.tps.clear();
                        if l.variables.iter().any(|x| !privado(interner.resolve(x.name.sym)))
                            && let Some(t) = l.ty
                        {
                            v.tipo(t);
                        }
                    }
                }
            }
            for sp in v.achados {
                relatar(&c::LIBRARY_PRIVATE_TYPES_IN_PUBLIC_API, sp, &[]);
            }
        }
    }

    // `cancel_subscriptions` e `close_sinks`.
    for (regra, codigo) in [("cancel_subscriptions", &c::CANCEL_SUBSCRIPTIONS), ("close_sinks", &c::CLOSE_SINKS)] {
        if !ligada(regra) {
            continue;
        }
        // Os predicados: (o tipo vale, o método que libera).
        let predicados: Vec<(&str, &str, &str)> = if regra == "cancel_subscriptions" {
            vec![("StreamSubscription", "dart.async", "cancel")]
        } else {
            vec![("Sink", "dart.core", "close"), ("Socket", "dart.io", "destroy")]
        };
        let casa = |t: dartforge_types::table::TypeId, metodo: Option<&str>| {
            predicados.iter().any(|(n, l, m)| metodo.is_none_or(|x| x == *m) && implementa(s, interner, t, n, l))
        };
        for (alvo, conteiner) in candidatos_de_vazamento(s, a) {
            let (nome, init, tipo, campo, local) = match alvo {
                Alvo::Campo { v, nome, init } => {
                    let t = s.outline.variables.get(v.0 as usize).and_then(|d| d.declared_type.or(d.inferred));
                    (nome, init, t, Some(v), None)
                }
                Alvo::Local { nome, init } => (nome, init, s.corpo.tipo_local(nome.span.start), None, Some(nome.span.start)),
            };
            // `variable.equals != null && initializer is SimpleIdentifier`.
            if init.is_some_and(|i| matches!(a.expr(i).kind, ExprKind::Identifier(_))) {
                continue;
            }
            let Some(t) = tipo else { continue };
            if !casa(t, None) {
                continue;
            }
            // O elemento do identificador é a variável (ou o acessor dela).
            let e_a_variavel = |e: ExprId| -> bool {
                match (local, campo) {
                    (Some(pos), _) => matches!(s.corpo.get_resolved(e), Some(Resolved::Local(_))) && s.corpo.declaracao_local(e) == Some(pos),
                    (_, Some(v)) => match s.corpo.get_resolved(e) {
                        Some(Resolved::Member { member: MemberRef::Variable(x), .. }) => *x == v,
                        Some(Resolved::Member { member: MemberRef::Function(f), .. }) => program.function(*f).variable == Some(v),
                        _ => false,
                    },
                    _ => false,
                }
            };
            // O elemento é a própria variável (não o acessor).
            let e_o_elemento = |e: ExprId| -> bool {
                match (local, campo) {
                    (Some(pos), _) => matches!(s.corpo.get_resolved(e), Some(Resolved::Local(_))) && s.corpo.declaracao_local(e) == Some(pos),
                    (_, Some(v)) => matches!(s.corpo.get_resolved(e), Some(Resolved::Member { member: MemberRef::Variable(x), .. }) if *x == v),
                    _ => false,
                }
            };
            let valido = uso_valido(s, a, interner, conteiner, nome, init, campo, local.is_some(), &e_a_variavel, &e_o_elemento, &|m| casa(t, Some(m)));
            if !valido {
                let fim = init.map_or(nome.span.end, |i| a.expr(i).span.end);
                relatar(codigo, Span { start: nome.span.start, end: fim }, &[]);
            }
        }
    }

    // `erase_dart_type_extension_types`.
    if ligada("erase_dart_type_extension_types") {
        for e in a.exprs.iter() {
            let ExprKind::Is { ty, .. } = &e.kind else { continue };
            if let Some(&t) = s.corpo.tipos_de_anotacoes.get(ty)
                && implementa(s, interner, t, "DartType", "kernel.ast")
            {
                relatar(&c::ERASE_DART_TYPE_EXTENSION_TYPES, e.span, &[]);
            }
        }
    }

    // `conditional_uri_does_not_exist`.
    if ligada("conditional_uri_does_not_exist") {
        let arquivo = program.unit(s.unidade).path.clone();
        let base = arquivo.as_ref().and_then(|p| p.parent().map(std::path::Path::to_path_buf));
        let config = arquivo.as_ref().and_then(|p| p.ancestors().skip(1).find(|d| d.join(".dart_tool").join("package_config.json").is_file()).map(|d| d.join(".dart_tool").join("package_config.json")));
        let pacotes = config.and_then(|c| dartforge_elements::PackageConfig::load(&c).ok());
        for d in &u.unit.directives {
            let configuracoes = match &d.kind {
                DirectiveKind::Import { configurations, .. } | DirectiveKind::Export { configurations, .. } => configurations,
                _ => continue,
            };
            for cfg in configuracoes {
                let Some(texto) = cfg.uri.constant_value().map(|v| v.to_string_lossy()) else { continue };
                let existe = if let Some(nome) = texto.strip_prefix("dart:") {
                    let nome = nome.split('/').next().unwrap_or("");
                    !nome.is_empty() && BIBLIOTECAS_DO_SDK.contains(&nome)
                } else if let Some(resto) = texto.strip_prefix("package:") {
                    match (resto.split_once('/'), &pacotes) {
                        (Some((pacote, rel)), Some(pc)) => pc.package_dirs.iter().find(|(n, _)| n == pacote).is_some_and(|(_, dir)| dir.join(rel).is_file()),
                        _ => false,
                    }
                } else if texto.contains(':') {
                    // Outro esquema: a fonte não é de arquivo conhecido.
                    true
                } else {
                    base.as_ref().is_some_and(|b| b.join(&texto).is_file())
                };
                if !existe {
                    relatar(&c::CONDITIONAL_URI_DOES_NOT_EXIST, cfg.uri.span, &[texto.as_str()]);
                }
            }
        }
    }

    // `prefer_mixin`.
    if ligada("prefer_mixin") {
        for d in a.decls.iter() {
            let com: &[TypeId] = match &d.kind {
                DeclKind::Class(x) => &x.with,
                DeclKind::Enum(x) => &x.with,
                _ => continue,
            };
            for &t in com {
                let Some(&tipo) = s.outline.tipos_escritos.get(&(s.unidade, t)) else { continue };
                let Type::Interface { class, .. } = table.get(tipo) else { continue };
                let k = program.class(*class);
                if k.kind == ClassKind::Class && !k.modifiers.mixin {
                    let TypeKind::Named { name, .. } = &a.ty(t).kind else { continue };
                    let nome = name[name.len() - 1];
                    relatar(&c::PREFER_MIXIN, a.ty(t).span, &[interner.resolve(nome.sym)]);
                }
            }
        }
    }

    // `avoid_catching_errors`.
    if ligada("avoid_catching_errors") {
        for st in a.stmts.iter() {
            let StmtKind::Try { catches, .. } = &st.kind else { continue };
            for k in catches.iter() {
                let Some(on) = k.on_type else { continue };
                let Some(&t) = s.corpo.tipos_de_anotacoes.get(&on) else { continue };
                if !implementa(s, interner, t, "Error", "dart.core") {
                    continue;
                }
                let exato = matches!(table.get(t), Type::Interface { class, .. } if interner.resolve(program.class(*class).name) == "Error" && nome_da_biblioteca(s, interner, program.class(*class).library).as_deref() == Some("dart.core"));
                if exato {
                    relatar(&c::AVOID_CATCHING_ERRORS_CLASS, k.span, &[]);
                } else {
                    let texto = dartforge_types::despejo::formatar(table, t, interner, program);
                    relatar(&c::AVOID_CATCHING_ERRORS_SUBCLASS, k.span, &[texto.as_str()]);
                }
            }
        }
    }

    // `avoid_web_libraries_in_flutter`.
    if ligada("avoid_web_libraries_in_flutter") {
        let arquivo = program.library(program.unit(s.unidade).library).units.first().and_then(|x| program.unit(*x).path.clone());
        let pubspec = arquivo.as_ref().and_then(|p| p.ancestors().skip(1).map(|d| d.join("pubspec.yaml")).find(|f| f.is_file()));
        let tem_flutter = pubspec.and_then(|p| std::fs::read_to_string(p).ok()).is_some_and(|t| {
            let Ok(Some(raiz)) = crate::naodart::yaml::ler(&t) else { return false };
            let depende = raiz.campo("dependencies").and_then(|d| d.campo("flutter")).is_some_and(|f| !f.nulo());
            let web = raiz.campo("flutter").and_then(|f| f.campo("plugin")).and_then(|p| p.campo("platforms")).and_then(|p| p.campo("web")).is_some_and(|w| !w.nulo());
            depende && !web
        });
        if tem_flutter {
            for d in &u.unit.directives {
                let DirectiveKind::Import { uri, .. } = &d.kind else { continue };
                let Some(v) = uri.constant_value().map(|v| v.to_string_lossy()) else { continue };
                if matches!(v.as_str(), "dart:html" | "dart:js" | "dart:js_util") {
                    relatar(&c::AVOID_WEB_LIBRARIES_IN_FLUTTER, d.span, &[]);
                }
            }
        }
    }

    // `cast_nullable_to_non_nullable`.
    if ligada("cast_nullable_to_non_nullable") {
        for e in a.exprs.iter() {
            let ExprKind::As { value, ty } = &e.kind else { continue };
            let (Some(te), Some(&tt)) = (s.corpo.get_type(*value), s.corpo.tipos_de_anotacoes.get(ty)) else { continue };
            if matches!(table.get(te), Type::Dynamic) || s.core.is_unknown(table, te) {
                continue;
            }
            if anulavel(table, te) && nao_anulavel(table, tt) {
                relatar(&c::CAST_NULLABLE_TO_NON_NULLABLE, e.span, &[]);
            }
        }
    }

    out
}

/// Uma variável candidata do `LeakDetectorProcessors`.
enum Alvo {
    Campo { v: VariableId, nome: dartforge_frontend::ast::Name, init: Option<ExprId> },
    Local { nome: dartforge_frontend::ast::Name, init: Option<ExprId> },
}

/// Os campos (com a unidade toda por contêiner) e as variáveis locais (com
/// o corpo da função mais próxima).
fn candidatos_de_vazamento(s: &super::Semantica<'_>, a: &Ast) -> Vec<(Alvo, Option<Span>)> {
    let mut v = Vec::new();
    for (k, m) in a.members.iter().enumerate() {
        let MemberKind::Field(l) = &m.kind else { continue };
        for (index, x) in l.variables.iter().enumerate() {
            let r = VariableRef::Field { unit: s.unidade, member: dartforge_frontend::ast::MemberId(k as u32), index };
            if let Some(i) = s.program.variables.iter().position(|y| y.node == r) {
                v.push((Alvo::Campo { v: VariableId(i as u32), nome: x.name, init: x.initializer }, None));
            }
        }
    }
    // Os corpos de função (o `FunctionBody` mais próximo).
    let mut corpos: Vec<Span> = Vec::new();
    for f in a.functions.iter() {
        match &f.body {
            FunctionBody::Block(s2) => corpos.push(a.stmt(*s2).span),
            FunctionBody::Expression(e) => corpos.push(a.expr(*e).span),
            _ => {}
        }
    }
    for m in a.members.iter() {
        if let MemberKind::Constructor(k) = &m.kind
            && let FunctionBody::Block(s2) = &k.body
        {
            corpos.push(a.stmt(*s2).span);
        }
    }
    for st in a.stmts.iter() {
        let StmtKind::Variables(l) = &st.kind else { continue };
        let corpo = corpos.iter().filter(|c| dentro(st.span, **c)).min_by_key(|c| c.end - c.start).copied();
        let Some(corpo) = corpo else { continue };
        for x in l.variables.iter() {
            v.push((Alvo::Local { nome: x.name, init: x.initializer }, Some(corpo)));
        }
    }
    v
}

/// O `_ValidUseVisitor`: algum uso válido no contêiner (`None`: a unidade).
#[allow(clippy::too_many_arguments)]
fn uso_valido(
    s: &super::Semantica<'_>,
    a: &Ast,
    interner: &Interner,
    conteiner: Option<Span>,
    nome: dartforge_frontend::ast::Name,
    init: Option<ExprId>,
    campo: Option<VariableId>,
    e_local: bool,
    e_a_variavel: &dyn Fn(ExprId) -> bool,
    e_o_elemento: &dyn Fn(ExprId) -> bool,
    libera: &dyn Fn(&str) -> bool,
) -> bool {
    let no_conteiner = |sp: Span| conteiner.is_none_or(|c| dentro(sp, c));
    let declaracao = Span { start: nome.span.start, end: init.map_or(nome.span.end, |i| a.expr(i).span.end) };
    // O `realTarget` de seções de cascata.
    let mut cascatas: std::collections::HashMap<ExprId, ExprId> = std::collections::HashMap::new();
    for e in a.exprs.iter() {
        let ExprKind::Cascade { target, sections, .. } = &e.kind else { continue };
        for &sec in sections.iter() {
            let mut x = sec;
            loop {
                let prox = match &a.expr(x).kind {
                    ExprKind::Property { target, .. } | ExprKind::Index { target, .. } | ExprKind::Call { target, .. } | ExprKind::Assign { target, .. } => *target,
                    ExprKind::CascadeTarget => {
                        cascatas.insert(x, *target);
                        break;
                    }
                    _ => break,
                };
                x = prox;
            }
        }
    }
    let real = |x: ExprId| if matches!(a.expr(x).kind, ExprKind::CascadeTarget) { cascatas.get(&x).copied().unwrap_or(x) } else { x };
    for e in a.exprs.iter() {
        if !no_conteiner(e.span) {
            continue;
        }
        match &e.kind {
            ExprKind::Assign { target, value, .. } => {
                if matches!(a.expr(*value).kind, ExprKind::Identifier(_)) {
                    if e_a_variavel(*target) {
                        return true;
                    }
                    if let ExprKind::Property { name, target: t2, .. } = &a.expr(*target).kind
                        && !matches!(a.expr(*t2).kind, ExprKind::Identifier(_))
                        && name.sym == nome.sym
                    {
                        return true;
                    }
                }
            }
            ExprKind::Call { target, arguments } => {
                let ExprKind::Property { target: alvo, name, .. } = &a.expr(*target).kind else { continue };
                let alvo = real(*alvo);
                let metodo = interner.resolve(name.sym);
                if libera(metodo) {
                    let por_identificador = matches!(a.expr(alvo).kind, ExprKind::Identifier(_)) && e_a_variavel(alvo);
                    let por_posfixo = match &a.expr(alvo).kind {
                        ExprKind::Unary { op: UnaryOp::PostfixInc | UnaryOp::PostfixDec | UnaryOp::NullAssert, operand } => {
                            matches!(a.expr(*operand).kind, ExprKind::Identifier(_)) && e_a_variavel(*operand)
                        }
                        _ => false,
                    };
                    let por_this = match &a.expr(alvo).kind {
                        ExprKind::Property { target: t2, .. } => matches!(a.expr(*t2).kind, ExprKind::This) && e_a_variavel(alvo),
                        _ => false,
                    };
                    let na_declaracao = dentro(e.span, declaracao);
                    if por_identificador || por_posfixo || por_this || na_declaracao {
                        return true;
                    }
                }
                // Pela cascata: o alvo identificador cujo elemento é o acessor
                // do campo.
                if campo.is_some()
                    && matches!(a.expr(alvo).kind, ExprKind::Identifier(_))
                    && matches!(s.corpo.get_resolved(alvo), Some(Resolved::Member { member: MemberRef::Function(_), .. }))
                    && e_a_variavel(alvo)
                {
                    return true;
                }
                // Passada como argumento.
                if arguments.args.iter().any(|x| x.name.is_none() && matches!(a.expr(x.value).kind, ExprKind::Identifier(_)) && e_o_elemento(x.value)) {
                    return true;
                }
            }
            // `PrefixedIdentifier`: `x.close` (sem chamar).
            ExprKind::Property { target, name, .. } => {
                if matches!(a.expr(*target).kind, ExprKind::Identifier(_)) && e_o_elemento(*target) && libera(interner.resolve(name.sym)) {
                    return true;
                }
            }
            _ => {}
        }
    }
    // O `return x;` de uma local.
    if e_local {
        for st in a.stmts.iter() {
            if let StmtKind::Return(Some(v)) = &st.kind
                && no_conteiner(st.span)
                && matches!(a.expr(*v).kind, ExprKind::Identifier(_))
                && e_o_elemento(*v)
            {
                return true;
            }
        }
    }
    // Os campos: inicializador de construtor e parâmetro `this.x`.
    if let Some(v) = campo {
        let classe = s.program.variable(v).class;
        for m in a.members.iter() {
            let MemberKind::Constructor(k) = &m.kind else { continue };
            for i in k.initializers.iter() {
                if let Initializer::Field { name, .. } = i
                    && name.sym == nome.sym
                    && classe.is_some()
                {
                    return true;
                }
            }
            if k.parameters.iter().any(|p| p.this_ && p.name.is_some_and(|n| n.sym == nome.sym)) {
                return true;
            }
        }
    }
    false
}
