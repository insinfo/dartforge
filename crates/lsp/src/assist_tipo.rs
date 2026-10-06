//! Assistências de anotação de tipo, portadas dos produtores do
//! `analysis_server` 3.6.2 sobre a árvore no formato do analyzer.
//!
//! | Título | Espécie | Produtor |
//! |---|---|---|
//! | `Remove type annotation` | `refactor.remove.typeAnnotation` | `RemoveTypeAnnotation.other` |
//! | `Add type annotation` | `refactor.add.typeAnnotation` | `AddTypeAnnotation.bulkFixable` |

use crate::acoes::AcaoDeCodigo;
use crate::refatoracoes::Contexto;
use crate::Edicao;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{self, ExprKind, StmtKind};
use dartforge_types::{Type, TypeId};

/// As espécies de `TypeAnnotation`.
const TIPOS: &[&str] = &["NamedType", "GenericFunctionType", "RecordTypeAnnotation"];

impl Contexto<'_> {
    fn acao_de_tipo(&self, uri: &str, titulo: &str, especie: &str, edicoes: Vec<(Span, String)>) -> AcaoDeCodigo {
        AcaoDeCodigo {
            titulo: titulo.into(),
            especie: especie.into(),
            edicoes: edicoes.into_iter().map(|(span, texto)| Edicao { uri: uri.to_string(), span, texto }).collect(),
            diagnostico: None,
            criar_arquivo: None,
        }
    }

    /// O filho de `n` que é uma anotação de tipo.
    fn tipo_filho(&self, n: usize) -> Option<usize> {
        self.filhos(n).iter().copied().find(|&f| TIPOS.contains(&self.especie(f)))
    }

    /// `RemoveTypeAnnotation.other` (remove_type_annotation.dart): o primeiro
    /// `DeclaredIdentifier`, `SimpleFormalParameter`, `SuperFormalParameter`
    /// ou `VariableDeclarationList` entre o nó da seleção e os ancestrais.
    pub(crate) fn remover_anotacao_de_tipo(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let no = self.arvore.localizar(inicio, fim)?;
        let alvo = self.com_pais(no).find(|&k| {
            matches!(self.especie(k), "DeclaredIdentifier" | "SimpleFormalParameter" | "SuperFormalParameter" | "VariableDeclarationList")
        })?;
        let edicoes = match self.especie(alvo) {
            "DeclaredIdentifier" => self.remover_de_identificador_declarado(alvo)?,
            "SimpleFormalParameter" | "SuperFormalParameter" => {
                // `_removeTypeAnnotation`: do tipo ao token seguinte; num
                // `super.f(int a)`, também a lista de parâmetros.
                let tipo = self.tipo_filho(alvo)?;
                let fim_tipo = self.arvore.nos[tipo].fim;
                let seguinte = self.token_seguinte(fim_tipo)?.start;
                let mut v = vec![(Span { start: self.arvore.nos[tipo].inicio, end: seguinte }, String::new())];
                if self.especie(alvo) == "SuperFormalParameter"
                    && let Some(lista) = self.filhos(alvo).iter().copied().find(|&f| self.especie(f) == "FormalParameterList")
                {
                    v.push((self.arvore.span(lista), String::new()));
                }
                v
            }
            _ => self.remover_da_lista_de_declaracao(alvo, inicio)?,
        };
        Some(self.acao_de_tipo(uri, "Remove type annotation", "refactor.remove.typeAnnotation", edicoes))
    }

    /// A palavra `final`/`const`/`var` da lista ou do identificador
    /// declarado (o `keyword`), entre `ini` e `limite`.
    fn palavra_da_declaracao(&self, ini: usize, limite: usize) -> Option<&str> {
        self.tokens
            .iter()
            .filter(|t| t.span.start >= ini && t.span.end <= limite)
            .map(|t| &self.fonte[t.span.start..t.span.end])
            .find(|s| matches!(*s, "final" | "const" | "var"))
    }

    /// `_removeFromDeclaredIdentifier`.
    fn remover_de_identificador_declarado(&self, alvo: usize) -> Option<Vec<(Span, String)>> {
        let tipo = self.tipo_filho(alvo)?;
        // O nome: o último token do nó.
        let nome = self.token_anterior(self.arvore.nos[alvo].fim)?;
        let palavra = self.palavra_da_declaracao(self.arvore.nos[alvo].inicio, self.arvore.nos[tipo].inicio);
        let faixa = Span { start: self.arvore.nos[tipo].inicio, end: nome.start };
        Some(vec![(faixa, if palavra.is_some_and(|p| p != "var") { String::new() } else { "var ".to_string() })])
    }

    /// `_removeFromDeclarationList`.
    fn remover_da_lista_de_declaracao(&self, lista: usize, selecao: usize) -> Option<Vec<(Span, String)>> {
        let tipo = self.tipo_filho(lista)?;
        let variaveis = self.filhos_da_especie(lista, "VariableDeclaration");
        let primeira = *variaveis.first()?;
        let nome = self.token_seguinte(self.arvore.nos[primeira].inicio)?;
        // Declaração incompleta (nome sintético).
        if variaveis.len() == 1 && nome.start != self.arvore.nos[primeira].inicio {
            return None;
        }
        if selecao > nome.end {
            return None;
        }
        // Sem inicializador não há outra fonte para o tipo.
        let mut inicializador = *self.filhos(primeira).first()?;
        let mut argumentos: Option<(String, usize)> = None;
        if self.especie(tipo) == "NamedType"
            && let Some(args) = self.filhos(tipo).iter().copied().find(|&f| self.especie(f) == "TypeArgumentList")
        {
            if self.especie(inicializador) == "CascadeExpression" {
                inicializador = *self.filhos(inicializador).first()?;
            }
            let texto = self.texto_do_no(args).to_string();
            match self.especie(inicializador) {
                "ListLiteral" | "SetOrMapLiteral" => {
                    if !self.filhos(inicializador).iter().any(|&f| self.especie(f) == "TypeArgumentList") {
                        // O `leftBracket`.
                        let abre = self
                            .tokens
                            .iter()
                            .find(|t| t.span.start >= self.arvore.nos[inicializador].inicio && matches!(&self.fonte[t.span.start..t.span.end], "[" | "{"))?;
                        argumentos = Some((texto, abre.span.start));
                    }
                }
                "InstanceCreationExpression" => {
                    let nome_do_construtor = self.filhos(inicializador).iter().copied().find(|&f| self.especie(f) == "ConstructorName")?;
                    let tipo_criado = self.filhos(nome_do_construtor).iter().copied().find(|&f| self.especie(f) == "NamedType")?;
                    if !self.filhos(tipo_criado).iter().any(|&f| self.especie(f) == "TypeArgumentList") {
                        argumentos = Some((texto, self.arvore.nos[tipo_criado].fim));
                    }
                }
                _ => {}
            }
        }
        // Um literal de conjunto ou mapa sem argumentos de tipo ficaria
        // ambíguo.
        if self.especie(inicializador) == "SetOrMapLiteral"
            && !self.filhos(inicializador).iter().any(|&f| self.especie(f) == "TypeArgumentList")
            && argumentos.is_none()
        {
            return None;
        }
        let palavra = self.palavra_da_declaracao(self.arvore.nos[lista].inicio, self.arvore.nos[tipo].inicio);
        let faixa = Span { start: self.arvore.nos[tipo].inicio, end: nome.start };
        let mut v = vec![(faixa, if palavra.is_some_and(|p| p != "var") { String::new() } else { "var ".to_string() })];
        if let Some((texto, em)) = argumentos {
            v.push((Span { start: em, end: em }, texto));
        }
        Some(v)
    }

    // -- AddTypeAnnotation -------------------------------------------------------

    /// `AddTypeAnnotation.compute` (add_type_annotation.dart): o nó da
    /// seleção como `SimpleFormalParameter`, `DeclaredVariablePattern` ou
    /// literal tipado; senão o primeiro `VariableDeclarationList`,
    /// `DeclaredIdentifier` ou `ForStatement` entre os ancestrais.
    pub(crate) fn adicionar_anotacao_de_tipo(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let no = self.arvore.localizar(inicio, fim)?;
        let mudanca = match self.especie(no) {
            "SimpleFormalParameter" => self.tipo_de_parametro_simples(no)?,
            "DeclaredVariablePattern" => self.tipo_de_padrao_declarado(no)?,
            "ListLiteral" | "SetOrMapLiteral" => self.tipo_de_literal(no)?,
            _ => {
                let alvo = self
                    .com_pais(no)
                    .find(|&k| matches!(self.especie(k), "VariableDeclarationList" | "DeclaredIdentifier" | "ForStatement"))?;
                match self.especie(alvo) {
                    "VariableDeclarationList" => self.tipo_de_lista_de_declaracao(alvo, inicio)?,
                    "DeclaredIdentifier" => self.tipo_de_identificador_declarado(alvo)?,
                    _ => {
                        // `ForStatement` com `ForEachPartsWithDeclaration`, o nó
                        // antes do iterável.
                        let partes = self.filhos(alvo).iter().copied().find(|&f| self.especie(f) == "ForEachPartsWithDeclaration")?;
                        let declarado = self.filhos(partes).iter().copied().find(|&f| self.especie(f) == "DeclaredIdentifier")?;
                        let iteravel = *self.filhos(partes).last()?;
                        if self.arvore.nos[no].inicio >= self.arvore.nos[iteravel].inicio {
                            return None;
                        }
                        self.tipo_de_identificador_declarado(declarado)?
                    }
                }
            }
        };
        let (edicoes, importar) = mudanca;
        let mut m = crate::refatoracoes_exec::Mudanca::default();
        for (s, texto) in edicoes {
            m.adicionar(uri, s, texto);
        }
        crate::refatoracoes_mover::imports_do_builder(self, &mut m, &importar);
        if m.conflito.is_some() {
            return None;
        }
        Some(AcaoDeCodigo {
            titulo: "Add type annotation".into(),
            especie: "refactor.add.typeAnnotation".into(),
            edicoes: m.arquivos.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(),
            diagnostico: None,
            criar_arquivo: None,
        })
    }

    /// `_applyChange`: com `var`, o tipo no lugar dele; senão antes do nome
    /// (`canWriteType` falso: nada).
    #[allow(clippy::type_complexity)]
    fn aplicar_tipo(&self, palavra: Option<Span>, nome: Span, t: TypeId) -> Option<(Vec<(Span, String)>, std::collections::BTreeSet<dartforge_elements::model::LibraryId>)> {
        let mut escritor = crate::escrever_tipo::Escritor::novo(self, nome.start);
        let texto = escritor.escrever(t, false)?;
        let edicao = match palavra {
            Some(p) if &self.fonte[p.start..p.end] == "var" => (p, texto),
            _ => (Span { start: nome.start, end: nome.start }, format!("{texto} ")),
        };
        Some((vec![edicao], escritor.importar))
    }

    /// `InterfaceType` do analyzer: classe, `FutureOr`, tipo de extensão.
    fn e_tipo_de_interface(&self, t: TypeId) -> bool {
        matches!(self.p.consulta.tabela.get(t), Type::Interface { .. } | Type::FutureOr { .. } | Type::ExtensionType { .. })
    }

    /// `_forSimpleFormalParameter`.
    #[allow(clippy::type_complexity)]
    fn tipo_de_parametro_simples(&self, no: usize) -> Option<(Vec<(Span, String)>, std::collections::BTreeSet<dartforge_elements::model::LibraryId>)> {
        if self.tipo_filho(no).is_some() {
            return None;
        }
        // O nome: o último token do nó.
        let nome = self.token_anterior(self.arvore.nos[no].fim)?;
        if !self.fonte[nome.start..nome.end].chars().next().is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$') {
            return None;
        }
        let t = self.corpos.tipo_local(nome.start)?;
        if !(self.e_tipo_de_interface(t) || matches!(self.p.consulta.tabela.get(t), Type::Record { .. })) {
            return None;
        }
        self.aplicar_tipo(None, nome, t)
    }

    /// O `PatternId` do nó de padrão (pelo intervalo).
    fn padrao_do_no(&self, no: usize) -> Option<ast::PatternId> {
        let s = self.arvore.span(no);
        self.ast.patterns.iter().position(|p| p.span == s && matches!(p.kind, ast::PatternKind::Variable { .. })).map(|i| ast::PatternId(i as u32))
    }

    /// `DeclaredVariablePattern`: o `matchedValueType`.
    #[allow(clippy::type_complexity)]
    fn tipo_de_padrao_declarado(&self, no: usize) -> Option<(Vec<(Span, String)>, std::collections::BTreeSet<dartforge_elements::model::LibraryId>)> {
        let pid = self.padrao_do_no(no)?;
        let ast::PatternKind::Variable { name, .. } = &self.ast.pattern(pid).kind else { return None };
        let t = *self.corpos.tipos_casados.get(&pid)?;
        let palavra = self
            .tokens
            .iter()
            .find(|tk| tk.span.start >= self.arvore.nos[no].inicio && tk.span.end <= name.span.start && matches!(&self.fonte[tk.span.start..tk.span.end], "var" | "final"))
            .map(|tk| tk.span);
        self.aplicar_tipo(palavra, name.span, t)
    }

    /// `_typedLiteral`: `<argumentos>` no `[`/`{`.
    #[allow(clippy::type_complexity)]
    fn tipo_de_literal(&self, no: usize) -> Option<(Vec<(Span, String)>, std::collections::BTreeSet<dartforge_elements::model::LibraryId>)> {
        let x = self.expr_do_no(no)?;
        let t = self.corpos.get_type(x)?;
        let args: Vec<TypeId> = match self.p.consulta.tabela.get(t) {
            Type::Interface { args, .. } | Type::ExtensionType { args, .. } => args.to_vec(),
            Type::FutureOr { arg, .. } => vec![*arg],
            _ => return None,
        };
        let abre = self
            .tokens
            .iter()
            .find(|tk| tk.span.start >= self.arvore.nos[no].inicio && matches!(&self.fonte[tk.span.start..tk.span.end], "[" | "{"))?
            .span
            .start;
        let mut escritor = crate::escrever_tipo::Escritor::novo(self, abre);
        let textos: Vec<String> = args.iter().map(|&a| escritor.escrever_tipo(Some(a), false).unwrap_or_default()).collect();
        Some((vec![(Span { start: abre, end: abre }, format!("<{}>", textos.join(", ")))], escritor.importar))
    }

    /// `_forDeclaredIdentifier`.
    #[allow(clippy::type_complexity)]
    fn tipo_de_identificador_declarado(&self, no: usize) -> Option<(Vec<(Span, String)>, std::collections::BTreeSet<dartforge_elements::model::LibraryId>)> {
        if self.tipo_filho(no).is_some() {
            return None;
        }
        let nome = self.token_anterior(self.arvore.nos[no].fim)?;
        let t = self.corpos.tipo_local(nome.start)?;
        if !(self.e_tipo_de_interface(t)
            || matches!(self.p.consulta.tabela.get(t), Type::Function { .. } | Type::Record { .. } | Type::TypeParameter { .. } | Type::Intersection { .. }))
        {
            return None;
        }
        let palavra = self
            .tokens
            .iter()
            .find(|tk| tk.span.start >= self.arvore.nos[no].inicio && tk.span.end <= nome.start && matches!(&self.fonte[tk.span.start..tk.span.end], "var" | "final" | "const"))
            .map(|tk| tk.span);
        self.aplicar_tipo(palavra, nome, t)
    }

    /// `_typeForVariable`: o tipo do inicializador; sem ele, num local de
    /// bloco, o LUB dos tipos atribuídos nos comandos seguintes.
    fn tipo_da_variavel(&self, variavel: usize) -> Option<TypeId> {
        let nome = self.token_seguinte(self.arvore.nos[variavel].inicio)?;
        if let Some(&ini) = self.filhos(variavel).first() {
            return self.expr_do_no(ini).and_then(|x| self.corpos.get_type(x));
        }
        // O comando de declaração e o bloco que o contém.
        let lista = self.pai(variavel)?;
        let comando = self.pai(lista)?;
        let bloco = self.pai(comando)?;
        if self.especie(comando) != "VariableDeclarationStatement" || self.especie(bloco) != "Block" {
            return None;
        }
        let fim_do_comando = self.arvore.nos[comando].fim;
        let fim_do_bloco = self.arvore.nos[bloco].fim;
        let mut tipos: Vec<TypeId> = Vec::new();
        for (i, e) in self.ast.exprs.iter().enumerate() {
            let ExprKind::Assign { op: ast::AssignOp::Assign, target, value } = &e.kind else { continue };
            if e.span.start < fim_do_comando || e.span.end > fim_do_bloco {
                continue;
            }
            if !matches!(self.ast.expr(*target).kind, ExprKind::Identifier(_)) || self.corpos.declaracao_local(*target) != Some(nome.start) {
                continue;
            }
            let _ = i;
            if let Some(t) = self.corpos.get_type(*value)
                && !tipos.contains(&t)
            {
                tipos.push(t);
            }
        }
        let (&primeiro, resto) = tipos.split_first()?;
        if resto.is_empty() {
            return Some(primeiro);
        }
        // `leastUpperBound` numa cópia da tabela: só vale um tipo que a
        // tabela da consulta já tem (o escritor lê esta).
        let consulta = &self.p.consulta;
        let mut tabela = consulta.tabela.clone();
        let mut env = dartforge_types::subtyping::SubtypeEnv::new(&mut tabela, &consulta.outline.hierarchy, &consulta.core);
        let mut melhor = primeiro;
        for &t in resto {
            melhor = dartforge_types::bounds::up(melhor, t, &mut env);
        }
        ((melhor.0 as usize) < consulta.tabela.len()).then_some(melhor)
    }

    /// `_forVariableDeclaration`.
    #[allow(clippy::type_complexity)]
    fn tipo_de_lista_de_declaracao(&self, lista: usize, selecao: usize) -> Option<(Vec<(Span, String)>, std::collections::BTreeSet<dartforge_elements::model::LibraryId>)> {
        if self.tipo_filho(lista).is_some() {
            return None;
        }
        let variaveis = self.filhos_da_especie(lista, "VariableDeclaration");
        let primeira = *variaveis.first()?;
        let nome = self.token_seguinte(self.arvore.nos[primeira].inicio)?;
        if selecao > nome.end {
            return None;
        }
        let t = self.tipo_da_variavel(primeira)?;
        for &v in &variaveis[1..] {
            if self.tipo_da_variavel(v) != Some(t) {
                return None;
            }
        }
        let tabela = &self.p.consulta.tabela;
        let interface_nao_nula = self.e_tipo_de_interface(t) && !matches!(tabela.get(t), Type::Null);
        if !(interface_nao_nula || matches!(tabela.get(t), Type::Function { .. } | Type::Record { .. } | Type::TypeParameter { .. } | Type::Intersection { .. })) {
            return None;
        }
        let palavra = self
            .tokens
            .iter()
            .find(|tk| tk.span.start >= self.arvore.nos[lista].inicio && tk.span.end <= nome.start && matches!(&self.fonte[tk.span.start..tk.span.end], "var" | "final" | "const"))
            .map(|tk| tk.span);
        let _ = StmtKind::Empty;
        self.aplicar_tipo(palavra, nome, t)
    }
}
