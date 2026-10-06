//! Uma consulta semântica transitória: o programa carregado com os textos
//! vigentes do editor, o outline e os corpos inferidos pela inferência comum
//! de `crates/types` ([`dartforge_types::BodyInferrer`]). O completar e o
//! renomear leem tipos e resoluções daqui, nunca de uma inferência própria.
//!
//! Tudo pertence a uma requisição: a consulta é descartada ao responder, e
//! nada dela sobrevive à próxima versão do texto (o platô de memória do LSP).

use dartforge_elements::model::{
    Element, FunctionElementId, FunctionRef, LibraryId, Program, UnitId, VariableId, VariableRef,
};
use dartforge_frontend::ast::{DeclKind, MemberKind};
use dartforge_intern::{Interner, SymbolId};
use dartforge_types::scope::MemberResolver;
use dartforge_types::{
    BodyInferrer, BodyTypes, CoreTypes, EscopoSondado, OutlineTypes, Type, TypeId, TypeTable,
    resolve_outline,
};

/// Como [`Consulta::trocar_unidade`] recria as tabelas da unidade.
#[derive(Debug, Clone, Copy)]
pub(crate) enum ModoDeTroca {
    /// O texto especulativo do completar: os inicializadores contam como
    /// corpo, e só o corpo que contém `offset` é inferido, com a sonda de
    /// escopo nele.
    Especulativa { offset: usize },
    /// O texto novo do documento (`didChange`): todos os corpos da unidade
    /// quando a biblioteca dela está entre as inferidas, senão só os
    /// inicializadores que a inferência de topo grava sob demanda.
    Definitiva { corpos_inferidos: bool, registrar_locais: bool },
}

/// O que [`Consulta::trocar_unidade`] substituiu.
pub(crate) struct Troca {
    anterior: dartforge_elements::incremental::Anterior,
    tabelas: dartforge_types::UnitBodyTypes,
    escopo: Option<EscopoSondado>,
}

/// Programa, tipos e corpos de uma requisição.
pub(crate) struct Consulta {
    pub programa: Program,
    pub nomes: Interner,
    pub tabela: TypeTable,
    pub core: CoreTypes,
    pub outline: OutlineTypes,
    pub corpos: BodyTypes,
    /// Escopo capturado pela sonda, quando pedida e alcançada.
    pub escopo: Option<EscopoSondado>,
}

impl Consulta {
    /// Resolve o outline e infere os corpos de `bibliotecas`.
    ///
    /// `registrar_locais` liga a tabela de declarações de locais (renomear);
    /// `sonda` é o identificador cujo escopo léxico o completar quer.
    pub fn inferir(
        programa: Program,
        nomes: Interner,
        bibliotecas: &[LibraryId],
        registrar_locais: bool,
        sonda: Option<(UnitId, usize)>,
    ) -> Self {
        let mut tabela = TypeTable::new();
        // O modo do analisador (M2): o `InvalidType`, o `Never?` e os aliases
        // escritos ficam na exibição, e o fluxo de padrões é o do analyzer.
        tabela.preservar_exibicao = true;
        let core = CoreTypes::init(&mut tabela, &programa, &nomes);
        let (mut outline, _) = resolve_outline(&programa, &nomes, &mut tabela, &core);
        let (corpos, escopo) = {
            let mut inferidor =
                BodyInferrer::new(&programa, &nomes, &mut tabela, &core, &mut outline);
            inferidor.apenas_bibliotecas = Some(bibliotecas.iter().map(|l| l.0).collect());
            inferidor.alocar_corpos_de(bibliotecas);
            inferidor.registrar_locais = registrar_locais;
            inferidor.sonda_escopo = sonda;
            let (corpos, _, escopo) = inferidor.infer_all_com_sonda();
            (corpos, escopo)
        };
        Self {
            programa,
            nomes,
            tabela,
            core,
            outline,
            corpos,
            escopo,
        }
    }

    /// Troca o texto da unidade `u` sem recarregar o programa quando só
    /// corpos mudaram (docs/LSP-ESPECIFICACAO.md §16.6, classe Corpo) e
    /// recria as tabelas dela pela inferência de corpos isolados (§16.5).
    /// `None`: recusada (assinatura, diretivas ou forma mudaram, ou o
    /// completar caiu fora de um corpo); a consulta fica como estava.
    pub fn trocar_unidade(&mut self, u: UnitId, texto: &str, modo: ModoDeTroca) -> Option<Troca> {
        use dartforge_elements::incremental::{restaurar_unidade, substituir_unidade};
        let especulativa = matches!(modo, ModoDeTroca::Especulativa { .. });
        let anterior = substituir_unidade(&mut self.programa, &mut self.nomes, u, texto, especulativa).ok()?;
        if !self.outline.remapear_tipos_escritos(u, anterior.mapa_de_tipos()) {
            restaurar_unidade(&mut self.programa, anterior);
            return None;
        }
        let (corpos, metadados, sonda, registrar) = match modo {
            ModoDeTroca::Especulativa { offset } => {
                // §16.7 regra 1: só com o sentinela dentro de um corpo.
                let Some(c) = dartforge_types::corpo_no_offset(&self.programa, u, offset) else {
                    self.outline.remapear_tipos_escritos(u, &anterior.mapa_inverso());
                    restaurar_unidade(&mut self.programa, anterior);
                    return None;
                };
                (vec![c], false, Some((u, offset)), false)
            }
            ModoDeTroca::Definitiva { corpos_inferidos, registrar_locais } => {
                (dartforge_types::corpos_da_unidade(&self.programa, &self.outline, u, corpos_inferidos), corpos_inferidos, None, registrar_locais)
            }
        };
        let r = dartforge_types::inferir_corpos(&self.programa, &self.nomes, &mut self.tabela, &self.core, &mut self.outline, u, &corpos, metadados, sonda, registrar);
        let tabelas = std::mem::replace(&mut self.corpos.units[u.0 as usize], r.tabelas);
        let escopo = std::mem::replace(&mut self.escopo, r.escopo);
        Some(Troca { anterior, tabelas, escopo })
    }

    /// Desfaz uma [`Consulta::trocar_unidade`] (o completar, §16.7 regra 3:
    /// o estado retido volta ao texto do documento; os tipos internados na
    /// tabela pela inferência especulativa ficam, o que é inofensivo).
    pub fn desfazer(&mut self, troca: Troca) {
        let Troca { anterior, tabelas, escopo } = troca;
        let u = anterior.unidade();
        self.corpos.units[u.0 as usize] = tabelas;
        self.escopo = escopo;
        self.outline.remapear_tipos_escritos(u, &anterior.mapa_inverso());
        dartforge_elements::incremental::restaurar_unidade(&mut self.programa, anterior);
    }

    /// A mesma consulta sobre o mesmo programa, com os corpos de outras
    /// bibliotecas (a busca de referências amplia a lista).
    pub fn reinferir(self, bibliotecas: &[LibraryId]) -> Self {
        let Consulta { programa, nomes, .. } = self;
        Consulta::inferir(programa, nomes, bibliotecas, true, None)
    }

    /// Texto de um tipo como o Dart o escreve.
    /// `DartType.getDisplayString()` (sem `preferTypeAlias`): o alias não
    /// aparece; o `InvalidType` e o `Never?`, sim.
    pub fn formatar(&self, tipo: TypeId) -> String {
        self.tabela.format_sem_alias(tipo, &self.nomes, &self.programa)
    }

    /// Texto de um símbolo.
    pub fn nome(&self, simbolo: SymbolId) -> &str {
        self.nomes.resolve(simbolo)
    }

    /// Busca de membros sobre receptores estáticos (a mesma de `crates/types`).
    pub fn resolvedor(&mut self) -> MemberResolver<'_> {
        MemberResolver::new(
            &self.programa,
            &self.outline,
            &self.nomes,
            &self.outline.hierarchy,
            &mut self.tabela,
            &self.core,
        )
    }

    /// Tipo de uma variável de topo ou campo: o escrito, senão o inferido.
    pub fn tipo_da_variavel(&self, variavel: VariableId) -> Option<TypeId> {
        let dados = self.outline.variables.get(variavel.0 as usize)?;
        dados.declared_type.or(dados.inferred)
    }

    /// Detalhe de uma função (`(int x, {String? nome}) → void`) a partir da
    /// assinatura `assinatura` (já substituída pelo receptor, quando há) e dos
    /// nomes dos parâmetros declarados.
    pub fn detalhe_de_funcao(&self, funcao: FunctionElementId, assinatura: TypeId) -> String {
        let Type::Function {
            ret,
            positional,
            optional,
            named,
            ..
        } = self.tabela.get(assinatura).clone()
        else {
            return self.formatar(assinatura);
        };
        let dados = self.outline.functions.get(funcao.0 as usize);
        let nomes_posicionais: Vec<Option<SymbolId>> = dados
            .map(|d| {
                d.parameters
                    .iter()
                    .filter(|p| p.kind != dartforge_frontend::ast::ParameterKind::Named)
                    .map(|p| p.name)
                    .collect()
            })
            .unwrap_or_default();
        let mut partes = Vec::new();
        for (i, t) in positional.iter().enumerate() {
            partes.push(self.parametro(*t, nomes_posicionais.get(i).copied().flatten()));
        }
        if !optional.is_empty() {
            let opcionais: Vec<String> = optional
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    self.parametro(
                        *t,
                        nomes_posicionais
                            .get(positional.len() + i)
                            .copied()
                            .flatten(),
                    )
                })
                .collect();
            partes.push(format!("[{}]", opcionais.join(", ")));
        }
        if !named.is_empty() {
            // A ordem da declaração, não a canônica do tipo (por símbolo).
            let mut nomeados: Vec<(usize, String)> = named
                .iter()
                .map(|(n, t, obrigatorio)| {
                    let ordem = dados
                        .and_then(|d| {
                            d.parameters
                                .iter()
                                .position(|p| p.externo.or(p.name) == Some(*n))
                        })
                        .unwrap_or(usize::MAX);
                    let prefixo = if *obrigatorio { "required " } else { "" };
                    (
                        ordem,
                        format!("{prefixo}{} {}", self.formatar(*t), self.nome(*n)),
                    )
                })
                .collect();
            nomeados.sort();
            partes.push(format!(
                "{{{}}}",
                nomeados
                    .into_iter()
                    .map(|(_, s)| s)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        format!("({}) → {}", partes.join(", "), self.formatar(ret))
    }

    fn parametro(&self, tipo: TypeId, nome: Option<SymbolId>) -> String {
        match nome {
            Some(n) => format!("{} {}", self.formatar(tipo), self.nome(n)),
            None => self.formatar(tipo),
        }
    }

    /// Nome simples de um elemento de topo, quando ele tem nome.
    pub fn nome_do_elemento(&self, elemento: Element) -> Option<SymbolId> {
        Some(match elemento {
            Element::Class(c) => self.programa.class(c).name,
            Element::Extension(x) => self.programa.extension(x).name?,
            Element::Typedef(t) => self.programa.typedef(t).name,
            Element::Function(f) => self.programa.function(f).name,
            Element::Variable(v) => self.programa.variable(v).name,
            Element::Prefix(_, p) => p,
        })
    }

    /// Unidade e início (com metadados) da declaração de um elemento de topo.
    pub fn inicio_do_elemento(&self, el: Element) -> Option<(UnitId, usize)> {
        let p = &self.programa;
        let d = match el {
            Element::Class(c) => p.class(c).decl?,
            Element::Extension(x) => p.extension(x).decl,
            Element::Typedef(t) => p.typedef(t).decl,
            Element::Function(f) => return self.inicio_da_funcao(f),
            Element::Variable(v) => return self.inicio_da_variavel(v),
            Element::Prefix(..) => return None,
        };
        Some((d.unit, p.unit(d.unit).ast.decl(d.decl).span.start))
    }

    /// Unidade e início (com metadados) de uma função, método ou construtor.
    pub fn inicio_da_funcao(&self, f: FunctionElementId) -> Option<(UnitId, usize)> {
        let p = &self.programa;
        match p.function(f).node {
            FunctionRef::Function { unit, function } => {
                let ast = &p.unit(unit).ast;
                let inicio = ast
                    .decls
                    .iter()
                    .find(|d| matches!(d.kind, DeclKind::Function(x) if x == function))
                    .map(|d| d.span.start)
                    .or_else(|| {
                        ast.members
                            .iter()
                            .find(|m| matches!(m.kind, MemberKind::Method(x) if x == function))
                            .map(|m| m.span.start)
                    })
                    .unwrap_or(ast.function(function).span.start);
                Some((unit, inicio))
            }
            FunctionRef::Constructor { unit, member } => {
                Some((unit, p.unit(unit).ast.member(member).span.start))
            }
            FunctionRef::None => p
                .function(f)
                .variable
                .and_then(|v| self.inicio_da_variavel(v)),
        }
    }

    /// Unidade e início (com metadados) de uma variável de topo, campo ou
    /// constante de enum.
    pub fn inicio_da_variavel(&self, v: VariableId) -> Option<(UnitId, usize)> {
        let p = &self.programa;
        match p.variable(v).node {
            VariableRef::TopLevel { unit, decl, .. } => {
                Some((unit, p.unit(unit).ast.decl(decl).span.start))
            }
            VariableRef::Field { unit, member, .. } => {
                Some((unit, p.unit(unit).ast.member(member).span.start))
            }
            VariableRef::EnumConstant { unit, decl, index } => {
                match &p.unit(unit).ast.decl(decl).kind {
                    DeclKind::Enum(e) => Some((unit, e.constants.get(index)?.span.start)),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Arquivo e início (com metadados) de uma declaração, para a
    /// documentação que o `completionItem/resolve` busca depois.
    pub fn origem(&self, local: Option<(UnitId, usize)>) -> Option<(std::path::PathBuf, usize)> {
        let (u, inicio) = local?;
        Some((self.programa.unit(u).path.clone()?, inicio))
    }
}
