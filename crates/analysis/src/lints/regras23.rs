//! O vigésimo terceiro lote de regras de lint
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), escritas direto dos emissores
//! da 3.6.2 (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`), pela
//! semântica da unidade:
//!
//! * `comment_references`: os `[this]`/`[null]`/`[true]`/`[false]` de cada
//!   token do comentário, e as referências (`_parseReferences`, fora de
//!   código) que o `CommentReferenceResolver` não resolve (o escopo da
//!   declaração documentada: parâmetros e parâmetros de tipo, membros
//!   declarados da classe, os da biblioteca; e os membros herdados da classe
//!   ou do tipo estendido), fora as definições de link (`[x]: …`).
//!   Desvio conhecido: os comentários de parâmetros, de parâmetros de tipo e
//!   de variáveis locais não são lidos.
//! * `prefer_void_to_null`: o tipo `Null` escrito, fora as posições que o
//!   emissor pula.
//! * `type_annotate_public_apis`: as declarações públicas sem tipo.
//!
//! Escrito sem compilar nem executar (2026-10-05).

use super::codigos_g as c;
use super::regras::RelatoDeLint;
use super::CodigoLint;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, Element, ExtensionId};
use dartforge_frontend::ast::{
    Annotation, Ast, DeclId, DeclKind, ExprKind, ForInit, MemberKind, Parameter, PatternKind, StmtKind, TypeId, TypeKind, TypedefKind,
};
use dartforge_intern::{Interner, SymbolId};
use dartforge_types::table::Type;
use std::collections::HashSet;

fn classe_da_decl(s: &super::Semantica<'_>, d: DeclId) -> Option<ClassId> {
    (0..s.program.classes.len()).map(|i| ClassId(i as u32)).find(|c| s.program.class(*c).decl.is_some_and(|r| r.unit == s.unidade && r.decl == d))
}

/// O dono de um comentário de documentação e o escopo dele.
struct Dono<'a> {
    /// Os nomes declarados pela própria declaração (parâmetros e
    /// parâmetros de tipo).
    proprios: Vec<SymbolId>,
    /// Os membros declarados da classe que envolve (o `InstanceScope`) e os
    /// parâmetros de tipo dela.
    da_classe: Vec<SymbolId>,
    classe: Option<ClassId>,
    extensao: Option<ExtensionId>,
    metadata: &'a [Annotation],
    inicio: usize,
}

/// Os nomes declarados de uma lista de membros (acessores e métodos).
fn nomes_dos_membros(a: &Ast, membros: &[dartforge_frontend::ast::MemberId]) -> Vec<SymbolId> {
    let mut v = Vec::new();
    for &m in membros {
        match &a.member(m).kind {
            MemberKind::Field(l) => v.extend(l.variables.iter().map(|x| x.name.sym)),
            MemberKind::Method(f) => v.extend(a.function(*f).name.map(|n| n.sym)),
            MemberKind::Constructor(_) => {}
        }
    }
    v
}

fn nomes_dos_parametros(ps: &[Parameter]) -> Vec<SymbolId> {
    ps.iter().filter_map(|p| p.name.map(|n| n.sym)).collect()
}

/// O elemento que um nome resolve no escopo do dono.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Elem {
    Nada,
    Prefixo,
    Classe(ClassId),
    Extensao(ExtensionId),
    Outro,
}

/// O membro `nome` (instância, estático ou construtor nomeado) da classe.
fn membro_da_classe(s: &super::Semantica<'_>, interner: &Interner, c: ClassId, nome: &str, herdados: bool) -> bool {
    let program = s.program;
    let Some(sym) = interner.lookup(nome) else { return false };
    let setter = interner.lookup(&format!("{nome}_="));
    let tem = |k: ClassId| {
        let x = program.class(k);
        x.instance_members.contains_key(&sym) || setter.is_some_and(|st| x.instance_members.contains_key(&st)) || x.static_members.contains_key(&sym) || setter.is_some_and(|st| x.static_members.contains_key(&st))
    };
    if tem(c) || program.class(c).constructors.contains_key(&sym) {
        return true;
    }
    if herdados {
        let mut todas: Vec<ClassId> = s.outline.hierarchy.get(c).map(|d| d.supertypes.keys().copied().collect()).unwrap_or_default();
        todas.extend(s.core.object_class);
        return todas.into_iter().any(|k| {
            let x = program.class(k);
            x.instance_members.contains_key(&sym) || setter.is_some_and(|st| x.instance_members.contains_key(&st))
        });
    }
    false
}

fn membro_da_extensao(s: &super::Semantica<'_>, interner: &Interner, e: ExtensionId, nome: &str) -> bool {
    let x = &s.program.extensions[e.0 as usize];
    let Some(sym) = interner.lookup(nome) else { return false };
    let setter = interner.lookup(&format!("{nome}_="));
    x.instance_members.contains_key(&sym) || x.static_members.contains_key(&sym) || setter.is_some_and(|st| x.instance_members.contains_key(&st) || x.static_members.contains_key(&st))
}

/// `_resolveSimpleIdentifier`.
fn resolver_simples(s: &super::Semantica<'_>, interner: &Interner, dono: &Dono<'_>, nome: &str) -> Elem {
    let program = s.program;
    if let Some(sym) = interner.lookup(nome) {
        if dono.proprios.contains(&sym) || dono.da_classe.contains(&sym) {
            return Elem::Outro;
        }
        if program.prefixos_na_unidade(s.unidade).contains_key(&sym) {
            return Elem::Prefixo;
        }
        if let Some(b) = program.lookup_na_unidade(s.unidade, sym)
            && let Some(e) = b.getter.or(b.setter)
        {
            return match e {
                Element::Class(c) => Elem::Classe(c),
                Element::Extension(x) => Elem::Extensao(x),
                Element::Prefix(..) => Elem::Prefixo,
                _ => Elem::Outro,
            };
        }
    }
    // O tipo da classe (ou o estendido) que envolve: os membros, também os
    // herdados.
    if let Some(c) = dono.classe
        && membro_da_classe(s, interner, c, nome, true)
    {
        return Elem::Outro;
    }
    if let Some(e) = dono.extensao {
        let on = s.outline.extensions.get(e.0 as usize).map(|x| x.on);
        let alvo = on.and_then(|t| match s.table.get(t) {
            Type::Interface { class, .. } => Some(*class),
            Type::Function { .. } => s.core.function_class,
            Type::TypeParameter { param, .. } => match s.table.get(s.table.param(*param).bound) {
                Type::Interface { class, .. } => Some(*class),
                _ => None,
            },
            _ => None,
        });
        if alvo.is_some_and(|c| membro_da_classe(s, interner, c, nome, true)) {
            return Elem::Outro;
        }
    }
    Elem::Nada
}

/// A referência resolve.
fn resolve(s: &super::Semantica<'_>, interner: &Interner, dono: &Dono<'_>, partes: &[&str]) -> bool {
    let program = s.program;
    match partes {
        [x] => resolver_simples(s, interner, dono, x) != Elem::Nada,
        [p, x] => match resolver_simples(s, interner, dono, p) {
            Elem::Prefixo => {
                let (Some(ps), Some(xs)) = (interner.lookup(p), interner.lookup(x)) else { return false };
                program.lookup_prefixed_na_unidade(s.unidade, ps, xs).is_some()
            }
            Elem::Classe(c) => membro_da_classe(s, interner, c, x, true),
            Elem::Extensao(e) => membro_da_extensao(s, interner, e, x),
            _ => false,
        },
        [p, k, x] => {
            if resolver_simples(s, interner, dono, p) != Elem::Prefixo {
                return false;
            }
            let (Some(ps), Some(ks)) = (interner.lookup(p), interner.lookup(k)) else { return false };
            match program.lookup_prefixed_na_unidade(s.unidade, ps, ks).and_then(|b| b.getter.or(b.setter)) {
                Some(Element::Class(c)) => membro_da_classe(s, interner, c, x, false),
                Some(Element::Extension(e)) => membro_da_extensao(s, interner, e, x),
                _ => false,
            }
        }
        _ => true,
    }
}

/// As referências de um comentário (`_parseReferences`, fora dos blocos de
/// código e dos trechos entre crases), com as partes e as posições.
fn referencias(fonte: &str, linhas: &[(usize, &str)], em_bloco: &dyn Fn(usize) -> bool) -> Vec<Vec<(String, Span)>> {
    let _ = fonte;
    let mut v = Vec::new();
    for (k, &(base, conteudo)) in linhas.iter().enumerate() {
        if em_bloco(k) {
            continue;
        }
        let b = conteudo.as_bytes();
        let mut i = 0;
        let mut so_brancos = true;
        while i < b.len() {
            match b[i] {
                b'[' => {
                    i += 1;
                    if b.get(i) == Some(&b':') {
                        match conteudo[i + 1..].find(":]") {
                            Some(k) => i = i + 1 + k + 1,
                            None => break,
                        }
                    } else {
                        let ini = i;
                        let Some(k) = conteudo[i..].find(']') else { break };
                        let fim = i + k;
                        let mut j = fim + 1;
                        let link = match b.get(j) {
                            Some(b'(') => true,
                            Some(b':') if so_brancos => true,
                            _ => {
                                while b.get(j).is_some_and(|c| c.is_ascii_whitespace()) {
                                    j += 1;
                                }
                                b.get(j) == Some(&b'[')
                            }
                        };
                        if !link {
                            // As partes com as posições.
                            let mut texto_ini = ini;
                            let mut texto = &conteudo[ini..fim];
                            let corte = texto.len() - texto.trim_start().len();
                            texto_ini += corte;
                            texto = texto.trim();
                            if let Some(r) = texto.strip_prefix("new ") {
                                let c2 = texto.len() - r.trim_start().len();
                                texto_ini += c2;
                                texto = r.trim_start();
                            }
                            let mut partes: Vec<(String, Span)> = Vec::new();
                            let mut pos = texto_ini;
                            let mut ok = true;
                            for p in texto.split('.') {
                                let lead = p.len() - p.trim_start().len();
                                let t = p.trim();
                                let id = !t.is_empty() && t.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$') && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$');
                                if !id {
                                    ok = false;
                                    break;
                                }
                                partes.push((t.to_string(), Span { start: base + pos + lead, end: base + pos + lead + t.len() }));
                                pos += p.len() + 1;
                            }
                            if ok && !partes.is_empty() && partes.len() <= 3 {
                                v.push(partes);
                            }
                        }
                        i = fim;
                    }
                    so_brancos = false;
                }
                b'`' => {
                    if let Some(k) = conteudo[i + 1..].find('`') {
                        i = i + 1 + k;
                    }
                    so_brancos = false;
                }
                c if !c.is_ascii_whitespace() => so_brancos = false,
                _ => {}
            }
            i += 1;
        }
    }
    v
}

/// Roda as regras deste lote que estão ligadas (`ligada(nome)`).
pub fn executar(u: Unidade<'_>, interner: &Interner, ligada: &dyn Fn(&str) -> bool, sem: Option<&super::Semantica<'_>>) -> Vec<RelatoDeLint> {
    let mut out: Vec<RelatoDeLint> = Vec::new();
    let Some(s) = sem else { return out };
    let a = u.ast;
    let fonte = u.fonte;
    let program = s.program;
    let table = s.table;
    let mut relatar = |codigo: &'static CodigoLint, span: Span, args: &[&str]| {
        out.push(RelatoDeLint { codigo, span, args: args.iter().map(|x| x.to_string()).collect() });
    };

    // `comment_references`.
    if ligada("comment_references") {
        let comentarios = dartforge_frontend::comentarios::Comentarios::de(fonte);
        // Os donos: diretivas, declarações de topo, membros, constantes.
        let mut donos: Vec<Dono<'_>> = Vec::new();
        for d in &u.unit.directives {
            donos.push(Dono { proprios: Vec::new(), da_classe: Vec::new(), classe: None, extensao: None, metadata: &d.metadata, inicio: d.span.start });
        }
        for (k, d) in a.decls.iter().enumerate() {
            let did = DeclId(k as u32);
            let mut dono = Dono { proprios: Vec::new(), da_classe: Vec::new(), classe: None, extensao: None, metadata: &d.metadata, inicio: d.span.start };
            let (membros, tps, constantes): (&[dartforge_frontend::ast::MemberId], Vec<SymbolId>, &[dartforge_frontend::ast::EnumConstant]) = match &d.kind {
                DeclKind::Class(x) => (&x.members, x.type_params.iter().map(|t| t.name.sym).collect(), &[]),
                DeclKind::Mixin(x) => (&x.members, x.type_params.iter().map(|t| t.name.sym).collect(), &[]),
                DeclKind::Enum(x) => (&x.members, x.type_params.iter().map(|t| t.name.sym).collect(), &x.constants),
                DeclKind::ExtensionType(x) => (&x.members, x.type_params.iter().map(|t| t.name.sym).collect(), &[]),
                DeclKind::Extension(x) => (&x.members, x.type_params.iter().map(|t| t.name.sym).collect(), &[]),
                DeclKind::Function(f) => {
                    let f = a.function(*f);
                    dono.proprios = f.type_params.iter().map(|t| t.name.sym).collect();
                    dono.proprios.extend(f.parameters.as_deref().map(nomes_dos_parametros).unwrap_or_default());
                    donos.push(dono);
                    continue;
                }
                DeclKind::Typedef(x) => {
                    dono.proprios = x.type_params.iter().map(|t| t.name.sym).collect();
                    if let TypedefKind::Legacy { parameters, .. } = &x.kind {
                        dono.proprios.extend(nomes_dos_parametros(parameters));
                    }
                    donos.push(dono);
                    continue;
                }
                DeclKind::Variables(_) => {
                    donos.push(dono);
                    continue;
                }
            };
            let classe = if matches!(d.kind, DeclKind::Extension(_)) { None } else { classe_da_decl(s, did) };
            let extensao = if matches!(d.kind, DeclKind::Extension(_)) {
                program.extensions.iter().position(|x| x.decl.unit == s.unidade && x.decl.decl == did).map(|i| ExtensionId(i as u32))
            } else {
                None
            };
            let mut da_classe = nomes_dos_membros(a, membros);
            da_classe.extend(constantes.iter().map(|k| k.name.sym));
            da_classe.extend(tps.iter().copied());
            dono.da_classe = da_classe.clone();
            dono.classe = classe;
            dono.extensao = extensao;
            donos.push(dono);
            for kc in constantes {
                donos.push(Dono { proprios: Vec::new(), da_classe: da_classe.clone(), classe, extensao, metadata: &kc.metadata, inicio: kc.name.span.start });
            }
            for &mid in membros {
                let m = a.member(mid);
                let proprios = match &m.kind {
                    MemberKind::Method(f) => {
                        let f = a.function(*f);
                        let mut v: Vec<SymbolId> = f.type_params.iter().map(|t| t.name.sym).collect();
                        v.extend(f.parameters.as_deref().map(nomes_dos_parametros).unwrap_or_default());
                        v
                    }
                    MemberKind::Constructor(kc) => nomes_dos_parametros(&kc.parameters),
                    MemberKind::Field(_) => Vec::new(),
                };
                donos.push(Dono { proprios, da_classe: da_classe.clone(), classe, extensao, metadata: &m.metadata, inicio: m.span.start });
            }
        }
        let mut vistos: HashSet<usize> = HashSet::new();
        for dono in &donos {
            let depois = match dono.metadata.last() {
                Some(m) => dartforge_frontend::fonte::pular_brancos(fonte.as_bytes(), m.span.end),
                None => dono.inicio,
            };
            let Some(inicio_doc) = comentarios.dart_doc(fonte, depois).or_else(|| dono.metadata.iter().rev().find_map(|m| comentarios.dart_doc(fonte, m.span.start))) else {
                continue;
            };
            if !vistos.insert(inicio_doc.start) {
                continue;
            }
            // Os tokens do `Comment`.
            let pai = if comentarios.dart_doc(fonte, depois) == Some(inicio_doc) { depois } else { dono.metadata.iter().rev().find(|m| comentarios.dart_doc(fonte, m.span.start) == Some(inicio_doc)).map_or(depois, |m| m.span.start) };
            let cadeia: Vec<Span> = comentarios.antes_de(fonte, pai).into_iter().filter(|x| x.start >= inicio_doc.start).collect();
            let mut tokens = vec![inicio_doc];
            if fonte[inicio_doc.start..inicio_doc.end].starts_with("///") {
                tokens.extend(cadeia.iter().skip(1).filter(|x| fonte[x.start..x.end].starts_with("///")).copied());
            }
            // `visitComment`: as definições de link e os casos especiais.
            let mut links: Vec<String> = Vec::new();
            for t in &tokens {
                let lexema = &fonte[t.start..t.end];
                let mut inicio = 0usize;
                let mut primeira = true;
                while let Some(l) = lexema[inicio..].find('[').map(|x| x + inicio) {
                    let Some(r) = lexema[l..].find(']').map(|x| x + l) else { break };
                    let referencia = &lexema[l + 1..r];
                    if primeira {
                        let prefixo = &lexema[..l];
                        let e_comeco = prefixo.starts_with("///") && prefixo.trim_start_matches('/').chars().all(char::is_whitespace);
                        if e_comeco && lexema[r + 1..].starts_with(':') {
                            links.push(referencia.to_string());
                        }
                        primeira = false;
                    }
                    if matches!(referencia, "this" | "null" | "true" | "false") {
                        relatar(&c::COMMENT_REFERENCES, Span { start: t.start + l + 1, end: t.start + l + 1 + referencia.len() }, &[]);
                    }
                    inicio = r;
                }
            }
            // As referências não resolvidas.
            let doc = super::regras14::Doc { tokens: tokens.clone(), cadeia: cadeia.clone(), pai };
            let linhas = super::regras14::linhas_do_doc(fonte, &doc);
            let blocos = super::regras14::blocos_de_codigo(&linhas);
            let em_bloco = |k: usize| {
                let pos = linhas[k].0;
                blocos.iter().any(|b| b.linhas.iter().any(|(p, _)| *p == pos))
            };
            for partes in referencias(fonte, &linhas, &em_bloco) {
                let nomes: Vec<&str> = partes.iter().map(|(n, _)| n.as_str()).collect();
                if resolve(s, interner, dono, &nomes) {
                    continue;
                }
                let texto = nomes.join(".");
                if links.contains(&texto) {
                    continue;
                }
                // `[p.C.x]`: só relata o acesso com o alvo prefixado.
                let sp = Span { start: partes[0].1.start, end: partes[partes.len() - 1].1.end };
                relatar(&c::COMMENT_REFERENCES, sp, &[]);
            }
        }
    }

    // `prefer_void_to_null`.
    if ligada("prefer_void_to_null") {
        let mut excluidos: HashSet<TypeId> = HashSet::new();
        for t in a.types.iter() {
            if let TypeKind::Function { return_type, parameters, .. } = &t.kind {
                excluidos.extend(*return_type);
                excluidos.extend(parameters.iter().filter_map(|p| p.ty));
            }
        }
        for p in a.patterns.iter() {
            if let PatternKind::Cast { ty, .. } = &p.kind {
                excluidos.insert(*ty);
            }
        }
        for e in a.exprs.iter() {
            match &e.kind {
                ExprKind::As { ty, .. } => {
                    excluidos.insert(*ty);
                }
                ExprKind::List { type_args, elements, .. } | ExprKind::SetOrMap { type_args, elements, .. } if elements.is_empty() => {
                    excluidos.extend(type_args.iter().copied());
                }
                _ => {}
            }
        }
        for st in a.stmts.iter() {
            match &st.kind {
                StmtKind::Variables(l) | StmtKind::For { init: Some(ForInit::Variables(l)), .. } => excluidos.extend(l.ty),
                _ => {}
            }
        }
        let mut aumentados: Vec<Span> = Vec::new();
        for d in a.decls.iter() {
            match &d.kind {
                DeclKind::Variables(l) => excluidos.extend(l.ty),
                DeclKind::Extension(x) => {
                    excluidos.insert(x.on);
                }
                DeclKind::ExtensionType(x) => {
                    excluidos.insert(x.representation_type);
                }
                _ => {}
            }
            if d.augment {
                aumentados.push(d.span);
            }
        }
        for m in a.members.iter() {
            if m.augment {
                aumentados.push(m.span);
            }
        }
        // O retorno de um método que sobrescreve um membro de retorno que
        // não é `void` nem `FutureOr<void>`.
        for (k, d) in a.decls.iter().enumerate() {
            let membros = match &d.kind {
                DeclKind::Class(x) => &x.members,
                DeclKind::Mixin(x) => &x.members,
                DeclKind::Enum(x) => &x.members,
                DeclKind::ExtensionType(x) => &x.members,
                _ => continue,
            };
            let Some(classe) = classe_da_decl(s, DeclId(k as u32)) else { continue };
            for &mid in membros {
                let MemberKind::Method(fid) = &a.member(mid).kind else { continue };
                let f = a.function(*fid);
                let (Some(rt), Some(n)) = (f.return_type, f.name) else { continue };
                // O membro sobrescrito (`overriddenMember`): o primeiro dos
                // supertipos com o nome.
                let chave = if f.kind == dartforge_frontend::ast::FunctionKind::Setter { interner.lookup(&format!("{}_=", interner.resolve(n.sym))) } else { Some(n.sym) };
                let Some(ch) = chave else { continue };
                let mut sup: Vec<ClassId> = s.outline.hierarchy.get(classe).map(|h| h.supertypes.keys().copied().collect()).unwrap_or_default();
                sup.sort();
                let sobrescrito = sup.iter().find_map(|sc| program.class(*sc).instance_members.get(&ch).copied());
                if let Some(g) = sobrescrito
                    && let Some(dados) = s.outline.functions.get(g.0 as usize)
                {
                    let r = dados.return_type;
                    let e_void = matches!(table.get(r), Type::Void) || matches!(table.get(r), Type::FutureOr { arg, .. } if matches!(table.get(*arg), Type::Void));
                    if !e_void {
                        excluidos.insert(rt);
                    }
                }
            }
        }
        for (k, t) in a.types.iter().enumerate() {
            let id = TypeId(k as u32);
            let TypeKind::Named { name, .. } = &t.kind else { continue };
            if excluidos.contains(&id) || aumentados.iter().any(|sp| t.span.start >= sp.start && t.span.end <= sp.end) {
                continue;
            }
            let tipo = s.corpo.tipos_de_anotacoes.get(&id).copied().or_else(|| s.outline.tipos_escritos.get(&(s.unidade, id)).copied());
            if tipo.is_some_and(|x| matches!(table.get(x), Type::Null)) {
                relatar(&c::PREFER_VOID_TO_NULL, name[name.len() - 1].span, &[]);
            }
        }
    }

    // `type_annotate_public_apis`.
    if ligada("type_annotate_public_apis") {
        let privado = |n: SymbolId| interner.resolve(n).starts_with('_');
        let sublinhados = |n: SymbolId| {
            let t = interner.resolve(n);
            !t.is_empty() && t.bytes().all(|c| c == b'_')
        };
        let mut achados: Vec<Span> = Vec::new();
        fn parametros(ps: &[Parameter], sublinhados: &dyn Fn(SymbolId) -> bool, achados: &mut Vec<Span>) {
            for p in ps {
                if let Some(internos) = &p.function_parameters {
                    parametros(internos, sublinhados, achados);
                    continue;
                }
                if p.this_ || p.super_ {
                    continue;
                }
                if p.ty.is_none()
                    && let Some(n) = p.name
                    && !sublinhados(n.sym)
                {
                    achados.push(p.span);
                }
            }
        }
        let variaveis = |l: &dartforge_frontend::ast::VariableList, achados: &mut Vec<Span>| {
            if l.ty.is_some() {
                return;
            }
            for v in l.variables.iter() {
                if privado(v.name.sym) || l.const_ {
                    continue;
                }
                let inferido = v.initializer.and_then(|i| s.corpo.get_type(i)).is_some_and(|t| !matches!(table.get(t), Type::Dynamic | Type::Null));
                if l.final_ && inferido {
                    continue;
                }
                achados.push(v.name.span);
            }
        };
        for d in a.decls.iter() {
            if d.augment {
                continue;
            }
            match &d.kind {
                DeclKind::Variables(l) => variaveis(l, &mut achados),
                DeclKind::Function(f) => {
                    let f = a.function(*f);
                    let Some(n) = f.name else { continue };
                    if privado(n.sym) {
                        continue;
                    }
                    if f.return_type.is_none() && f.kind != dartforge_frontend::ast::FunctionKind::Setter {
                        achados.push(n.span);
                    } else if let Some(ps) = &f.parameters {
                        parametros(ps, &sublinhados, &mut achados);
                    }
                }
                DeclKind::Typedef(x) => {
                    if privado(x.name.sym) {
                        continue;
                    }
                    if let TypedefKind::Legacy { return_type, parameters } = &x.kind {
                        if return_type.is_none() {
                            achados.push(x.name.span);
                        } else {
                            parametros(parameters, &sublinhados, &mut achados);
                        }
                    }
                }
                _ => {}
            }
        }
        for m in a.members.iter() {
            if m.augment {
                continue;
            }
            match &m.kind {
                MemberKind::Field(l) => variaveis(l, &mut achados),
                MemberKind::Method(f) => {
                    let f = a.function(*f);
                    let Some(n) = f.name else { continue };
                    if privado(n.sym) {
                        continue;
                    }
                    if f.return_type.is_none() && f.kind != dartforge_frontend::ast::FunctionKind::Setter {
                        achados.push(n.span);
                    } else if let Some(ps) = &f.parameters {
                        parametros(ps, &sublinhados, &mut achados);
                    }
                }
                MemberKind::Constructor(_) => {}
            }
        }
        achados.sort_by_key(|x| x.start);
        for sp in achados {
            relatar(&c::TYPE_ANNOTATE_PUBLIC_APIS, sp, &[]);
        }
    }

    out
}
