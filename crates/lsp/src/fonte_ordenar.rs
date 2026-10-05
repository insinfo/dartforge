//! `Sort Members` (`dart.edit.sortMembers`): o `MemberSorter` do servidor
//! do Dart 3.6.2 (`AS:src/services/correction/sort_members.dart:14-278`;
//! docs/LSP-ESPECIFICACAO.md §13.12.5).
//!
//! Ordena os membros de cada classe, enum, mixin, extensão e extension type
//! e depois as declarações de topo, pela tabela de prioridades do original;
//! dentro de um grupo, pelo nome em minúsculas (campos de classe mantêm a
//! ordem do arquivo). Cada membro leva os comentários que o precedem e o
//! comentário de fim de linha; o que está entre os membros (linhas em
//! branco, indentação) não se move. O resultado é **uma** edição, do
//! primeiro ao último caractere diferentes (`computeSimpleDiff`).
//!
//! O passo final é o do original: as diretivas reordenadas pelo
//! `ImportOrganizer` sem remoção (`crate::fonte_imports`, §13.12.6).
//! Escrito sem compilar nem executar (2026-10-04).

use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{Ast, CompilationUnit, DeclKind, FunctionKind, MemberId, MemberKind, TypedefKind};
use dartforge_intern::Interner;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Especie {
    UnitFunctionMain,
    UnitVariableConst,
    UnitVariable,
    UnitAccessor,
    UnitFunction,
    UnitGenericTypeAlias,
    UnitFunctionType,
    UnitClass,
    UnitExtensionType,
    UnitExtension,
    ClassField,
    ClassAccessor,
    ClassConstructor,
    ClassMethod,
}

/// `_PRIORITY_ITEMS` (`sort_members.dart:15-47`): `(estático, espécie,
/// privado)`; para os campos de classe o terceiro não entra na igualdade.
const PRIORIDADES: &[(bool, Especie, bool)] = &[
    (false, Especie::UnitFunctionMain, false),
    (false, Especie::UnitVariableConst, false),
    (false, Especie::UnitVariableConst, true),
    (false, Especie::UnitVariable, false),
    (false, Especie::UnitVariable, true),
    (false, Especie::UnitAccessor, false),
    (false, Especie::UnitAccessor, true),
    (false, Especie::UnitFunction, false),
    (false, Especie::UnitFunction, true),
    (false, Especie::UnitGenericTypeAlias, false),
    (false, Especie::UnitGenericTypeAlias, true),
    (false, Especie::UnitFunctionType, false),
    (false, Especie::UnitFunctionType, true),
    (false, Especie::UnitClass, false),
    (false, Especie::UnitClass, true),
    (false, Especie::UnitExtensionType, false),
    (false, Especie::UnitExtensionType, true),
    (false, Especie::UnitExtension, false),
    (false, Especie::UnitExtension, true),
    (true, Especie::ClassField, false),
    (true, Especie::ClassAccessor, false),
    (true, Especie::ClassAccessor, true),
    (false, Especie::ClassField, false),
    (false, Especie::ClassConstructor, false),
    (false, Especie::ClassConstructor, true),
    (false, Especie::ClassAccessor, false),
    (false, Especie::ClassAccessor, true),
    (false, Especie::ClassMethod, false),
    (false, Especie::ClassMethod, true),
    (true, Especie::ClassMethod, false),
    (true, Especie::ClassMethod, true),
];

/// Um membro a ordenar: a chave e o intervalo do texto dele (com os
/// comentários que o acompanham) na fonte ORIGINAL.
struct Item {
    especie: Especie,
    estatico: bool,
    nome: String,
    inicio: usize,
    fim: usize,
}

impl Item {
    /// `_getPriority`: o índice do primeiro item igual da tabela.
    fn prioridade(&self) -> usize {
        let privado = self.nome.starts_with('_');
        PRIORIDADES
            .iter()
            .position(|&(estatico, especie, p)| {
                especie == self.especie && estatico == self.estatico && (especie == Especie::ClassField || p == privado)
            })
            .unwrap_or(0)
    }
}

/// Um comentário num vão entre dois membros.
pub(crate) struct Comentario {
    pub(crate) inicio: usize,
    pub(crate) fim: usize,
    /// `///` ou `/** */`.
    pub(crate) de_documentacao: bool,
}

/// Os comentários de `texto[de..ate]`, que só tem brancos e comentários.
pub(crate) fn comentarios(texto: &str, de: usize, ate: usize) -> Vec<Comentario> {
    let b = texto.as_bytes();
    let mut saida = Vec::new();
    let mut i = de;
    while i + 1 < ate {
        if b[i] == b'/' && b[i + 1] == b'/' {
            let fim = texto[i..ate].find('\n').map_or(ate, |k| i + k);
            let fim_sem_cr = if fim > i && b[fim - 1] == b'\r' { fim - 1 } else { fim };
            let de_documentacao = b.get(i + 2) == Some(&b'/') && b.get(i + 3) != Some(&b'/');
            saida.push(Comentario { inicio: i, fim: fim_sem_cr, de_documentacao });
            i = fim;
        } else if b[i] == b'/' && b[i + 1] == b'*' {
            let mut nivel = 1;
            let mut j = i + 2;
            while j < ate && nivel > 0 {
                if j + 1 < ate && b[j] == b'/' && b[j + 1] == b'*' {
                    nivel += 1;
                    j += 2;
                } else if j + 1 < ate && b[j] == b'*' && b[j + 1] == b'/' {
                    nivel -= 1;
                    j += 2;
                } else {
                    j += 1;
                }
            }
            let de_documentacao = b.get(i + 2) == Some(&b'*') && b.get(i + 3) != Some(&b'/');
            saida.push(Comentario { inicio: i, fim: j.min(ate), de_documentacao });
            i = j;
        } else {
            i += 1;
        }
    }
    saida
}

/// A linha (base 0) do offset.
pub(crate) fn linha_de(texto: &str, offset: usize) -> usize {
    texto.as_bytes()[..offset.min(texto.len())].iter().filter(|c| **c == b'\n').count()
}

/// `range.nodeWithComments`: o intervalo do texto de um nó, com os
/// comentários da frente e o de fim de linha. `anterior`: o fim do token
/// anterior ao nó (`None` quando o nó é a primeira coisa do arquivo).
fn com_comentarios(texto: &str, no: Span, anterior: Option<usize>) -> (usize, usize) {
    let inicio = match anterior {
        // A primeira coisa do arquivo: os comentários de antes são cabeçalho,
        // salvo o comentário de documentação do próprio nó.
        None => {
            let cs = comentarios(texto, 0, no.start);
            inicio_da_documentacao(&cs).unwrap_or(no.start)
        }
        Some(fim_anterior) => {
            let cs = comentarios(texto, fim_anterior, no.start);
            match inicio_da_documentacao(&cs) {
                // Com comentário de documentação, o nó começa nele: os
                // comentários comuns acima ficam onde estão.
                Some(i) => i,
                None => {
                    // Os que começam na linha do token anterior são o
                    // comentário de fim de linha dele.
                    let linha_anterior = linha_de(texto, fim_anterior.saturating_sub(1));
                    cs.iter().find(|c| linha_de(texto, c.inicio) != linha_anterior).map_or(no.start, |c| c.inicio)
                }
            }
        }
    };
    // O comentário de fim de linha: os que começam na linha em que começa o
    // último token do nó (aqui, a linha do último caractere dele).
    let linha_do_fim = linha_de(texto, no.end.saturating_sub(1));
    let fim_da_linha = texto[no.end.min(texto.len())..].find('\n').map_or(texto.len(), |k| no.end + k);
    let mut fim = no.end;
    for c in comentarios(texto, no.end.min(texto.len()), fim_da_linha) {
        // Só comentários separados do nó por brancos (nada de código no meio).
        if linha_de(texto, c.inicio) == linha_do_fim && texto[fim..c.inicio].trim().is_empty() {
            fim = c.fim;
        } else {
            break;
        }
    }
    (inicio, fim)
}

/// O começo do comentário de documentação de um nó: a última sequência de
/// linhas `///` consecutivas, ou o último bloco `/** */`, do vão.
fn inicio_da_documentacao(cs: &[Comentario]) -> Option<usize> {
    let ultimo = cs.iter().rposition(|c| c.de_documentacao)?;
    let mut primeiro = ultimo;
    while primeiro > 0 && cs[primeiro - 1].de_documentacao {
        primeiro -= 1;
    }
    Some(cs[primeiro].inicio)
}

/// `_getSortedMembers`: prioridade; campos de classe pela posição; o resto
/// pelo nome em minúsculas, em unidades UTF-16. Estável.
fn ordenar(itens: &[Item]) -> Vec<usize> {
    let mut ordem: Vec<usize> = (0..itens.len()).collect();
    ordem.sort_by(|&x, &y| {
        let (a, b) = (&itens[x], &itens[y]);
        a.prioridade().cmp(&b.prioridade()).then_with(|| {
            if a.especie == Especie::ClassField {
                a.inicio.cmp(&b.inicio)
            } else {
                a.nome.to_lowercase().encode_utf16().cmp(b.nome.to_lowercase().encode_utf16())
            }
        })
    });
    ordem
}

/// `_sortAndReorderMembers`: o k-ésimo encaixe recebe o texto do k-ésimo
/// membro ordenado, de trás para a frente. Os intervalos são os da fonte
/// original; como cada troca preserva o comprimento total do contêiner,
/// continuam válidos em `codigo` para os encaixes anteriores.
fn reordenar(codigo: &mut String, itens: &[Item]) {
    let ordem = ordenar(itens);
    if ordem.iter().enumerate().all(|(i, &j)| i == j) {
        return;
    }
    let textos: Vec<String> = itens.iter().map(|it| codigo[it.inicio..it.fim].to_string()).collect();
    // O deslocamento acumulado dos encaixes à esquerda muda os offsets dos
    // da direita: monta-se o trecho inteiro de uma vez.
    let (primeiro, ultimo) = (itens[0].inicio, itens[itens.len() - 1].fim);
    let mut novo = String::with_capacity(ultimo - primeiro);
    let mut cursor = primeiro;
    for (i, it) in itens.iter().enumerate() {
        novo.push_str(&codigo[cursor..it.inicio]);
        novo.push_str(&textos[ordem[i]]);
        cursor = it.fim;
    }
    codigo.replace_range(primeiro..ultimo, &novo);
}

fn membros_de(decl: &DeclKind) -> Option<&[MemberId]> {
    match decl {
        DeclKind::Class(x) => Some(&x.members),
        DeclKind::Enum(x) => Some(&x.members),
        DeclKind::Mixin(x) => Some(&x.members),
        DeclKind::Extension(x) => Some(&x.members),
        DeclKind::ExtensionType(x) => Some(&x.members),
        _ => None,
    }
}

/// Os itens dos membros de uma classe; `None` abandona a ordenação dela
/// (um campo sem variável).
fn itens_da_classe(texto: &str, ast: &Ast, nomes: &Interner, abertura: usize, membros: &[MemberId]) -> Option<Vec<Item>> {
    let mut itens = Vec::with_capacity(membros.len());
    let mut anterior: Option<usize> = None;
    for &mid in membros {
        let membro = ast.member(mid);
        let (especie, estatico, nome) = match &membro.kind {
            MemberKind::Constructor(k) => (Especie::ClassConstructor, false, k.name.map_or(String::new(), |n| nomes.resolve(n.sym).to_string())),
            MemberKind::Field(l) => {
                let primeira = l.variables.first()?;
                (Especie::ClassField, l.static_, nomes.resolve(primeira.name.sym).to_string())
            }
            MemberKind::Method(f) => {
                let func = ast.function(*f);
                let base = func.name.map_or(String::new(), |n| nomes.resolve(n.sym).to_string());
                match func.kind {
                    FunctionKind::Getter => (Especie::ClassAccessor, func.static_, format!("{base} getter")),
                    FunctionKind::Setter => (Especie::ClassAccessor, func.static_, format!("{base} setter")),
                    FunctionKind::Function | FunctionKind::Operator => (Especie::ClassMethod, func.static_, base),
                }
            }
        };
        let fim_anterior = anterior.unwrap_or(abertura.min(membro.span.start));
        let (inicio, fim) = com_comentarios(texto, membro.span, Some(fim_anterior));
        itens.push(Item { especie, estatico, nome, inicio, fim });
        anterior = Some(membro.span.end);
    }
    Some(itens)
}

/// `MemberSorter.sort`: a edição única
/// `(offset, comprimento, texto novo)` em bytes da fonte, ou `None` quando
/// nada muda.
pub(crate) fn ordenar_membros(texto: &str, unidade: &CompilationUnit, ast: &Ast, nomes: &Interner) -> Option<(usize, usize, String)> {
    let mut codigo = texto.to_string();
    // 1. Os membros de cada classe, na ordem do arquivo.
    for &d in unidade.declarations.iter() {
        let decl = ast.decl(d);
        let Some(membros) = membros_de(&decl.kind) else { continue };
        if membros.len() < 2 {
            continue;
        }
        // O fim do token anterior ao primeiro membro: o `;` das constantes
        // de um enum, ou o `{` do corpo.
        let primeiro = ast.member(membros[0]).span.start;
        let depois_das_constantes = match &decl.kind {
            DeclKind::Enum(x) => x.constants.last().and_then(|k| {
                let de = k.span.end.min(primeiro);
                texto[de..primeiro].find(';').map(|i| de + i + 1)
            }),
            _ => None,
        };
        let abertura = depois_das_constantes
            .or_else(|| texto.get(decl.span.start..primeiro).and_then(|t| t.rfind('{')).map(|i| decl.span.start + i + 1))
            .unwrap_or(primeiro);
        if let Some(itens) = itens_da_classe(texto, ast, nomes, abertura, membros) {
            reordenar(&mut codigo, &itens);
        }
    }
    // 2. As declarações de topo; os textos saem do código já com os membros
    // ordenados.
    let mut itens: Vec<Item> = Vec::new();
    let mut anterior: Option<usize> = unidade.directives.last().map(|x| x.span.end);
    let mut abandonar = false;
    for &d in unidade.declarations.iter() {
        let decl = ast.decl(d);
        let texto_de = |n: dartforge_frontend::ast::Name| nomes.resolve(n.sym).to_string();
        let (especie, nome) = match &decl.kind {
            DeclKind::Class(x) => (Especie::UnitClass, texto_de(x.name)),
            DeclKind::Enum(x) => (Especie::UnitClass, texto_de(x.name)),
            DeclKind::Mixin(x) => (Especie::UnitClass, texto_de(x.name)),
            DeclKind::ExtensionType(x) => (Especie::UnitExtensionType, texto_de(x.name)),
            DeclKind::Extension(x) => (Especie::UnitExtension, x.name.map_or(String::new(), texto_de)),
            DeclKind::Function(f) => {
                let func = ast.function(*f);
                let base = func.name.map_or(String::new(), texto_de);
                match func.kind {
                    FunctionKind::Getter => (Especie::UnitAccessor, format!("{base} getter")),
                    FunctionKind::Setter => (Especie::UnitAccessor, format!("{base} setter")),
                    _ if base == "main" => (Especie::UnitFunctionMain, base),
                    _ => (Especie::UnitFunction, base),
                }
            }
            DeclKind::Typedef(x) => match x.kind {
                TypedefKind::Legacy { .. } => (Especie::UnitFunctionType, texto_de(x.name)),
                TypedefKind::Alias(_) => (Especie::UnitGenericTypeAlias, texto_de(x.name)),
            },
            DeclKind::Variables(l) => match l.variables.first() {
                Some(primeira) => (if l.const_ { Especie::UnitVariableConst } else { Especie::UnitVariable }, texto_de(primeira.name)),
                None => {
                    abandonar = true;
                    break;
                }
            },
        };
        let (inicio, fim) = com_comentarios(texto, decl.span, anterior);
        itens.push(Item { especie, estatico: false, nome, inicio, fim });
        anterior = Some(decl.span.end);
    }
    if !abandonar && itens.len() >= 2 {
        reordenar(&mut codigo, &itens);
    }
    // 3. As diretivas, por último: só reordena e reagrupa (sem erros e sem
    // remoção). Os offsets delas são os originais: as trocas acima só mexem
    // do primeiro encaixe de declaração em diante.
    if let Some(novo) = crate::fonte_imports::organizar(&codigo, unidade, &[], false, false) {
        codigo = novo;
    }
    diferenca_simples(texto, &codigo)
}

/// `computeSimpleDiff` (`AS:src/utilities/strings.dart:36-49`): o maior
/// prefixo e o maior sufixo comuns; a edição é o miolo.
fn diferenca_simples(antigo: &str, novo: &str) -> Option<(usize, usize, String)> {
    if antigo == novo {
        return None;
    }
    let (a, n) = (antigo.as_bytes(), novo.as_bytes());
    let mut prefixo = a.iter().zip(n.iter()).take_while(|(x, y)| x == y).count();
    while !antigo.is_char_boundary(prefixo) || !novo.is_char_boundary(prefixo) {
        prefixo -= 1;
    }
    let maximo = a.len().min(n.len()) - prefixo;
    let mut sufixo = a.iter().rev().zip(n.iter().rev()).take(maximo).take_while(|(x, y)| x == y).count();
    while !antigo.is_char_boundary(a.len() - sufixo) || !novo.is_char_boundary(n.len() - sufixo) {
        sufixo -= 1;
    }
    Some((prefixo, a.len() - prefixo - sufixo, novo[prefixo..n.len() - sufixo].to_string()))
}

#[cfg(test)]
mod testes {
    use super::*;

    fn ordenado(fonte: &str) -> String {
        let mut nomes = Interner::new();
        let p = dartforge_frontend::parser::parse(fonte, &mut nomes);
        match ordenar_membros(fonte, &p.unit, &p.ast, &nomes) {
            Some((offset, comprimento, novo)) => format!("{}{}{}", &fonte[..offset], novo, &fonte[offset + comprimento..]),
            None => fonte.to_string(),
        }
    }

    /// Campos na ordem do arquivo, o comentário de cima e o de fim de linha
    /// acompanham o membro, o construtor sem nome antes do nomeado.
    #[test]
    fn exemplo_da_especificacao() {
        let antes = "class A {\n  void b() {}\n  // sobre o campo\n  final int z = 1; // fim\n  A.named();\n  static int s = 0;\n  int a = 2;\n  A();\n  int get x => 0;\n}\n";
        let depois = "class A {\n  static int s = 0;\n  // sobre o campo\n  final int z = 1; // fim\n  int a = 2;\n  A();\n  A.named();\n  int get x => 0;\n  void b() {}\n}\n";
        assert_eq!(ordenado(antes), depois);
    }

    #[test]
    fn topo_com_main_primeiro_e_classes_depois_das_funcoes() {
        let antes = "class B {}\nvoid z() {}\nvoid main() {}\nconst k = 1;\n";
        let depois = "void main() {}\nconst k = 1;\nvoid z() {}\nclass B {}\n";
        assert_eq!(ordenado(antes), depois);
    }

    #[test]
    fn enum_com_comentario_entre_as_constantes() {
        let antes = "enum E {\n  a, // primeira\n  b;\n\n  void z() {}\n  void y() {}\n}\n";
        let depois = "enum E {\n  a, // primeira\n  b;\n\n  void y() {}\n  void z() {}\n}\n";
        assert_eq!(ordenado(antes), depois);
    }

    #[test]
    fn diretivas_reordenadas_sem_remover_duplicatas() {
        let antes = "import 'b.dart';\nimport 'dart:io';\nimport 'b.dart';\n\nvoid main() {}\n";
        let depois = "import 'dart:io';\n\nimport 'b.dart';\nimport 'b.dart';\n\nvoid main() {}\n";
        assert_eq!(ordenado(antes), depois);
    }

    #[test]
    fn ja_ordenado_nao_tem_edicao() {
        let mut nomes = Interner::new();
        let fonte = "void main() {}\nclass A {}\n";
        let p = dartforge_frontend::parser::parse(fonte, &mut nomes);
        assert!(ordenar_membros(fonte, &p.unit, &p.ast, &nomes).is_none());
    }
}
