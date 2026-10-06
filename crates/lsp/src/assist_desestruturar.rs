//! `DestructureLocalVariableAssignment`
//! (destructure_local_variable_assignment.dart) do `analysis_server` 3.6.2:
//! o nome de uma variável local de tipo record vira um padrão de record; o de
//! tipo de classe, cujas leituras são só de propriedades, vira um padrão de
//! objeto, e cada leitura `v.p` passa a ser a variável do padrão. As posições
//! ligadas do produtor saem como o texto delas.
//!
//! | Título | Espécie |
//! |---|---|
//! | `Destructure variable assignment` | `refactor.destructureLocalVariableAssignment` |

use crate::acoes::AcaoDeCodigo;
use crate::arvore_analyzer::Marca;
use crate::refatoracoes::Contexto;
use crate::refatoracoes_exec::nomes_para_texto;
use crate::Edicao;
use dartforge_diagnostics::Span;
use dartforge_types::{Resolved, Type};
use std::collections::HashSet;

impl Contexto<'_> {
    /// O nome de um parâmetro formal (o último token dele, por dentro do
    /// `DefaultFormalParameter`).
    fn nome_do_parametro(&self, p: usize) -> Option<Span> {
        let p = if self.especie(p) == "DefaultFormalParameter" { *self.filhos(p).first()? } else { p };
        let depois = self.filhos(p).iter().rev().find(|&&k| self.especie(k) == "FormalParameterList").map_or(self.arvore.nos[p].fim, |&l| self.arvore.nos[l].inicio);
        self.token_anterior(depois)
    }

    /// `ScopedNameFinder(posicao)` aplicado a `n`: os nomes de locais e
    /// parâmetros visíveis em `n`, declarados antes da posição.
    pub(crate) fn nomes_em_escopo(&self, n: usize, posicao: usize) -> HashSet<String> {
        let mut nomes = HashSet::new();
        let mut adicionar = |s: Option<Span>, nomes: &mut HashSet<String>| {
            if let Some(s) = s
                && s.end < posicao
            {
                nomes.insert(self.fonte[s.start..s.end].to_string());
            }
        };
        let nome_da_variavel = |v: usize| self.token_seguinte(self.arvore.nos[v].inicio);
        let mut em_funcao_local = false;
        let mut filho = n;
        let mut atual = Some(n);
        while let Some(k) = atual {
            let parametros_fora_do_filho = |nomes: &mut HashSet<String>, adicionar: &mut dyn FnMut(Option<Span>, &mut HashSet<String>)| {
                if let Some(&lista) = self.filhos(k).iter().find(|&&f| self.especie(f) == "FormalParameterList")
                    && lista != filho
                {
                    for &p in self.filhos(lista) {
                        adicionar(self.nome_do_parametro(p), nomes);
                    }
                }
            };
            match self.especie(k) {
                "Block" | "SwitchCase" | "SwitchPatternCase" | "SwitchDefault" => {
                    // `_checkStatements`: os comandos antes do filho imediato.
                    for &c in self.filhos(k) {
                        if c == filho {
                            break;
                        }
                        match self.especie(c) {
                            "VariableDeclarationStatement" => {
                                if let Some(&l) = self.filhos(c).first() {
                                    for &v in self.filhos(l).iter().filter(|&&v| self.especie(v) == "VariableDeclaration") {
                                        adicionar(nome_da_variavel(v), &mut nomes);
                                    }
                                }
                            }
                            "FunctionDeclarationStatement" if !em_funcao_local => {
                                if let Some(&f) = self.filhos(c).first()
                                    && let Marca::Funcao(fid) = self.arvore.nos[f].marca
                                {
                                    adicionar(self.ast.function(fid).name.map(|x| x.span), &mut nomes);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                "CatchClause" => {
                    for &p in self.filhos(k).iter().filter(|&&p| self.especie(p) == "CatchClauseParameter") {
                        adicionar(Some(self.arvore.span(p)), &mut nomes);
                    }
                }
                "ConstructorDeclaration" => {
                    parametros_fora_do_filho(&mut nomes, &mut adicionar);
                    break;
                }
                "FieldDeclaration" | "TopLevelVariableDeclaration" | "FunctionTypeAlias" | "GenericTypeAlias" | "ClassTypeAlias" => break,
                "ForEachPartsWithDeclaration" => {
                    if let Some(&d) = self.filhos(k).iter().find(|&&f| self.especie(f) == "DeclaredIdentifier") {
                        adicionar(self.token_anterior(self.arvore.nos[d].fim), &mut nomes);
                    }
                }
                "ForPartsWithDeclarations" => {
                    if let Some(&l) = self.filhos(k).iter().find(|&&f| self.especie(f) == "VariableDeclarationList") {
                        for &v in self.filhos(l).iter().filter(|&&v| self.especie(v) == "VariableDeclaration") {
                            adicionar(nome_da_variavel(v), &mut nomes);
                        }
                    }
                }
                "FunctionDeclaration" => {
                    if self.pai(k).is_none_or(|p| self.especie(p) != "FunctionDeclarationStatement") {
                        break;
                    }
                }
                "FunctionDeclarationStatement" => em_funcao_local = true,
                "FunctionExpression" => parametros_fora_do_filho(&mut nomes, &mut adicionar),
                "MethodDeclaration" => {
                    parametros_fora_do_filho(&mut nomes, &mut adicionar);
                    break;
                }
                _ => {}
            }
            filho = k;
            atual = self.pai(k);
        }
        nomes
    }

    /// As leituras do local declarado em `nome` dentro de `corpo`, na ordem
    /// do texto: `(propriedade, nó do acesso)` para `v.p`, e `None` para uma
    /// leitura do objeto inteiro.
    fn leituras_do_local(&self, corpo: usize, nome: usize) -> Vec<Option<(String, usize)>> {
        let mut v = Vec::new();
        let mut pilha = vec![corpo];
        while let Some(k) = pilha.pop() {
            if self.especie(k) == "SimpleIdentifier"
                && let Marca::Expr(x) = self.arvore.nos[k].marca
                && matches!(self.corpos.get_resolved(x), Some(Resolved::Local(_)))
                && self.corpos.declaracao_local(x) == Some(nome)
            {
                let pai = self.pai(k);
                match pai.map(|p| self.especie(p)) {
                    Some("PrefixedIdentifier" | "PropertyAccess") if self.filhos(pai.unwrap_or(k)).first() == Some(&k) => {
                        let p = pai.unwrap_or(k);
                        let propriedade = self.filhos(p).last().map(|&q| self.texto_do_no(q).to_string()).unwrap_or_default();
                        v.push(Some((propriedade, p)));
                    }
                    _ => v.push(None),
                }
            }
            pilha.extend(self.filhos(k).iter().rev().copied());
        }
        v
    }

    /// `DestructureLocalVariableAssignment`.
    pub(crate) fn desestruturar_local(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let no = self.arvore.localizar(inicio, fim)?;
        if self.especie(no) != "VariableDeclaration" {
            return None;
        }
        let nome = self.token_seguinte(self.arvore.nos[no].inicio)?;
        // `declaredElement2`: só um local.
        let tipo = *self.corpos.tipos_de_locais.get(&nome.start)?;
        let tabela = &self.p.consulta.tabela;
        let acao = |edicoes: Vec<(Span, String)>| AcaoDeCodigo {
            titulo: "Destructure variable assignment".into(),
            especie: "refactor.destructureLocalVariableAssignment".into(),
            edicoes: edicoes.into_iter().rev().map(|(span, texto)| Edicao { uri: uri.to_string(), span, texto }).collect(),
            diagnostico: None,
            criar_arquivo: None,
        };
        match tabela.get(tipo) {
            Type::Record { positional, named, .. } => {
                let mut excluidos = self.nomes_em_escopo(no, self.arvore.nos[no].inicio);
                let mut variaveis: Vec<String> = Vec::new();
                for i in 1..=positional.len() {
                    let mut nome_v = format!("${i}");
                    if excluidos.contains(&nome_v) {
                        // `getIndexedVariableName`.
                        if let Some(n) = (b'a'..b'z').map(|c| format!("${i}{}", c as char)).find(|n| !excluidos.contains(n)) {
                            nome_v = n;
                        }
                    }
                    excluidos.insert(nome_v.clone());
                    variaveis.push(nome_v);
                }
                let mut nomeados: Vec<String> = named.iter().map(|(s, _)| self.p.nome(*s).to_string()).collect();
                nomeados.sort();
                for campo in nomeados {
                    if !excluidos.contains(&campo) {
                        variaveis.push(format!(":{campo}"));
                    } else {
                        let sugestao = nomes_para_texto(&campo, &excluidos).into_iter().next()?;
                        variaveis.push(format!("{campo}: {sugestao}"));
                    }
                }
                Some(acao(vec![(nome, format!("({})", variaveis.join(", ")))]))
            }
            Type::Interface { class, .. } => {
                let corpo = self.com_pais(no).find(|&k| matches!(self.especie(k), "BlockFunctionBody" | "ExpressionFunctionBody"))?;
                let leituras = self.leituras_do_local(corpo, nome.start);
                if leituras.iter().any(Option::is_none) {
                    return None;
                }
                let em_escopo = self.nomes_em_escopo(no, self.arvore.nos[no].inicio);
                // As propriedades, na ordem da primeira leitura.
                let mut propriedades: Vec<(String, Vec<usize>)> = Vec::new();
                for (p, acesso) in leituras.into_iter().flatten() {
                    match propriedades.iter_mut().find(|(q, _)| *q == p) {
                        Some((_, l)) => l.push(acesso),
                        None => propriedades.push((p, vec![acesso])),
                    }
                }
                let mut campos: Vec<(String, String, Vec<usize>)> = Vec::new();
                for (p, acessos) in propriedades {
                    let mut excluidos = self.conflitos_de_local(self.arvore.nos[no].inicio);
                    excluidos.extend(em_escopo.iter().cloned());
                    for &a in &acessos {
                        // `inSetterContext`.
                        if self.expr_do_no(a).is_some_and(|x| self.escritas.contains(&x)) {
                            return None;
                        }
                        excluidos.extend(self.conflitos_de_local(self.arvore.nos[a].inicio));
                    }
                    let variavel = nomes_para_texto(&p, &excluidos).into_iter().next()?;
                    campos.push((variavel, p, acessos));
                }
                let prog = self.p.programa();
                let padrao = campos
                    .iter()
                    .map(|(v, p, _)| if v == p { format!(":{v}") } else { format!("{p}: {v}") })
                    .collect::<Vec<_>>()
                    .join(", ");
                let mut edicoes = vec![(nome, format!("{}({padrao})", self.p.nome(prog.class(*class).name)))];
                for (v, _, acessos) in &campos {
                    for &a in acessos {
                        edicoes.push((self.arvore.span(a), v.clone()));
                    }
                }
                edicoes.sort_by_key(|(s, _)| s.start);
                Some(acao(edicoes))
            }
            _ => None,
        }
    }
}
