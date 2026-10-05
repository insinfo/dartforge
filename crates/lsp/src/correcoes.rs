//! Mais correções rápidas dos códigos publicados, com os títulos, espécies e
//! edições dos produtores do Dart 3.6.2
//! (`pkg/analysis_server/lib/src/services/correction/dart/*.dart`, ligados
//! aos códigos em `fix_internal.dart`):
//!
//! | Código | Correções |
//! | --- | --- |
//! | `non_abstract_class_inherits_abstract_member` | `Create N missing override(s)`, `Create 'noSuchMethod' method`, `Make class 'C' abstract` |
//! | `concrete_class_with_abstract_member` | `Convert to block body`, `Make class 'C' abstract` |
//! | `unused_field` | `Remove unused field` |
//! | `unused_catch_clause` / `unused_catch_stack` | `Remove unused 'catch' clause` / `Remove unused stack trace variable` |
//! | `assignment_to_final_local` | `Make variable 'x' not final` |
//! | `missing_default_value_for_parameter` | `Add 'required' keyword` (nomeado), `Make 'x' nullable` |
//! | `await_in_wrong_context` | `Add 'async' modifier` (com `Future<…>` no retorno) |
//! | `nullable_type_in_{extends,implements,on,with}_clause` | `Remove the '?'` |
//! | `const_instance_field` | `Add 'static' modifier` |
//! | `non_final_field_in_enum` | `Make final` |
//! | `extension_declares_member_of_object`, `extension_type_declares_member_of_object` | `Remove method declaration` |
//! | `assert_in_redirecting_constructor` | `Remove the assertion` |
//!
//! Cada edição é conferida contra a árvore do texto vigente: diagnóstico
//! que não corresponde ao nó esperado não gera ação.

use crate::acoes::AcaoDeCodigo;
use crate::projeto::{Projeto, nome_base};
use crate::Edicao;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{ClassId, FunctionElementId, FunctionKind, FunctionRef, UnitId};
use dartforge_frontend::ast::{self, DeclKind, MemberKind, ParameterKind, StmtKind};
use dartforge_types::{Type, TypeId};
use std::collections::{BTreeMap, HashMap, HashSet};

fn acao(uri: &str, titulo: String, especie: &str, edicoes: Vec<(Span, String)>, d: &Diagnostic) -> AcaoDeCodigo {
    AcaoDeCodigo {
        titulo,
        especie: especie.into(),
        edicoes: edicoes.into_iter().map(|(span, texto)| Edicao { uri: uri.to_string(), span, texto }).collect(),
        diagnostico: Some(d.clone()),
        criar_arquivo: None,
    }
}

/// O trecho estendido à linha inteira quando ele ocupa a linha sozinho.
fn linhas_inteiras(texto: &str, span: Span) -> Span {
    let ini = texto[..span.start].rfind('\n').map_or(0, |i| i + 1);
    let fim = texto[span.end..].find('\n').map_or(texto.len(), |i| span.end + i);
    if texto[ini..span.start].trim().is_empty() && texto[span.end..fim].trim().is_empty() {
        Span { start: ini, end: (fim + 1).min(texto.len()) }
    } else {
        span
    }
}

/// As correções desta lista para os diagnósticos que tocam `inicio..fim`.
pub(crate) fn corrigir(projeto: &mut Projeto, uri: &str, diagnosticos: &[Diagnostic], inicio: usize, fim: usize) -> Vec<AcaoDeCodigo> {
    let Some(unidade) = projeto.unidade_do_uri(uri) else { return Vec::new() };
    let mut saida = Vec::new();
    for d in diagnosticos {
        if !(d.span.start <= fim && inicio <= d.span.end) {
            continue;
        }
        let Some(codigo) = d.code.map(|c| c.info().nome) else { continue };
        let texto = projeto.programa().unit(unidade).source.clone();
        if d.span.end > texto.len() {
            continue;
        }
        match codigo {
            "non_abstract_class_inherits_abstract_member" => {
                let Some((c, decl)) = classe_com_nome(projeto, unidade, d.span) else { continue };
                if let Some((titulo, edicao)) = sobrescritas_ausentes(projeto, c, unidade, decl) {
                    saida.push(acao(uri, titulo, "quickfix.create.missingOverrides", vec![edicao], d));
                }
                if let Some(e) = no_such_method(projeto, unidade, decl) {
                    saida.push(acao(uri, "Create 'noSuchMethod' method".into(), "quickfix.create.noSuchMethod", vec![e], d));
                }
                if let Some((nome, e)) = classe_abstrata(projeto, unidade, decl) {
                    saida.push(acao(uri, format!("Make class '{nome}' abstract"), "quickfix.makeClassAbstract", vec![e], d));
                }
            }
            "concrete_class_with_abstract_member" => {
                // `ConvertIntoBlockBody.missingBody`, `CreateNoSuchMethod`
                // (só com `node` na `ClassDeclaration`: aqui o erro está no
                // membro, nunca sai) e `MakeClassAbstract`.
                let cx = crate::refatoracoes::Contexto::novo(projeto, unidade);
                if let Some(e) = cx.converter_em_corpo_de_bloco(d.span) {
                    saida.push(acao(uri, "Convert to block body".into(), "quickfix.convert.bodyToBlock", vec![e], d));
                }
                drop(cx);
                let Some(decl) = classe_que_contem(projeto, unidade, d.span.start) else { continue };
                if let Some((nome, e)) = classe_abstrata(projeto, unidade, decl) {
                    saida.push(acao(uri, format!("Make class '{nome}' abstract"), "quickfix.makeClassAbstract", vec![e], d));
                }
            }
            "unused_field" => {
                // `RemoveUnusedField`: a declaração e cada referência.
                let cx = crate::refatoracoes::Contexto::novo(projeto, unidade);
                if let Some(faixas) = cx.remover_campo(d.span) {
                    let edicoes = faixas.into_iter().map(|s| (s, String::new())).collect();
                    saida.push(acao(uri, "Remove unused field".into(), "quickfix.remove.unusedField", edicoes, d));
                }
            }
            "unused_catch_clause" | "unused_catch_stack" => {
                let ast = &projeto.programa().unit(unidade).ast;
                for s in &ast.stmts {
                    let StmtKind::Try { catches, .. } = &s.kind else { continue };
                    for k in catches.iter() {
                        let corpo = ast.stmt(k.body).span.start;
                        if codigo == "unused_catch_clause" && k.exception.is_some_and(|n| n.span == d.span) {
                            // De `catch` ao corpo (fica o `on T`).
                            let Some(c) = texto[k.span.start..corpo].find("catch").map(|i| k.span.start + i) else { continue };
                            if k.on_type.is_some() {
                                saida.push(acao(uri, "Remove unused 'catch' clause".into(), "quickfix.remove.unusedCatchClause", vec![(Span { start: c, end: corpo }, String::new())], d));
                            }
                        }
                        if codigo == "unused_catch_stack"
                            && let (Some(e), Some(st)) = (k.exception, k.stack_trace)
                            && st.span == d.span
                        {
                            saida.push(acao(uri, "Remove unused stack trace variable".into(), "quickfix.remove.unusedCatchStack", vec![(Span { start: e.span.end, end: st.span.end }, String::new())], d));
                        }
                    }
                }
            }
            "assignment_to_final_local" => {
                if let Some((nome, e)) = local_nao_final(projeto, unidade, d.span) {
                    saida.push(acao(uri, format!("Make variable '{nome}' not final"), "quickfix.makeVariableNotFinal", e, d));
                }
            }
            "missing_default_value_for_parameter" => {
                let ast = &projeto.programa().unit(unidade).ast;
                if let Some(p) = parametro_com_nome(ast, d.span) {
                    if p.kind == ParameterKind::Named && !p.required {
                        let ini = p.metadata.last().map_or(p.span.start, |a| {
                            let resto = &texto[a.span.end..];
                            a.span.end + (resto.len() - resto.trim_start().len())
                        });
                        saida.push(acao(uri, "Add 'required' keyword".into(), "quickfix.add.required", vec![(Span { start: ini, end: ini }, "required ".into())], d));
                    }
                    if let Some(t) = p.ty {
                        let ts = ast.ty(t).span;
                        let nome = &texto[d.span.start..d.span.end];
                        saida.push(acao(uri, format!("Make '{nome}' nullable"), "quickfix.makeVariableNullable", vec![(Span { start: ts.end, end: ts.end }, "?".into())], d));
                    }
                }
            }
            "await_in_wrong_context" => {
                // `AddAsync` com o `convertFunctionFromSyncToAsync`; as
                // bibliotecas que o `writeType` agendou entram como imports.
                let cx = crate::refatoracoes::Contexto::novo(projeto, unidade);
                if let Some((edicoes, importar)) = cx.adicionar_async(d.span) {
                    let mut m = crate::refatoracoes_exec::Mudanca::default();
                    for (s, x) in edicoes {
                        m.adicionar(uri, s, x);
                    }
                    crate::refatoracoes_metodo::adicionar_imports(&cx, &mut m, &importar);
                    if m.conflito.is_none() {
                        let todas: Vec<Edicao> = m.arquivos.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect();
                        saida.push(AcaoDeCodigo {
                            titulo: "Add 'async' modifier".into(),
                            especie: "quickfix.add.async".into(),
                            edicoes: todas,
                            diagnostico: Some(d.clone()),
                            criar_arquivo: None,
                        });
                    }
                }
            }
            "nullable_type_in_extends_clause" | "nullable_type_in_implements_clause" | "nullable_type_in_on_clause" | "nullable_type_in_with_clause" => {
                let trecho = &texto[d.span.start..d.span.end];
                if trecho.ends_with('?') {
                    saida.push(acao(uri, "Remove the '?'".into(), "quickfix.remove.questionMark", vec![(Span { start: d.span.end - 1, end: d.span.end }, String::new())], d));
                }
            }
            "const_instance_field" => {
                let ast = &projeto.programa().unit(unidade).ast;
                if let Some(m) = ast.members.iter().find(|m| m.span.start <= d.span.start && d.span.end <= m.span.end && matches!(m.kind, MemberKind::Field(_))) {
                    let ini = m.metadata.last().map_or(m.span.start, |a| {
                        let resto = &texto[a.span.end..];
                        a.span.end + (resto.len() - resto.trim_start().len())
                    });
                    saida.push(acao(uri, "Add 'static' modifier".into(), "quickfix.add.static", vec![(Span { start: ini, end: ini }, "static ".into())], d));
                }
            }
            "non_final_field_in_enum" => {
                let cx = crate::refatoracoes::Contexto::novo(projeto, unidade);
                if let Some(e) = cx.tornar_final(d.span) {
                    saida.push(acao(uri, "Make final".into(), "quickfix.makeFinal", vec![e], d));
                }
            }
            "extension_declares_member_of_object" | "extension_type_declares_member_of_object" => {
                let ast = &projeto.programa().unit(unidade).ast;
                if let Some(m) = ast.members.iter().find(|m| m.span.start <= d.span.start && d.span.end <= m.span.end) {
                    let span = com_documentacao(&texto, linhas_inteiras(&texto, m.span));
                    saida.push(acao(uri, "Remove method declaration".into(), "quickfix.remove.methodDeclaration", vec![(span, String::new())], d));
                }
            }
            "assert_in_redirecting_constructor" => {
                // `RemoveAssertion`.
                let cx = crate::refatoracoes::Contexto::novo(projeto, unidade);
                if let Some(s) = cx.remover_assercao(d.span) {
                    saida.push(acao(uri, "Remove the assertion".into(), "quickfix.remove.assertion", vec![(s, String::new())], d));
                }
            }
            _ => {}
        }
    }
    saida
}

/// `span` estendido para cima pelas linhas `///` logo acima dele.
fn com_documentacao(texto: &str, span: Span) -> Span {
    let mut inicio = span.start;
    while inicio > 0 {
        let fim_anterior = inicio - 1;
        let comeco = texto[..fim_anterior].rfind('\n').map_or(0, |i| i + 1);
        if texto[comeco..fim_anterior].trim_start().starts_with("///") {
            inicio = comeco;
        } else {
            break;
        }
    }
    Span { start: inicio, end: span.end }
}

/// O parâmetro (de função, método ou construtor) cujo nome está em `nome`.
fn parametro_com_nome(ast: &ast::Ast, nome: Span) -> Option<&ast::Parameter> {
    let funcoes = ast.functions.iter().flat_map(|f| f.parameters.iter().flatten());
    let construtores = ast.members.iter().flat_map(|m| match &m.kind {
        MemberKind::Constructor(k) => k.parameters.iter(),
        _ => [].iter(),
    });
    funcoes.chain(construtores).find(|p| p.name.is_some_and(|n| n.span == nome))
}

/// A classe (elemento e declaração) cujo nome está em `nome`.
fn classe_com_nome(projeto: &Projeto, unidade: UnitId, nome: Span) -> Option<(ClassId, ast::DeclId)> {
    let p = projeto.programa();
    let ast = &p.unit(unidade).ast;
    let (i, _) = ast.decls.iter().enumerate().find(|(_, d)| match &d.kind {
        DeclKind::Class(k) => k.name.span == nome,
        DeclKind::Enum(k) => k.name.span == nome,
        _ => false,
    })?;
    let decl = ast::DeclId(i as u32);
    let c = (0..p.classes.len()).map(|x| ClassId(x as u32)).find(|c| p.class(*c).decl.is_some_and(|d| d.unit == unidade && d.decl == decl))?;
    Some((c, decl))
}

/// A declaração de classe que contém `offset`.
fn classe_que_contem(projeto: &Projeto, unidade: UnitId, offset: usize) -> Option<ast::DeclId> {
    let ast = &projeto.programa().unit(unidade).ast;
    ast.decls
        .iter()
        .enumerate()
        .find(|(_, d)| matches!(d.kind, DeclKind::Class(_)) && d.span.start <= offset && offset < d.span.end)
        .map(|(i, _)| ast::DeclId(i as u32))
}

/// `abstract ` antes da palavra `class` (depois dos metadados).
fn classe_abstrata(projeto: &Projeto, unidade: UnitId, decl: ast::DeclId) -> Option<(String, (Span, String))> {
    let u = projeto.programa().unit(unidade);
    let d = u.ast.decl(decl);
    let DeclKind::Class(k) = &d.kind else { return None };
    if k.modifiers.abstract_ || k.modifiers.sealed {
        return None;
    }
    let ini = d.metadata.last().map_or(d.span.start, |a| a.span.end);
    let resto = &u.source[ini..];
    let ini = ini + (resto.len() - resto.trim_start().len());
    Some((u.source[k.name.span.start..k.name.span.end].to_string(), (Span { start: ini, end: ini }, "abstract ".into())))
}

/// `noSuchMethod` antes do `}` da classe.
fn no_such_method(projeto: &Projeto, unidade: UnitId, decl: ast::DeclId) -> Option<(Span, String)> {
    let u = projeto.programa().unit(unidade);
    let d = u.ast.decl(decl);
    let DeclKind::Class(k) = &d.kind else { return None };
    if !u.source[..d.span.end].ends_with('}') {
        return None;
    }
    let pos = d.span.end - 1;
    let separador = if k.members.is_empty() { "" } else { "\n" };
    Some((Span { start: pos, end: pos }, format!("{separador}  @override\n  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);\n")))
}

/// `Make variable 'x' not final` para a atribuição em `uso`.
fn local_nao_final(projeto: &Projeto, unidade: UnitId, uso: Span) -> Option<(String, Vec<(Span, String)>)> {
    let u = projeto.programa().unit(unidade);
    let ast = &u.ast;
    let corpos = &projeto.consulta.corpos.units[unidade.0 as usize];
    let expr = ast.exprs.iter().position(|e| e.span == uso && matches!(e.kind, ast::ExprKind::Identifier(_)))?;
    let decl = corpos.declaracao_local(ast::ExprId(expr as u32))?;
    let lista = ast.stmts.iter().find_map(|s| match &s.kind {
        StmtKind::Variables(l) if l.variables.len() == 1 && l.variables[0].name.span.start == decl => Some((s.span, l)),
        _ => None,
    })?;
    let (span, l) = lista;
    if !l.final_ {
        return None;
    }
    let trecho = &u.source[span.start..];
    let f = span.start + trecho.find("final")?;
    let nome = u.source[decl..].split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$')).next()?.to_string();
    let edicao = match l.ty {
        Some(t) => (Span { start: f, end: ast.ty(t).span.start }, String::new()),
        None => (Span { start: f, end: f + 5 }, "var".to_string()),
    };
    Some((nome, vec![edicao]))
}

/// Um membro a sobrescrever: nome exibido, se é getter/setter e o elemento.
struct Ausente {
    nome: String,
    getter: bool,
    setter: bool,
    funcao: FunctionElementId,
    dono: ClassId,
}

/// `Create N missing override(s)`: os membros abstratos ou de interface sem
/// implementação concreta na cadeia de `c`, ordenados pelo nome (getter
/// antes de setter; um par getter/setter vira campo), inseridos depois do
/// último membro (ou logo depois do `{`).
fn sobrescritas_ausentes(projeto: &mut Projeto, c: ClassId, unidade: UnitId, decl: ast::DeclId) -> Option<(String, (Span, String))> {
    let ausentes = ausentes(projeto, c);
    if ausentes.is_empty() {
        return None;
    }
    let p = projeto.programa();
    let u = p.unit(unidade);
    let d = u.ast.decl(decl);
    let (membros, enum_) = match &d.kind {
        DeclKind::Class(k) => (k.members.clone(), false),
        DeclKind::Enum(k) => (k.members.clone(), true),
        _ => return None,
    };
    let fonte = u.source.clone();
    let ultimo = membros.iter().map(|m| u.ast.member(*m).span.end).max();
    let abre = fonte[d.span.start..d.span.end].find('{').map(|i| d.span.start + i + 1);
    let (posicao, prefixo) = match (ultimo, abre) {
        (Some(f), _) => (f, "\n\n  ".to_string()),
        (None, Some(a)) => (a, "\n  ".to_string()),
        _ => return None,
    };
    let linha_unica = !fonte[d.span.start..d.span.end].contains('\n');
    let mapa = mapeamento(projeto, c);
    let mut textos = Vec::new();
    let mut n = ausentes.len();
    let mut i = 0;
    while i < ausentes.len() {
        let a = &ausentes[i];
        if a.getter && ausentes.get(i + 1).is_some_and(|b| b.setter && b.nome == a.nome) {
            let tipo = retorno(projeto, a.funcao, mapa.get(&a.dono));
            let final_ = if enum_ { "final " } else { "" };
            textos.push(format!("@override\n  {final_}{tipo} {};", a.nome));
            n -= 1;
            i += 2;
            continue;
        }
        textos.push(sobrescrita(projeto, a, mapa.get(&a.dono)));
        i += 1;
    }
    let mut texto = prefixo + &textos.join("\n\n  ");
    if ultimo.is_none() && linha_unica {
        texto.push('\n');
    }
    let titulo = format!("Create {n} missing override{}", if n == 1 { "" } else { "s" });
    Some((titulo, (Span { start: posicao, end: posicao }, texto)))
}

/// Os membros sem implementação concreta na cadeia de `c`.
fn ausentes(projeto: &Projeto, c: ClassId) -> Vec<Ausente> {
    let p = projeto.programa();
    // Cadeia de implementação: a classe, os mixins (do último ao primeiro) e
    // a superclasse, recursivamente.
    let mut cadeia = Vec::new();
    let mut atual = Some(c);
    let mut vistos = HashSet::new();
    while let Some(x) = atual {
        if !vistos.insert(x) {
            break;
        }
        cadeia.push(x);
        for m in p.class(x).mixin_classes.iter().rev() {
            cadeia.push(*m);
        }
        atual = p.class(x).supertype_class;
    }
    let concreto = |nome: &str| {
        cadeia.iter().any(|x| {
            p.class(*x).instance_members.iter().any(|(s, f)| projeto.nome(*s) == nome && !p.function(*f).abstract_)
        })
    };
    let mut supers: Vec<ClassId> = projeto.supertipos(c).into_iter().collect();
    supers.sort();
    let mut por_nome: BTreeMap<String, (FunctionElementId, ClassId)> = BTreeMap::new();
    for s in supers {
        for (sym, f) in &p.class(s).instance_members {
            let nome = projeto.nome(*sym).to_string();
            if nome == "noSuchMethod" {
                continue;
            }
            por_nome.entry(nome).or_insert((*f, s));
        }
    }
    let mut saida: Vec<Ausente> = por_nome
        .into_iter()
        .filter(|(nome, _)| !concreto(nome))
        .map(|(nome, (f, dono))| {
            let fe = p.function(f);
            let setter = nome.ends_with('=') && nome != "==" && !matches!(fe.kind, FunctionKind::Operator);
            let getter = matches!(fe.kind, FunctionKind::Getter) || (fe.kind == FunctionKind::ImplicitAccessor && !setter);
            Ausente { nome: nome_base(&nome).to_string(), getter, setter, funcao: f, dono }
        })
        .collect();
    // Pelo nome exibido; getter antes de setter.
    saida.sort_by(|a, b| (a.nome.as_str(), !a.getter).cmp(&(b.nome.as_str(), !b.getter)));
    saida
}

/// Para cada supertipo de `c`, os argumentos de tipo como `c` o vê.
fn mapeamento(projeto: &mut Projeto, c: ClassId) -> HashMap<ClassId, HashMap<dartforge_types::TypeParamId, TypeId>> {
    let supers: Vec<ClassId> = projeto.supertipos(c).into_iter().collect();
    let consulta = &mut projeto.consulta;
    let Some(dados) = consulta.outline.classes.get(c.0 as usize) else { return HashMap::new() };
    let args: Box<[TypeId]> = dados
        .type_params
        .clone()
        .iter()
        .map(|tp| consulta.tabela.intern(Type::TypeParameter { param: *tp, nullable: false }))
        .collect();
    let este = consulta.tabela.intern(Type::Interface { class: c, args, nullable: false });
    let mut saida = HashMap::new();
    for s in supers {
        let Some(t) = consulta.outline.hierarchy.supertype_of(este, s, &mut consulta.tabela, &consulta.core) else { continue };
        let Type::Interface { args, .. } = consulta.tabela.get(t).clone() else { continue };
        let Some(ds) = consulta.outline.classes.get(s.0 as usize) else { continue };
        if ds.type_params.len() == args.len() {
            saida.insert(s, ds.type_params.iter().copied().zip(args.iter().copied()).collect());
        }
    }
    saida
}

fn formatar(projeto: &mut Projeto, t: TypeId, mapa: Option<&HashMap<dartforge_types::TypeParamId, TypeId>>) -> String {
    let t = match mapa {
        Some(m) => dartforge_types::ops::substitute(t, m, &mut projeto.consulta.tabela),
        None => t,
    };
    projeto.consulta.formatar(t)
}

/// O tipo de retorno (ou do getter) de `f` como `c` o vê.
fn retorno(projeto: &mut Projeto, f: FunctionElementId, mapa: Option<&HashMap<dartforge_types::TypeParamId, TypeId>>) -> String {
    let fe = projeto.programa().function(f);
    let t = match (fe.kind, fe.variable) {
        (FunctionKind::ImplicitAccessor, Some(v)) => projeto.consulta.tipo_da_variavel(v).unwrap_or(projeto.consulta.outline.functions[f.0 as usize].return_type),
        _ => projeto.consulta.outline.functions[f.0 as usize].return_type,
    };
    formatar(projeto, t, mapa)
}

/// O texto de uma sobrescrita (`writeOverride` do Dart).
fn sobrescrita(projeto: &mut Projeto, a: &Ausente, mapa: Option<&HashMap<dartforge_types::TypeParamId, TypeId>>) -> String {
    let nome = a.nome.clone();
    if a.getter {
        let tipo = retorno(projeto, a.funcao, mapa);
        return format!("@override\n  // TODO: implement {nome}\n  {tipo} get {nome} => throw UnimplementedError();");
    }
    let parametros = lista_de_parametros(projeto, a.funcao, mapa);
    if a.setter {
        return format!("@override\n  set {nome}{parametros} {{\n    // TODO: implement {nome}\n  }}");
    }
    let tipo = retorno(projeto, a.funcao, mapa);
    let operador = if projeto.programa().function(a.funcao).kind == FunctionKind::Operator { "operator " } else { "" };
    let corpo = if tipo == "void" { String::new() } else { "\n    throw UnimplementedError();".to_string() };
    format!("@override\n  {tipo} {operador}{nome}{parametros} {{\n    // TODO: implement {nome}{corpo}\n  }}")
}

/// `<T>(int a, [int b = 0], {required int c})` de `f`, com os tipos como a
/// classe os vê.
fn lista_de_parametros(projeto: &mut Projeto, f: FunctionElementId, mapa: Option<&HashMap<dartforge_types::TypeParamId, TypeId>>) -> String {
    let p = projeto.programa();
    let (unit, ps, tps): (UnitId, Vec<(ParameterKind, bool, Option<String>, Option<String>)>, String) = match p.function(f).node {
        FunctionRef::Function { unit, function } => {
            let u = p.unit(unit);
            let func = u.ast.function(function);
            let ps = func
                .parameters
                .iter()
                .flatten()
                .map(|x| {
                    let nome = x.public_name.or(x.name).map(|n| u.source[n.span.start..n.span.end].to_string());
                    let padrao = x.default_value.map(|e| {
                        let s = u.ast.expr(e).span;
                        u.source[s.start..s.end].to_string()
                    });
                    (x.kind, x.required, nome, padrao)
                })
                .collect();
            let tps = if func.type_params.is_empty() {
                String::new()
            } else {
                format!("<{}>", func.type_params.iter().map(|t| &u.source[t.span.start..t.span.end]).collect::<Vec<_>>().join(", "))
            };
            (unit, ps, tps)
        }
        _ => {
            // Setter implícito de campo: um parâmetro com o tipo do campo.
            let ps = vec![(ParameterKind::Required, false, Some("value".to_string()), None)];
            (UnitId(0), ps, String::new())
        }
    };
    let _ = unit;
    let tipos: Vec<TypeId> = projeto.consulta.outline.functions[f.0 as usize].parameters.iter().map(|q| q.ty).collect();
    let mut obrigatorios = Vec::new();
    let mut opcionais = Vec::new();
    let mut nomeados = Vec::new();
    for (i, (tipo_p, req, nome, padrao)) in ps.into_iter().enumerate() {
        let tipo = tipos.get(i).map_or_else(|| "dynamic".to_string(), |t| formatar(projeto, *t, mapa));
        let mut s = format!("{tipo} {}", nome.unwrap_or_else(|| format!("p{i}")));
        if let Some(v) = padrao {
            s.push_str(&format!(" = {v}"));
        }
        match tipo_p {
            ParameterKind::Required => obrigatorios.push(s),
            ParameterKind::Optional => opcionais.push(s),
            ParameterKind::Named => nomeados.push(if req { format!("required {s}") } else { s }),
        }
    }
    let mut partes = obrigatorios;
    if !opcionais.is_empty() {
        partes.push(format!("[{}]", opcionais.join(", ")));
    }
    if !nomeados.is_empty() {
        partes.push(format!("{{{}}}", nomeados.join(", ")));
    }
    format!("{tps}({})", partes.join(", "))
}

