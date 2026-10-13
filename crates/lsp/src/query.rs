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

/// Como [`Query::swap_unit`] recria as tabelas da unidade.
#[derive(Debug, Clone, Copy)]
pub(crate) enum SwapMode {
    /// O texto especulativo do completar: os inicializadores contam como
    /// corpo, e só o corpo que contém `offset` é inferido, com a sonda de
    /// escopo nele.
    Speculative { offset: usize },
    /// O texto novo do documento (`didChange`): todos os corpos da unidade
    /// quando a biblioteca dela está entre as inferidas, senão só os
    /// inicializadores que a inferência de topo grava sob demanda.
    Final { inferred_bodies: bool, register_locals: bool },
}

/// O que [`Query::swap_unit`] substituiu.
pub(crate) struct Swap {
    previous: dartforge_elements::incremental::Anterior,
    tables: dartforge_types::UnitBodyTypes,
    scope: Option<EscopoSondado>,
}

/// Programa, tipos e corpos de uma requisição.
pub(crate) struct Query {
    pub program: Program,
    pub names: Interner,
    pub table: TypeTable,
    pub core: CoreTypes,
    pub outline: OutlineTypes,
    pub bodies: BodyTypes,
    /// Escopo capturado pela sonda, quando pedida e alcançada.
    pub scope: Option<EscopoSondado>,
}

impl Query {
    /// Resolve o outline e infere os corpos de `libraries`.
    ///
    /// `register_locals` liga a tabela de declarações de locais (renomear);
    /// `probe` é o identificador cujo escopo léxico o completar quer.
    pub fn infer(
        program: Program,
        names: Interner,
        libraries: &[LibraryId],
        register_locals: bool,
        probe: Option<(UnitId, usize)>,
    ) -> Self {
        let mut table = TypeTable::new();
        // O modo do analisador (M2): o `InvalidType`, o `Never?` e os aliases
        // escritos ficam na exibição, e o fluxo de padrões é o do analyzer.
        table.preservar_exibicao = true;
        let core = CoreTypes::init(&mut table, &program, &names);
        let (mut outline, _) = resolve_outline(&program, &names, &mut table, &core);
        let (bodies, scope) = {
            let mut inferrer =
                BodyInferrer::new(&program, &names, &mut table, &core, &mut outline);
            inferrer.apenas_bibliotecas = Some(libraries.iter().map(|l| l.0).collect());
            inferrer.alocar_corpos_de(libraries);
            inferrer.registrar_locais = register_locals;
            inferrer.sonda_escopo = probe;
            let (bodies, _, scope) = inferrer.infer_all_com_sonda();
            (bodies, scope)
        };
        Self {
            program,
            names,
            table,
            core,
            outline,
            bodies,
            scope,
        }
    }

    /// Troca o texto da unidade `u` sem recarregar o programa quando só
    /// corpos mudaram (docs/LSP-ESPECIFICACAO.md §16.6, classe Corpo) e
    /// recria as tabelas dela pela inferência de corpos isolados (§16.5).
    /// `None`: recusada (assinatura, diretivas ou forma mudaram, ou o
    /// completar caiu fora de um corpo); a consulta fica como estava.
    pub fn swap_unit(&mut self, u: UnitId, text: &str, mode: SwapMode) -> Option<Swap> {
        use dartforge_elements::incremental::{restaurar_unidade, substituir_unidade};
        let speculative = matches!(mode, SwapMode::Speculative { .. });
        let previous = substituir_unidade(&mut self.program, &mut self.names, u, text, speculative).ok()?;
        if !self.outline.remapear_tipos_escritos(u, previous.mapa_de_tipos()) {
            restaurar_unidade(&mut self.program, previous);
            return None;
        }
        let (bodies, metadata, probe, register) = match mode {
            SwapMode::Speculative { offset } => {
                // §16.7 regra 1: só com o sentinela dentro de um corpo.
                let Some(c) = dartforge_types::corpo_no_offset(&self.program, u, offset) else {
                    self.outline.remapear_tipos_escritos(u, &previous.mapa_inverso());
                    restaurar_unidade(&mut self.program, previous);
                    return None;
                };
                (vec![c], false, Some((u, offset)), false)
            }
            SwapMode::Final { inferred_bodies, register_locals } => {
                (dartforge_types::corpos_da_unidade(&self.program, &self.outline, u, inferred_bodies), inferred_bodies, None, register_locals)
            }
        };
        let r = dartforge_types::inferir_corpos(&self.program, &self.names, &mut self.table, &self.core, &mut self.outline, u, &bodies, metadata, probe, register);
        let tables = std::mem::replace(&mut self.bodies.units[u.0 as usize], r.tabelas);
        let scope = std::mem::replace(&mut self.scope, r.escopo);
        Some(Swap { previous, tables, scope })
    }

    /// Desfaz uma [`Query::swap_unit`] (o completar, §16.7 regra 3:
    /// o estado retido volta ao texto do documento; os tipos internados na
    /// tabela pela inferência especulativa ficam, o que é inofensivo).
    pub fn undo(&mut self, swap: Swap) {
        let Swap { previous, tables, scope } = swap;
        let u = previous.unidade();
        self.bodies.units[u.0 as usize] = tables;
        self.scope = scope;
        self.outline.remapear_tipos_escritos(u, &previous.mapa_inverso());
        dartforge_elements::incremental::restaurar_unidade(&mut self.program, previous);
    }

    /// Infere os corpos de mais `libraries` sobre o esboço e a tabela
    /// retidos, unidade por unidade (`inferir_corpos`, §16.5: cada corpo sai
    /// como na inferência completa), sem refazer o esboço nem os corpos já
    /// inferidos.
    pub fn infer_libraries(&mut self, libraries: &[LibraryId]) {
        for &lib in libraries {
            let units = self.program.library(lib).units.clone();
            for u in units {
                let bodies = dartforge_types::corpos_da_unidade(&self.program, &self.outline, u, true);
                let r = dartforge_types::inferir_corpos(&self.program, &self.names, &mut self.table, &self.core, &mut self.outline, u, &bodies, true, None, true);
                self.bodies.units[u.0 as usize] = r.tabelas;
            }
        }
    }

    /// A mesma consulta sobre o mesmo programa, com os corpos de outras
    /// bibliotecas (as cores do Flutter ampliam a lista).
    pub fn reinfer(self, libraries: &[LibraryId]) -> Self {
        let Query { program, names, .. } = self;
        Query::infer(program, names, libraries, true, None)
    }

    /// Texto de um tipo como o Dart o escreve.
    /// `DartType.getDisplayString()` (sem `preferTypeAlias`): o alias não
    /// aparece; o `InvalidType` e o `Never?`, sim.
    pub fn format(&self, ty: TypeId) -> String {
        self.table.format_sem_alias(ty, &self.names, &self.program)
    }

    /// Texto de um símbolo.
    pub fn name(&self, symbol: SymbolId) -> &str {
        self.names.resolve(symbol)
    }

    /// Busca de membros sobre receptores estáticos (a mesma de `crates/types`).
    pub fn resolver(&mut self) -> MemberResolver<'_> {
        MemberResolver::new(
            &self.program,
            &self.outline,
            &self.names,
            &self.outline.hierarchy,
            &mut self.table,
            &self.core,
        )
    }

    /// Tipo de uma variável de topo ou campo: o escrito, senão o inferido.
    pub fn variable_type(&self, variable: VariableId) -> Option<TypeId> {
        let data = self.outline.variables.get(variable.0 as usize)?;
        data.declared_type.or(data.inferred)
    }

    /// Detalhe de uma função (`(int x, {String? name}) → void`) a partir da
    /// assinatura `signature` (já substituída pelo receptor, quando há) e dos
    /// nomes dos parâmetros declarados.
    pub fn function_detail(&self, function: FunctionElementId, signature: TypeId) -> String {
        let Type::Function {
            ret,
            positional,
            optional,
            named,
            ..
        } = self.table.get(signature).clone()
        else {
            return self.format(signature);
        };
        let data = self.outline.functions.get(function.0 as usize);
        let positional_names: Vec<Option<SymbolId>> = data
            .map(|d| {
                d.parameters
                    .iter()
                    .filter(|p| p.kind != dartforge_frontend::ast::ParameterKind::Named)
                    .map(|p| p.name)
                    .collect()
            })
            .unwrap_or_default();
        let mut parts = Vec::new();
        for (i, t) in positional.iter().enumerate() {
            parts.push(self.parameter(*t, positional_names.get(i).copied().flatten()));
        }
        if !optional.is_empty() {
            let optional_: Vec<String> = optional
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    self.parameter(
                        *t,
                        positional_names
                            .get(positional.len() + i)
                            .copied()
                            .flatten(),
                    )
                })
                .collect();
            parts.push(format!("[{}]", optional_.join(", ")));
        }
        if !named.is_empty() {
            // A ordem da declaração, não a canônica do tipo (por símbolo).
            let mut named_: Vec<(usize, String)> = named
                .iter()
                .map(|(n, t, required)| {
                    let order = data
                        .and_then(|d| {
                            d.parameters
                                .iter()
                                .position(|p| p.externo.or(p.name) == Some(*n))
                        })
                        .unwrap_or(usize::MAX);
                    let prefix = if *required { "required " } else { "" };
                    (
                        order,
                        format!("{prefix}{} {}", self.format(*t), self.name(*n)),
                    )
                })
                .collect();
            named_.sort();
            parts.push(format!(
                "{{{}}}",
                named_
                    .into_iter()
                    .map(|(_, s)| s)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        format!("({}) → {}", parts.join(", "), self.format(ret))
    }

    fn parameter(&self, ty: TypeId, name: Option<SymbolId>) -> String {
        match name {
            Some(n) => format!("{} {}", self.format(ty), self.name(n)),
            None => self.format(ty),
        }
    }

    /// Nome simples de um elemento de topo, quando ele tem nome.
    pub fn element_name(&self, element: Element) -> Option<SymbolId> {
        Some(match element {
            Element::Class(c) => self.program.class(c).name,
            Element::Extension(x) => self.program.extension(x).name?,
            Element::Typedef(t) => self.program.typedef(t).name,
            Element::Function(f) => self.program.function(f).name,
            Element::Variable(v) => self.program.variable(v).name,
            Element::Prefix(_, p) => p,
        })
    }

    /// Unidade e início (com metadados) da declaração de um elemento de topo.
    pub fn element_start(&self, el: Element) -> Option<(UnitId, usize)> {
        let p = &self.program;
        let d = match el {
            Element::Class(c) => p.class(c).decl?,
            Element::Extension(x) => p.extension(x).decl,
            Element::Typedef(t) => p.typedef(t).decl,
            Element::Function(f) => return self.function_start(f),
            Element::Variable(v) => return self.variable_start(v),
            Element::Prefix(..) => return None,
        };
        Some((d.unit, p.unit(d.unit).ast.decl(d.decl).span.start))
    }

    /// Unidade e início (com metadados) de uma função, método ou construtor.
    pub fn function_start(&self, f: FunctionElementId) -> Option<(UnitId, usize)> {
        let p = &self.program;
        match p.function(f).node {
            FunctionRef::Function { unit, function } => {
                let ast = &p.unit(unit).ast;
                let start = ast
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
                Some((unit, start))
            }
            FunctionRef::Constructor { unit, member } => {
                Some((unit, p.unit(unit).ast.member(member).span.start))
            }
            FunctionRef::None => p
                .function(f)
                .variable
                .and_then(|v| self.variable_start(v)),
        }
    }

    /// Unidade e início (com metadados) de uma variável de topo, campo ou
    /// constante de enum.
    pub fn variable_start(&self, v: VariableId) -> Option<(UnitId, usize)> {
        let p = &self.program;
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
    pub fn origin(&self, local: Option<(UnitId, usize)>) -> Option<(std::path::PathBuf, usize)> {
        let (u, start) = local?;
        Some((self.program.unit(u).path.clone()?, start))
    }
}
