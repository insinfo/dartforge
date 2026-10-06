//! O `insertIntoUnitMember` com o `_InsertionPreparer` e o
//! `writeParametersMatchingArguments` do `DartFileEditBuilder` do Dart 3.6.2
//! (`change_builder_dart.dart:2202-2296`, `:2982-3131`, `:827-940`,
//! `:1221-1390`; docs/LSP-ESPECIFICACAO.md §13.7.4.0), e o
//! `CreateConstructor` (`create_constructor.dart`).
//! Escrito sem compilar nem executar (2026-10-05).

use crate::arvore_analyzer::Marca;
use crate::escrever_tipo::Escritor;
use crate::refatoracoes::Contexto;
use crate::refatoracoes_exec::{Texto, UM_RECUO, adicionar_letra, adicionar_todos, combinacoes_camel};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassKind, LibraryId, UnitId};
use dartforge_frontend::ast;
use dartforge_types::Type;
use std::collections::{BTreeSet, HashSet};

/// O filtro do último membro compatível (`lastMemberFilter`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Filtro {
    /// `insertField`.
    Campo,
    /// `insertConstructor` (com `sort_constructors_first`, só construtores).
    Construtor { construtores_primeiro: bool },
    /// `insertGetter`.
    Getter,
    /// `insertMethod`.
    Metodo,
    /// Sem filtro: o último membro.
    Nenhum,
}

/// Os prefixos de nome de método que a sugestão tira (`get`, `is`, `to`).
const PREFIXOS_CONHECIDOS: [&str; 3] = ["get", "is", "to"];

impl Contexto<'_> {
    /// Os membros (`ClassMember`) de uma declaração.
    pub(crate) fn membros_do_conteiner(&self, c: usize) -> Vec<usize> {
        self.filhos(c).iter().copied().filter(|&f| matches!(self.especie(f), "ConstructorDeclaration" | "MethodDeclaration" | "FieldDeclaration")).collect()
    }

    fn e_getter(&self, m: usize) -> bool {
        match self.arvore.nos[m].marca {
            Marca::Funcao(fid) => self.ast.function(fid).kind == ast::FunctionKind::Getter,
            _ => false,
        }
    }

    /// `insertIntoUnitMember(container, filter)`: o offset e o texto
    /// completo (prefixo, membro, sufixo) da inserção de `membro`.
    pub(crate) fn inserir_no_membro(&self, conteiner: usize, filtro: Filtro, membro: &str) -> Option<(usize, String)> {
        let tx = Texto::novo(self.fonte);
        let eol = tx.eol();
        let membros = self.membros_do_conteiner(conteiner);
        let passa = |m: usize| match filtro {
            Filtro::Campo => self.especie(m) == "FieldDeclaration",
            Filtro::Construtor { construtores_primeiro: true } => self.especie(m) == "ConstructorDeclaration",
            Filtro::Construtor { construtores_primeiro: false } => matches!(self.especie(m), "ConstructorDeclaration" | "FieldDeclaration"),
            Filtro::Getter => matches!(self.especie(m), "FieldDeclaration" | "ConstructorDeclaration") || (self.especie(m) == "MethodDeclaration" && self.e_getter(m)),
            Filtro::Metodo => matches!(self.especie(m), "FieldDeclaration" | "ConstructorDeclaration" | "MethodDeclaration"),
            Filtro::Nenhum => true,
        };
        let alvo = membros.iter().rev().copied().find(|&m| passa(m));
        let enum_ = self.especie(conteiner) == "EnumDeclaration";
        let constantes = self.filhos_da_especie(conteiner, "EnumConstantDeclaration");
        // O `;` das constantes.
        let ponto_e_virgula = if enum_ {
            constantes.last().and_then(|&c| {
                let mut t = self.token_seguinte(self.arvore.nos[c].fim)?;
                if &self.fonte[t.start..t.end] == "," {
                    t = self.token_seguinte(t.end)?;
                }
                (&self.fonte[t.start..t.end] == ";").then_some(t)
            })
        } else {
            None
        };
        // O `{` da declaração (o primeiro depois do cabeçalho).
        let chave = {
            let mut pos = self.arvore.nos[conteiner].inicio;
            let limite = membros.first().map_or(self.arvore.nos[conteiner].fim, |&m| self.arvore.nos[m].inicio).min(constantes.first().map_or(usize::MAX, |&c| self.arvore.nos[c].inicio));
            let mut achada = None;
            while let Some(t) = self.token_seguinte(pos) {
                if t.start >= limite {
                    break;
                }
                if &self.fonte[t.start..t.end] == "{" {
                    achada = Some(t);
                }
                pos = t.end;
            }
            achada
        };
        let offset = match alvo {
            Some(m) => self.arvore.nos[m].fim,
            None if enum_ => match ponto_e_virgula {
                Some(p) => p.end,
                None => self.arvore.nos[*constantes.last()?].fim,
            },
            None => chave?.end,
        };
        let mut s = String::new();
        if enum_ && ponto_e_virgula.is_none() && alvo.is_none() {
            s.push(';');
        }
        if alvo.is_some() || (enum_ && !constantes.is_empty()) {
            s.push_str(eol);
            s.push_str(eol);
        } else {
            s.push_str(eol);
        }
        s.push_str(UM_RECUO);
        s.push_str(membro);
        // O sufixo.
        if alvo.is_none() && !(enum_ && !constantes.is_empty()) {
            if !membros.is_empty() {
                s.push_str(eol);
            } else {
                let inicio = self.primeiro_token_apos_comentario_e_metadados(conteiner).map_or(self.arvore.nos[conteiner].inicio, |t| t.start);
                let linha_inicio = self.fonte[..inicio].matches('\n').count();
                let linha_fim = self.fonte[..self.arvore.nos[conteiner].fim].matches('\n').count();
                if linha_inicio == linha_fim {
                    s.push_str(eol);
                }
            }
        }
        Some((offset, s))
    }

    /// `_getBaseNameFromExpression`.
    fn nome_base_da_expressao_ins(&self, e: usize) -> Option<String> {
        match self.especie(e) {
            "AsExpression" | "ParenthesizedExpression" => self.nome_base_da_expressao_ins(*self.filhos(e).first()?),
            _ => self.nome_base_desembrulhado(e),
        }
    }

    /// `_getBaseNameFromUnwrappedExpression`.
    fn nome_base_desembrulhado(&self, e: usize) -> Option<String> {
        let mut nome: Option<String> = None;
        match self.especie(e) {
            "SimpleIdentifier" => return Some(self.texto_do_no(e).to_string()),
            "PrefixedIdentifier" => return Some(self.texto_do_no(*self.filhos(e).get(1)?).to_string()),
            "PropertyAccess" => return Some(self.texto_do_no(*self.filhos(e).last()?).to_string()),
            "MethodInvocation" => nome = self.nome_do_metodo(e).map(|m| self.texto_do_no(m).to_string()),
            "InstanceCreationExpression" => {
                let cn = self.filhos_da_especie(e, "ConstructorName").first().copied()?;
                let tipo = self.filhos_da_especie(cn, "NamedType").first().copied()?;
                // O `name2` do tipo: o identificador depois do prefixo.
                let fim_prefixo = self.filhos_da_especie(tipo, "ImportPrefixReference").first().map_or(self.arvore.nos[tipo].inicio, |&p| self.arvore.nos[p].fim);
                let s = crate::projeto::palavra(self.fonte, self.token_seguinte(fim_prefixo)?.start)?;
                return Some(self.fonte[s.start..s.end].to_string());
            }
            "IndexExpression" => {
                nome = self.nome_base_da_expressao_ins(*self.filhos(e).first()?);
                if let Some(n) = &nome {
                    if let Some(x) = n.strip_suffix("es") {
                        nome = Some(x.to_string());
                    } else if let Some(x) = n.strip_suffix('s') {
                        nome = Some(x.to_string());
                    }
                }
            }
            _ => {}
        }
        if let Some(n) = &nome {
            for p in PREFIXOS_CONHECIDOS {
                if n.starts_with(p) {
                    if n == p {
                        return None;
                    } else if n.as_bytes().get(p.len()).is_some_and(|c| c.is_ascii_uppercase()) {
                        return Some(n[p.len()..].to_string());
                    }
                }
            }
        }
        nome
    }

    /// `_getParameterNameSuggestions(usedNames, type, expression, index)`.
    fn sugestoes_de_parametro(&self, usados: &HashSet<String>, tipo: Option<&Type>, arg: usize, indice: usize) -> Vec<String> {
        let mut res: Vec<String> = Vec::new();
        if let Some(n) = self.nome_base_da_expressao_ins(arg) {
            let n = n.strip_prefix('_').unwrap_or(&n).to_string();
            adicionar_todos(usados, &mut res, combinacoes_camel(&n), None);
        }
        // `_getBaseNameFromLocationInParent`: o rótulo do nomeado (o
        // parâmetro de um posicional não existe: a chamada não resolve).
        if let Some(p) = self.pai(arg)
            && self.especie(p) == "NamedExpression"
            && self.filhos(p).get(1) == Some(&arg)
            && let Some(&rotulo) = self.filhos(p).first()
            && let Some(&id) = self.filhos(rotulo).first()
        {
            adicionar_todos(usados, &mut res, combinacoes_camel(self.texto_do_no(id)), None);
        }
        let core = &self.p.consulta.core;
        match tipo {
            Some(Type::Interface { class, .. }) if Some(*class) == core.int_class => adicionar_letra(usados, &mut res, b'i'),
            Some(Type::Interface { class, .. }) if Some(*class) == core.double_class => adicionar_letra(usados, &mut res, b'd'),
            Some(Type::Interface { class, .. }) if Some(*class) == core.string_class => adicionar_letra(usados, &mut res, b's'),
            Some(Type::Interface { class, .. }) => {
                let nome = self.p.nome(self.p.programa().class(*class).name).to_string();
                adicionar_todos(usados, &mut res, combinacoes_camel(&nome), None);
            }
            _ => {}
        }
        if res.is_empty() {
            return vec![format!("param{indice}")];
        }
        res
    }

    /// `writeParametersMatchingArguments(argumentList)`: os tipos vêm deste
    /// contexto e são escritos pelo `escritor` (o do arquivo de destino).
    pub(crate) fn parametros_para_argumentos(&self, lista: usize, escritor: &mut Escritor<'_, '_>) -> String {
        let mut usados: HashSet<String> = HashSet::new();
        let mut s = String::new();
        let mut nomeados = false;
        let argumentos: Vec<usize> = self.filhos(lista).to_vec();
        for (i, &arg) in argumentos.iter().enumerate() {
            if i > 0 {
                s.push_str(", ");
            }
            let nomeado = self.especie(arg) == "NamedExpression";
            if nomeado && !nomeados {
                nomeados = true;
                s.push('{');
            }
            // `writeParameterMatchingArgument`.
            let valor = if nomeado { self.filhos(arg).get(1).copied().unwrap_or(arg) } else { arg };
            let tabela = &self.p.consulta.tabela;
            let tipo = self.tipo_do_no(valor).filter(|t| !matches!(tabela.get(*t), Type::Never | Type::Null) && !matches!(tabela.exibicao(*t), Some(dartforge_types::table::Exibicao::NeverAnulavel)));
            let sem_sufixo = tipo.is_none_or(|t| !tabela.get(t).is_declared_nullable());
            if nomeado && sem_sufixo {
                s.push_str("required ");
            }
            if let Some(t) = escritor.escrever_tipo(tipo, false) {
                s.push_str(&t);
                s.push(' ');
            }
            if nomeado {
                let rotulo = self.filhos(arg).first().and_then(|&r| self.filhos(r).first().copied());
                s.push_str(rotulo.map_or("", |id| self.texto_do_no(id)));
            } else {
                let sugestoes = self.sugestoes_de_parametro(&usados, tipo.map(|t| tabela.get(t)), valor, i);
                let favorita = sugestoes[0].clone();
                usados.insert(favorita.clone());
                s.push_str(&favorita);
            }
        }
        if nomeados {
            s.push('}');
        }
        s
    }
}

/// O resultado do `CreateConstructor`: o nome (`{0}`), as edições por
/// unidade e as bibliotecas a importar na de destino.
pub(crate) struct Construtor {
    pub(crate) nome: String,
    pub(crate) unidade: UnitId,
    pub(crate) edicao: (Span, String),
    pub(crate) importar: BTreeSet<LibraryId>,
}

/// `CreateConstructor._proposeFromConstructorName` (o caminho do
/// `new_with_undefined_constructor`): o `node` é o nome do construtor numa
/// `InstanceCreationExpression` (`new A.b()`); a classe (só
/// `ClassDeclaration`) ganha `A.b(<parâmetros>);` pelo `insertConstructor`.
pub(crate) fn criar_construtor(cx: &Contexto<'_>, erro: Span) -> Option<Construtor> {
    let node = cx.arvore.localizar(erro.start, erro.end)?;
    if cx.especie(node) != "SimpleIdentifier" {
        return None;
    }
    let cn = cx.pai(node).filter(|&c| cx.especie(c) == "ConstructorName")?;
    if cx.filhos(cn).get(1) != Some(&node) {
        return None;
    }
    let criacao = cx.pai(cn).filter(|&c| cx.especie(c) == "InstanceCreationExpression")?;
    let nome = cx.texto_do_no(cn).split_whitespace().collect::<String>();
    // A classe do tipo.
    let ty = cx.ast.types.iter().enumerate().find(|(_, t)| t.span.start == cx.arvore.nos[cn].inicio).map(|(i, _)| ast::TypeId(i as u32))?;
    let classe = cx.p.classe_do_tipo(cx.unidade, ty)?;
    let prog = cx.p.programa();
    let c = prog.class(classe);
    if c.kind != ClassKind::Class {
        return None;
    }
    let decl = c.decl?;
    let destino = crate::refatoracoes::Contexto::novo(cx.p, decl.unit);
    let conteiner = destino.arvore.nos.iter().position(|n| n.especie == "ClassDeclaration" && matches!(n.marca, Marca::Decl(d) if d == decl.decl))?;
    let construtores_primeiro = crate::refatoracoes_exec::regra_ligada(cx.p, decl.unit, "sort_constructors_first");
    let lista = cx.filhos_da_especie(criacao, "ArgumentList").first().copied()?;
    // O ponto de inserção (para o escritor de tipos) antes de escrever.
    let (offset, _) = destino.inserir_no_membro(conteiner, Filtro::Construtor { construtores_primeiro }, "")?;
    let mut escritor = Escritor::novo(&destino, offset);
    let parametros = cx.parametros_para_argumentos(lista, &mut escritor);
    let nome_da_classe = cx.p.nome(c.name).to_string();
    let membro = format!("{nome_da_classe}.{}({parametros});", cx.texto_do_no(node));
    let (offset, texto) = destino.inserir_no_membro(conteiner, Filtro::Construtor { construtores_primeiro }, &membro)?;
    Some(Construtor { nome, unidade: decl.unit, edicao: (Span { start: offset, end: offset }, texto), importar: escritor.importar })
}
