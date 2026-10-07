//! `final_not_initialized` conforme `_checkForNotInitializedFieldDeclaration`
//! e a verificação de variáveis de topo do `ErrorVerifier` do analyzer.

use crate::Unidade;
use dartforge_diagnostics::codigos::compile_time_error as c;
use dartforge_diagnostics::Diagnostic;
use dartforge_frontend::ast::{Ast, DeclKind, Initializer, MemberKind, TypeId, TypeKind, VariableList};
use dartforge_intern::{Interner, SymbolId};
use std::collections::{HashMap, HashSet};

fn final_sem_inicializador(v: &VariableList, nomes: &Interner, out: &mut Vec<(usize, Diagnostic)>, unidade: usize, enum_index: bool) {
    if !v.final_ || v.const_ || v.late || v.external || v.abstract_ {
        return;
    }
    for var in v.variables.iter().filter(|var| var.initializer.is_none()) {
        if enum_index && nomes.resolve(var.name.sym) == "index" { continue; }
        out.push((unidade, Diagnostic::com_codigo(
            c::FINAL_NOT_INITIALIZED,
            var.name.span,
            [nomes.resolve(var.name.sym)],
        )));
    }
}

/// Evidência sintática suficiente de não anulabilidade. Tipos importados,
/// aliases e parâmetros de tipo ficam para a fase semântica; supô-los aqui
/// poderia criar falsos positivos (`typedef T = int?`, por exemplo).
fn tipo_certo_nao_nulo(ast: &Ast, id: TypeId, nomes: &Interner, aliases: &HashSet<SymbolId>) -> bool {
    let ty = ast.ty(id);
    if ty.nullable { return false; }
    match &ty.kind {
        TypeKind::Function { .. } | TypeKind::Record { .. } => true,
        TypeKind::Named { name, .. } if name.len() == 1 && !aliases.contains(&name[0].sym) => matches!(
            nomes.resolve(name[0].sym),
            "Object" | "Never" | "num" | "int" | "double" | "bool" | "String"
                | "List" | "Map" | "Set" | "Iterable" | "Iterator" | "Future" | "Stream"
        ),
        _ => false,
    }
}

/// `_checkForNotInitializedNonNullableInstanceFields` (3.6.2,
/// `error_verifier.dart:4837-4862`): a lista não estática, não `late` e não
/// `final` (a `const` entra: o `isFinal` da lista é só a palavra `final`);
/// cada variável sem inicializador, não abstrata nem externa, de tipo
/// potencialmente não anulável. `nao_anulavel` é o predicado pelo tipo do
/// elemento (unidade, início do nome); sem ele, a evidência sintática.
fn instancia_nao_final_nao_nula(v: &VariableList, ast: &Ast, nomes: &Interner, aliases: &HashSet<SymbolId>, out: &mut Vec<(usize, Diagnostic)>, unidade: usize, enum_index: bool, nao_anulavel: &dyn Fn(usize, usize) -> Option<bool>) {
    if v.static_ || v.final_ || v.late || v.external || v.abstract_ {
        return;
    }
    for var in v.variables.iter().filter(|var| var.initializer.is_none()) {
        let potencial = match nao_anulavel(unidade, var.name.span.start) {
            Some(b) => b,
            None => !v.const_ && v.ty.is_some_and(|t| tipo_certo_nao_nulo(ast, t, nomes, aliases)),
        };
        if !potencial {
            continue;
        }
        if enum_index && nomes.resolve(var.name.sym) == "index" { continue; }
        out.push((unidade, Diagnostic::com_codigo(
            c::NOT_INITIALIZED_NON_NULLABLE_INSTANCE_FIELD,
            var.name.span,
            [nomes.resolve(var.name.sym)],
        )));
    }
}

/// A classe dos campos: (enum?, nome, unidade, declaração); a unidade e a
/// declaração são zero quando as declarações se juntam pelo nome.
type Chave = (bool, SymbolId, usize, usize);

/// Uma classe com construtor gerador explícito delega a verificação dos seus
/// campos de instância ao `ConstructorFieldsVerifier`; factories não a fazem.
/// `CONST_NOT_INITIALIZED` (o `parseFieldInitializerOpt` do parser,
/// `parser_impl.dart:3942-3947`, e o `_checkForFinalNotInitialized` do
/// `ErrorVerifier`, `error_verifier.dart:3576-3617`): cada variável sem
/// inicializador numa lista `const` de topo, de campo (qualquer declaração)
/// ou local — não a do cabeçalho de um `for`. No nome. Escrito sem compilar
/// nem executar (2026-10-05).
pub fn constantes_nao_inicializadas(u: &Unidade<'_>, nomes: &Interner) -> Vec<Diagnostic> {
    let a = u.ast;
    let mut out = Vec::new();
    let mut lista = |l: &VariableList| {
        if !l.const_ {
            return;
        }
        for var in l.variables.iter().filter(|v| v.initializer.is_none()) {
            out.push(Diagnostic::com_codigo(c::CONST_NOT_INITIALIZED, var.name.span, [nomes.resolve(var.name.sym)]));
        }
    };
    for d in &a.decls {
        if let DeclKind::Variables(l) = &d.kind {
            lista(l);
        }
    }
    for m in &a.members {
        if let MemberKind::Field(l) = &m.kind {
            lista(l);
        }
    }
    for s in &a.stmts {
        if let dartforge_frontend::ast::StmtKind::Variables(l) = &s.kind {
            lista(l);
        }
    }
    out
}

pub fn finais_nao_inicializados(unidades: &[Unidade<'_>], nomes: &Interner) -> Vec<(usize, Diagnostic)> {
    finais_nao_inicializados_com(unidades, nomes, &|_, _| None)
}

/// Como [`finais_nao_inicializados`], com o predicado semântico
/// "potencialmente não anulável" do tipo de cada campo (pela unidade, na
/// ordem de `unidades`, e pelo início do nome); `None` cai na evidência
/// sintática.
pub fn finais_nao_inicializados_com(unidades: &[Unidade<'_>], nomes: &Interner, nao_anulavel: &dyn Fn(usize, usize) -> Option<bool>) -> Vec<(usize, Diagnostic)> {
    let aliases: HashSet<_> = unidades.iter().flat_map(|u| u.unit.declarations.iter().filter_map(|&id| {
        match &u.ast.decl(id).kind {
            DeclKind::Typedef(x) => Some(x.name.sym),
            _ => None,
        }
    })).collect();
    // `unidades` contém uma biblioteca e suas partes. As declarações
    // aumentadas da mesma classe podem estar em unidades diferentes, e com
    // o experimento elas se juntam pelo nome. Sem ele (nenhuma declaração
    // `augment`), cada declaração tem os próprios campos, inclusive a classe
    // homônima (T1.1 e, `c11`: o construtor de cada uma acusa os campos
    // dela).
    let juntar = unidades.iter().any(|u| u.unit.declarations.iter().any(|&d| u.ast.decl(d).augment));
    let chave_de = |e: bool, nome: SymbolId, unidade: usize, id: dartforge_frontend::ast::DeclId| -> Chave {
        if juntar { (e, nome, 0, 0) } else { (e, nome, unidade, id.0 as usize) }
    };
    let mut com_gerador = HashSet::<Chave>::new();
    let mut campos = HashMap::<Chave, Vec<(SymbolId, bool, bool)>>::new();
    for (ui, unidade) in unidades.iter().enumerate() {
        for &id in &unidade.unit.declarations {
            let (chave, membros, ffi) = match &unidade.ast.decl(id).kind {
                DeclKind::Class(x) => {
                    let ffi = x.extends.is_some_and(|t| {
                        if let TypeKind::Named { name, .. } = &unidade.ast.ty(t).kind {
                            name.last().is_some_and(|n| matches!(nomes.resolve(n.sym), "Struct" | "Union"))
                        } else { false }
                    });
                    (chave_de(false, x.name.sym, ui, id), &x.members, ffi)
                }
                DeclKind::Enum(x) => (chave_de(true, x.name.sym, ui, id), &x.members, false),
                _ => continue,
            };
            if membros.iter().any(|&m| {
                matches!(&unidade.ast.member(m).kind, MemberKind::Constructor(k) if !k.factory)
            }) {
                com_gerador.insert(chave);
            }
            for &m in membros {
                if let MemberKind::Field(v) = &unidade.ast.member(m).kind {
                    if v.static_ { continue; }
                    for var in &v.variables {
                        // O membro `Enum.index` implícito não participa da
                        // checagem de inicialização, mesmo que o usuário
                        // declare outro `index` (erro próprio de enum).
                        if chave.0 && nomes.resolve(var.name.sym) == "index" { continue; }
                        campos.entry(chave).or_default().push((
                            var.name.sym,
                            v.final_ && !v.const_ && !v.late && !v.external && !v.abstract_ && var.initializer.is_none(),
                            !ffi && !v.final_ && !v.const_ && !v.late && !v.external && !v.abstract_
                                && var.initializer.is_none()
                                && match nao_anulavel(ui, var.name.span.start) {
                                    Some(b) => b,
                                    None => v.ty.is_some_and(|t| tipo_certo_nao_nulo(unidade.ast, t, nomes, &aliases)),
                                },
                        ));
                    }
                }
            }
        }
    }

    // O verificador oficial ignora nomes de campos duplicados; o diagnóstico
    // de duplicata já é emitido por outro verificador.
    let mut pendentes = HashMap::<Chave, Vec<SymbolId>>::new();
    let mut pendentes_nao_finais = HashMap::<Chave, Vec<SymbolId>>::new();
    for (chave, campos) in campos {
        let mut contagem = HashMap::<SymbolId, usize>::new();
        for (nome, _, _) in &campos { *contagem.entry(*nome).or_default() += 1; }
        pendentes.insert(chave, campos.iter().filter_map(|(nome, falta, _)| {
            (*falta && contagem[nome] == 1).then_some(*nome)
        }).collect());
        pendentes_nao_finais.insert(chave, campos.iter().filter_map(|(nome, _, falta)| {
            (*falta && contagem[nome] == 1).then_some(*nome)
        }).collect());
    }

    let mut out = Vec::new();
    for (i, unidade) in unidades.iter().enumerate() {
        for &id in &unidade.unit.declarations {
            let membros = match &unidade.ast.decl(id).kind {
                DeclKind::Variables(v) => {
                    final_sem_inicializador(v, nomes, &mut out, i, false);
                    continue;
                }
                DeclKind::Class(x) => (&x.members, com_gerador.contains(&chave_de(false, x.name.sym, i, id))),
                DeclKind::Enum(x) => (&x.members, com_gerador.contains(&chave_de(true, x.name.sym, i, id))),
                DeclKind::Mixin(x) => (&x.members, false),
                DeclKind::Extension(x) => (&x.members, false),
                DeclKind::ExtensionType(x) => (&x.members, false),
                _ => continue,
            };
            for &m in membros.0 {
                match &unidade.ast.member(m).kind {
                    MemberKind::Field(v) => {
                        if v.static_ || !membros.1 {
                            let enum_index = !v.static_ && matches!(&unidade.ast.decl(id).kind, DeclKind::Enum(_));
                            final_sem_inicializador(v, nomes, &mut out, i, enum_index);
                            if !membros.1 {
                                let valido = match &unidade.ast.decl(id).kind {
                                    DeclKind::Class(x) => !x.extends.is_some_and(|t| {
                                        if let TypeKind::Named { name, .. } = &unidade.ast.ty(t).kind {
                                            name.last().is_some_and(|n| matches!(nomes.resolve(n.sym), "Struct" | "Union"))
                                        } else { false }
                                    }),
                                    // Classe, enum, extensão e mixin
                                    // (`_checkForFinalNotInitializedInClass`).
                                    DeclKind::Enum(_) | DeclKind::Mixin(_) | DeclKind::Extension(_) => true,
                                    _ => false,
                                };
                                if valido {
                                    instancia_nao_final_nao_nula(v, unidade.ast, nomes, &aliases, &mut out, i, enum_index, nao_anulavel);
                                }
                            }
                        }
                    }
                    MemberKind::Constructor(k) if !k.factory && !k.external && k.redirect.is_none() => {
                        let (chave, primario) = match &unidade.ast.decl(id).kind {
                            DeclKind::Class(x) => (chave_de(false, x.name.sym, i, id), x.primary_constructor),
                            DeclKind::Enum(x) => (chave_de(true, x.name.sym, i, id), x.primary_constructor),
                            _ => continue,
                        };
                        // O construtor primário (sintaxe 3.13, conferida pelo
                        // oráculo 3.13) também inicializa: relata no nome da
                        // classe, como os outros.
                        let _ = primario;
                        if k.initializers.iter().any(|x| matches!(x, Initializer::Redirect { .. })) { continue; }
                        let inicializado = |nome: &SymbolId| {
                            k.parameters.iter().any(|p| p.this_ && p.name.is_some_and(|n| n.sym == *nome))
                                || k.initializers.iter().any(|x| matches!(x, Initializer::Field { name, .. } if name.sym == *nome))
                        };
                        let mut faltantes = pendentes.get(&chave).cloned().unwrap_or_default();
                        faltantes.retain(|nome| !inicializado(nome));
                        // O oráculo 3.6.2 marca só o nome da classe, inclusive
                        // em `A.named()`; o analyzer main estende até `.named`.
                        if !faltantes.is_empty() {
                            let mut nomes_faltantes: Vec<_> = faltantes.iter().map(|s| nomes.resolve(*s).to_string()).collect();
                            nomes_faltantes.sort();
                            let codigo = match nomes_faltantes.len() {
                                1 => c::FINAL_NOT_INITIALIZED_CONSTRUCTOR_1,
                                2 => c::FINAL_NOT_INITIALIZED_CONSTRUCTOR_2,
                                _ => c::FINAL_NOT_INITIALIZED_CONSTRUCTOR_3_PLUS,
                            };
                            let argumentos = match nomes_faltantes.len() {
                                1 | 2 => nomes_faltantes,
                                n => vec![nomes_faltantes[0].clone(), nomes_faltantes[1].clone(), (n - 2).to_string()],
                            };
                            out.push((i, Diagnostic::com_codigo(codigo, k.class_name.span, argumentos)));
                        }
                        let mut nao_finais = pendentes_nao_finais.get(&chave).cloned().unwrap_or_default();
                        nao_finais.retain(|nome| !inicializado(nome));
                        let mut nomes_nao_finais: Vec<_> = nao_finais.iter().map(|s| nomes.resolve(*s).to_string()).collect();
                        nomes_nao_finais.sort();
                        for nome in nomes_nao_finais {
                            out.push((i, Diagnostic::com_codigo(
                                c::NOT_INITIALIZED_NON_NULLABLE_INSTANCE_FIELD_CONSTRUCTOR,
                                k.class_name.span,
                                [nome],
                            )));
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod testes {
    use super::*;
    use dartforge_frontend::parser::parse;

    fn testar(fonte: &str) -> Vec<(String, String)> {
        let mut nomes = Interner::new();
        let parsed = parse(fonte, &mut nomes);
        finais_nao_inicializados(&[Unidade { ast: &parsed.ast, unit: &parsed.unit, fonte }], &nomes)
            .into_iter()
            .map(|(_, d)| {
                let nome = fonte[d.span.start as usize..d.span.end as usize].to_string();
                (nome, d.message)
            })
            .collect()
    }

    #[test]
    fn topo_estatico_instancia_e_excecoes_do_oraculo() {
        let fonte = "final int topo; class A { static final int estatico; final int instancia; factory A() => throw 0; } class B { final int delegado; B(); } abstract class C { abstract final int abstrato; external final int externo; late final int tardio; }";
        let mut encontrados = testar(fonte);
        encontrados.sort();
        assert_eq!(encontrados, vec![
            ("B".into(), "All final variables must be initialized, but 'delegado' isn't.".into()),
            ("estatico".into(), "The final variable 'estatico' must be initialized.".into()),
            ("instancia".into(), "The final variable 'instancia' must be initialized.".into()),
            ("topo".into(), "The final variable 'topo' must be initialized.".into()),
        ]);
    }

    #[test]
    fn lista_parcial_e_nullable_tem_mesma_regra() {
        let encontrados = testar("final Object? v; class A { static final int a = 0, b, c = 0; }");
        assert_eq!(encontrados, vec![
            ("v".into(), "The final variable 'v' must be initialized.".into()),
            ("b".into(), "The final variable 'b' must be initialized.".into()),
        ]);
    }

    #[test]
    fn tres_fontes_do_corpus_oficial() {
        for (fonte, nome) in [
            (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/variable_not_initialized/VariableNotInitialized__topLevelVariabl_dd6c7267.dart")), "v"),
            (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/variable_not_initialized/VariableNotInitialized__class_staticFie_99366e40.dart")), "v2"),
            (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/variable_not_initialized/VariableNotInitialized__class_instanceF_5d207b49.dart")), "v"),
        ] {
            assert_eq!(testar(fonte), vec![(
                nome.into(),
                format!("The final variable '{nome}' must be initialized."),
            )]);
        }
    }

    #[test]
    fn construtores_inicializam_campos_por_parametro_ou_lista() {
        let mut encontrados = testar("class A { final int x; final int y; A(this.x); A.named() : y = 0; } class B { final int v; B() : this.named(); B.named() : v = 0; }");
        encontrados.sort();
        assert_eq!(encontrados, vec![
            ("A".into(), "All final variables must be initialized, but 'x' isn't.".into()),
            ("A".into(), "All final variables must be initialized, but 'y' isn't.".into()),
        ]);
    }

    #[test]
    fn construtor_com_tres_campos_usa_variante_3_plus() {
        let encontrados = testar("class A { final int z; final int x; final int y; A(); }");
        assert_eq!(encontrados, vec![(
            "A".into(),
            "All final variables must be initialized, but 'x', 'y', and 1 others aren't.".into(),
        )]);
    }

    #[test]
    fn construtor_primario_relata_no_nome_da_classe_como_o_oraculo_313() {
        // Arquivo de sintaxe nova: o oráculo é o 3.13, que confere o primário.
        let fonte = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/variable_not_initialized/VariableNotInitialized__class_instanceF_01f4790b.dart"));
        assert_eq!(testar(fonte), vec![("A".into(), "All final variables must be initialized, but 'v2' isn't.".into())]);
    }

    #[test]
    fn construtores_secundarios_iguais_ao_oraculo_36() {
        for (fonte, offset, codigo, mensagem) in [
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/variable_not_initialized/VariableNotInitialized__class_instanceF_0811e46c.dart")),
                60,
                c::FINAL_NOT_INITIALIZED_CONSTRUCTOR_3_PLUS,
                "All final variables must be initialized, but 'v1', 'v2', and 1 others aren't.",
            ),
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/variable_not_initialized/VariableNotInitialized__class_instanceF_568a572a.dart")),
                45,
                c::FINAL_NOT_INITIALIZED_CONSTRUCTOR_1,
                "All final variables must be initialized, but 'v' isn't.",
            ),
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/variable_not_initialized/VariableNotInitialized__enum_instanceFi_5eb8f155.dart")),
                70,
                c::FINAL_NOT_INITIALIZED_CONSTRUCTOR_1,
                "All final variables must be initialized, but 'v' isn't.",
            ),
        ] {
            let mut nomes = Interner::new();
            let parsed = parse(fonte, &mut nomes);
            let achados = finais_nao_inicializados(&[Unidade { ast: &parsed.ast, unit: &parsed.unit, fonte }], &nomes);
            assert_eq!(achados.len(), 1, "{achados:?}");
            let d = &achados[0].1;
            assert_eq!(d.code, Some(codigo), "{achados:?}");
            assert_eq!(d.span.start, offset, "{achados:?}");
            assert_eq!(d.span.end, offset + 1, "{achados:?}");
            assert_eq!(d.message, mensagem, "{achados:?}");
        }
    }

    #[test]
    fn index_de_enum_usa_diagnostico_proprio() {
        let fonte = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/illegal_concrete_enum_member_declaration/IllegalConcreteEnumMemberDeclarationEnu_9000f3c7.dart"));
        assert!(testar(fonte).is_empty());
        assert!(testar("enum E { v; final int index; }").is_empty());
        assert_eq!(testar("enum E { v; static final int index; }"), vec![(
            "index".into(), "The final variable 'index' must be initialized.".into(),
        )]);
    }

    #[test]
    fn campos_de_instancia_nao_finais_nao_nulos_do_oraculo() {
        for fonte in [
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/variable_not_initialized/VariableNotInitialized__class_instanceF_87a3792e.dart")),
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/variable_not_initialized/VariableNotInitialized__mixin_instanceF_fa8a8f5a.dart")),
        ] {
            assert_eq!(testar(fonte), vec![(
                "v".into(), "Non-nullable instance field 'v' must be initialized.".into(),
            )]);
            let mut nomes = Interner::new();
            let parsed = parse(fonte, &mut nomes);
            let achados = finais_nao_inicializados(&[Unidade { ast: &parsed.ast, unit: &parsed.unit, fonte }], &nomes);
            assert_eq!(achados[0].1.code, Some(c::NOT_INITIALIZED_NON_NULLABLE_INSTANCE_FIELD));
            assert_eq!((achados[0].1.span.start, achados[0].1.span.end), (16, 17));
            assert_eq!(achados[0].1.code.unwrap().info().correcao, Some("Try adding an initializer expression, or a generative constructor that initializes it, or mark it 'late'."));
        }
        assert_eq!(testar("class A { int x; Object? y; late int z; factory A() => throw 0; }"), vec![(
            "x".into(), "Non-nullable instance field 'x' must be initialized.".into(),
        )]);
        assert!(testar("class A { int x; A(this.x); }").is_empty());
    }

    #[test]
    fn construtores_com_campos_nao_finais_nao_nulos_do_oraculo() {
        for (fonte, offset, campo) in [
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/variable_not_initialized/VariableNotInitialized__class_instanceF_50a84c0d.dart")),
                31,
                "v2",
            ),
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/variable_not_initialized/VariableNotInitialized__class_instanceF_b270bd6f.dart")),
                40,
                "v",
            ),
        ] {
            let mut nomes = Interner::new();
            let parsed = parse(fonte, &mut nomes);
            let achados = finais_nao_inicializados(&[Unidade { ast: &parsed.ast, unit: &parsed.unit, fonte }], &nomes);
            assert_eq!(achados.len(), 1, "{achados:?}");
            let d = &achados[0].1;
            assert_eq!(d.code, Some(c::NOT_INITIALIZED_NON_NULLABLE_INSTANCE_FIELD_CONSTRUCTOR));
            assert_eq!((d.span.start, d.span.end), (offset, offset + 1));
            assert_eq!(d.message, format!("Non-nullable instance field '{campo}' must be initialized."));
            assert_eq!(d.code.unwrap().info().correcao, Some("Try adding an initializer expression, or add a field initializer in this constructor, or mark it 'late'."));
        }
        assert!(testar("class A { int x; A(this.x); A.named() : x = 0; }").is_empty());
    }
}
