//! `ConvertToSuperParameters` (convert_to_super_parameters.dart) do
//! `analysis_server` 3.6.2: os parâmetros do construtor só repassados ao
//! `super(…)` viram parâmetros `super.`.
//!
//! | Título | Espécie |
//! |---|---|
//! | `Convert to using super parameters` | `refactor.convert.toSuperParameters` |

use crate::acoes::AcaoDeCodigo;
use crate::arvore_analyzer::Marca;
use crate::refatoracoes::Contexto;
use crate::refatoracoes_exec::Mudanca;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{FunctionElementId, FunctionRef, LibraryId, UnitId};
use dartforge_frontend::ast::{self, ExprId, MemberKind, ParameterKind};
use dartforge_types::{Resolved, Type, TypeId};
use std::collections::{HashMap, HashSet};

/// Um parâmetro do construtor da superclasse.
struct ParametroSuper {
    nome: String,
    nomeado: bool,
    tipo: TypeId,
    /// A expressão do valor padrão (unidade, biblioteca, expressão).
    padrao: Option<(UnitId, LibraryId, ExprId)>,
}

/// Um parâmetro a converter (`_ParameterData`).
struct Dados {
    final_: Option<Span>,
    tipo_a_tirar: Option<(Option<Span>, Option<Span>)>,
    nome: Span,
    inicializador_nulo: bool,
    faixa_do_padrao: Option<Span>,
    indice_do_parametro: usize,
    indice_do_argumento: usize,
}

impl Contexto<'_> {
    /// `_findConstructor`.
    fn construtor_da_selecao(&self, no: usize) -> Option<usize> {
        match self.especie(no) {
            "ConstructorDeclaration" => Some(no),
            "SimpleIdentifier" => {
                let p = self.pai(no)?;
                match self.especie(p) {
                    "ConstructorDeclaration" => Some(p),
                    "ConstructorName" => self.pai(p).filter(|&a| self.especie(a) == "ConstructorDeclaration"),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Os parâmetros do construtor da superclasse, com os tipos vistos da
    /// classe (`ConstructorMember`: os argumentos de tipo da superclasse
    /// substituídos).
    fn parametros_do_super(&self, classe: dartforge_elements::model::ClassId, sf: FunctionElementId, tabela: &mut dartforge_types::TypeTable) -> Option<Vec<ParametroSuper>> {
        let prog = self.p.programa();
        let consulta = &self.p.consulta;
        let dados = consulta.outline.functions.get(sf.0 as usize)?;
        let sfe = prog.function(sf);
        let dono = sfe.class?;
        // `C<…>` com os próprios parâmetros, visto como a classe do super.
        let parametros_c: Vec<TypeId> = consulta
            .outline
            .classes
            .get(classe.0 as usize)
            .map(|c| c.type_params.iter().map(|&p| tabela.intern(Type::TypeParameter { param: p, nullable: false })).collect())
            .unwrap_or_default();
        let este = tabela.intern(Type::Interface { class: classe, args: parametros_c.into_boxed_slice(), nullable: false });
        let mut mapa: HashMap<dartforge_types::TypeParamId, TypeId> = HashMap::new();
        if let Some(visto) = consulta.outline.hierarchy.supertype_of(este, dono, tabela, &consulta.core)
            && let Type::Interface { args, .. } = tabela.get(visto).clone()
            && let Some(cd) = consulta.outline.classes.get(dono.0 as usize)
        {
            for (p, a) in cd.type_params.iter().zip(args.iter()) {
                mapa.insert(*p, *a);
            }
        }
        // Os valores padrão, do AST do construtor da superclasse.
        let padroes: Vec<Option<(UnitId, LibraryId, ExprId)>> = match sfe.node {
            FunctionRef::Constructor { unit, member } => match &prog.unit(unit).ast.member(member).kind {
                MemberKind::Constructor(k) => k.parameters.iter().map(|p| p.default_value.map(|d| (unit, prog.unit(unit).library, d))).collect(),
                _ => Vec::new(),
            },
            _ => Vec::new(),
        };
        Some(
            dados
                .parameters
                .iter()
                .enumerate()
                .map(|(i, p)| ParametroSuper {
                    nome: p.externo.or(p.name).map(|n| self.p.nome(n).to_string()).unwrap_or_default(),
                    nomeado: p.kind == ParameterKind::Named,
                    tipo: dartforge_types::ops::substitute(p.ty, &mapa, tabela),
                    padrao: padroes.get(i).copied().flatten(),
                })
                .collect(),
        )
    }

    /// `ConvertToSuperParameters`.
    pub(crate) fn converter_em_super_parametros(&self, uri: &str, inicio: usize, fim: usize) -> Option<AcaoDeCodigo> {
        let prog = self.p.programa();
        if prog.library(prog.unit(self.unidade).library).features.versao() < dartforge_frontend::features::LanguageVersion::new(2, 17) {
            return None;
        }
        let no = self.arvore.localizar(inicio, fim)?;
        let construtor = self.construtor_da_selecao(no)?;
        // `_superInvocation`: o último `super(…)` dos inicializadores.
        let inicializadores: Vec<usize> = self
            .filhos(construtor)
            .iter()
            .copied()
            .filter(|&k| matches!(self.especie(k), "ConstructorFieldInitializer" | "SuperConstructorInvocation" | "RedirectingConstructorInvocation" | "AssertInitializer"))
            .collect();
        let invocacao = *inicializadores.iter().rev().find(|&&k| self.especie(k) == "SuperConstructorInvocation")?;
        let fim_do_no = self.arvore.nos[construtor].fim;
        let mi = self.ast.members.iter().position(|m| matches!(m.kind, MemberKind::Constructor(_)) && m.span.end == fim_do_no)?;
        let MemberKind::Constructor(k) = &self.ast.member(ast::MemberId(mi as u32)).kind else { return None };
        let f = self.p.construtor_do_no(self.unidade, ast::MemberId(mi as u32))?;
        let sf = self.p.construtor_super(f)?;
        let classe = prog.function(f).class?;
        let consulta = &self.p.consulta;
        let mut tabela = consulta.tabela.clone();
        let parametros_super = self.parametros_do_super(classe, sf, &mut tabela)?;
        let tipos_deste: Vec<TypeId> = consulta.outline.functions.get(f.0 as usize)?.parameters.iter().map(|p| p.ty).collect();
        let lista = self.filhos(construtor).iter().copied().find(|&c| self.especie(c) == "FormalParameterList")?;
        let nos_dos_parametros = self.filhos(lista).to_vec();
        // `_parameterMap`: os simples e os de função, pelo offset do nome.
        let mut mapa: HashMap<usize, usize> = HashMap::new();
        for (i, &p) in nos_dos_parametros.iter().enumerate() {
            let interno = if self.especie(p) == "DefaultFormalParameter" { *self.filhos(p).first()? } else { p };
            if matches!(self.especie(interno), "SimpleFormalParameter" | "FunctionTypedFormalParameter")
                && let Some(n) = k.parameters.get(i).and_then(|x| x.name)
            {
                mapa.insert(n.span.start, i);
            }
        }
        // O parâmetro (índice) que um identificador cita.
        let parametro_de = |e: usize| -> Option<usize> {
            if self.especie(e) != "SimpleIdentifier" {
                return None;
            }
            let Marca::Expr(x) = self.arvore.nos[e].marca else { return None };
            let decl = match self.corpos.get_resolved(x)? {
                Resolved::Local(_) => self.corpos.declaracao_local(x)?,
                Resolved::Parameter { name, .. } => k.parameters.iter().find(|p| p.name.is_some_and(|n| n.sym == *name))?.name?.span.start,
                _ => return None,
            };
            mapa.get(&decl).copied()
        };
        // `_referencedParameters`: os parâmetros citados no corpo.
        let mut citados: HashSet<usize> = HashSet::new();
        if let Some(&corpo) = self.filhos(construtor).iter().find(|&&c| matches!(self.especie(c), "BlockFunctionBody" | "ExpressionFunctionBody")) {
            let mut pilha = vec![corpo];
            while let Some(n) = pilha.pop() {
                if self.especie(n) == "SimpleIdentifier"
                    && let Marca::Expr(x) = self.arvore.nos[n].marca
                {
                    match self.corpos.get_resolved(x) {
                        Some(Resolved::Parameter { name, .. }) => {
                            if let Some(i) = k.parameters.iter().position(|p| p.name.is_some_and(|q| q.sym == *name)) {
                                citados.insert(i);
                            }
                        }
                        Some(Resolved::Local(_)) => {
                            if let Some(d) = self.corpos.declaracao_local(x)
                                && let Some(&i) = mapa.get(&d)
                            {
                                citados.insert(i);
                            }
                        }
                        _ => {}
                    }
                }
                pilha.extend(self.filhos(n).iter().copied());
            }
        }
        let argumentos = self.argumentos_da_lista(invocacao);
        let mut motor = None;
        let mut dados_de = |i: usize, indice_arg: usize, super_p: &ParametroSuper, tabela: &mut dartforge_types::TypeTable| -> Option<Dados> {
            let este_tipo = *tipos_deste.get(i)?;
            let mut env = dartforge_types::subtyping::SubtypeEnv::new(tabela, &consulta.outline.hierarchy, &consulta.core);
            if !dartforge_types::subtyping::is_subtype(este_tipo, super_p.tipo, &mut env) {
                return None;
            }
            let p = k.parameters.get(i)?;
            let nome = p.name?.span;
            let no_p = nos_dos_parametros[i];
            let interno = if self.especie(no_p) == "DefaultFormalParameter" { *self.filhos(no_p).first()? } else { no_p };
            // `_defaultValueRange`: o padrão igual ao do super sai.
            let faixa_do_padrao = match (p.default_value, super_p.padrao) {
                (Some(d), Some((su, sl, se))) => {
                    let m = motor.get_or_insert_with(|| tabela.clone());
                    let mut mt = dartforge_types::constantes::avaliador::Motor::novo(prog, &consulta.nomes, m, &consulta.core, &consulta.outline, &consulta.corpos, &self.p.bibliotecas);
                    let lib = prog.unit(self.unidade).library;
                    let deste = mt.avaliar(&dartforge_types::constantes::avaliador::Ctx::simples(self.unidade, lib), d, true);
                    let dele = mt.avaliar(&dartforge_types::constantes::avaliador::Ctx::simples(su, sl), se, true);
                    match (dele.valor(), deste.valor()) {
                        (Some(a), Some(b)) if mt.iguais(a, b) => Some(Span { start: nome.end, end: self.ast.expr(d).span.end }),
                        _ => None,
                    }
                }
                _ => None,
            };
            let final_ = if p.final_ && self.especie(interno) == "SimpleFormalParameter" {
                self.tokens.iter().find(|t| t.span.start >= self.arvore.nos[interno].inicio && t.span.end <= nome.start && &self.fonte[t.span.start..t.span.end] == "final").map(|t| t.span)
            } else {
                None
            };
            let inicializador_nulo = p.kind != ParameterKind::Required && !p.required && p.default_value.is_none() && super_p.padrao.is_some();
            let tipo_a_tirar = if este_tipo == super_p.tipo {
                match self.especie(interno) {
                    "SimpleFormalParameter" => p.ty.map(|t| (Some(Span { start: self.ast.ty(t).span.start, end: nome.start }), None)),
                    "FunctionTypedFormalParameter" => {
                        let lista_f = self.filhos(interno).iter().rev().find(|&&c| self.especie(c) == "FormalParameterList").map(|&l| self.arvore.span(l));
                        Some((p.ty.map(|t| Span { start: self.ast.ty(t).span.start, end: nome.start }), lista_f))
                    }
                    _ => None,
                }
            } else {
                None
            };
            Some(Dados { final_, tipo_a_tirar, nome, inicializador_nulo, faixa_do_padrao, indice_do_parametro: i, indice_do_argumento: indice_arg })
        };
        let mut posicionais: Option<Vec<Dados>> = Some(Vec::new());
        let mut nomeados: Vec<Dados> = Vec::new();
        let mut posicao = 0usize;
        let super_posicionais: Vec<&ParametroSuper> = parametros_super.iter().filter(|p| !p.nomeado).collect();
        for (ia, &arg) in argumentos.iter().enumerate() {
            if self.especie(arg) == "NamedExpression" {
                let f = self.filhos(arg);
                let rotulo = self.texto_do_no(*f.first()?).trim_end_matches(':').trim().to_string();
                let Some(&exp) = f.get(1) else { continue };
                if let Some(i) = parametro_de(exp)
                    && k.parameters[i].kind == ParameterKind::Named
                    && k.parameters[i].nome_externo().is_some_and(|n| self.p.nome(n.sym) == rotulo)
                    && !citados.contains(&i)
                    && let Some(sp) = parametros_super.iter().find(|p| p.nomeado && p.nome == rotulo)
                    && let Some(d) = dados_de(i, ia, sp, &mut tabela)
                {
                    nomeados.push(d);
                }
            } else {
                let super_p = super_posicionais.get(posicao).copied();
                posicao += 1;
                if posicionais.is_some() {
                    match parametro_de(arg) {
                        Some(i) if k.parameters[i].kind != ParameterKind::Named && !citados.contains(&i) => {
                            match super_p.and_then(|sp| dados_de(i, ia, sp, &mut tabela)) {
                                Some(d) => posicionais.as_mut().expect("posicionais").push(d),
                                None => posicionais = None,
                            }
                        }
                        _ => posicionais = None,
                    }
                }
            }
        }
        // `_inOrder`.
        if let Some(p) = &posicionais
            && p.windows(2).any(|w| w[1].indice_do_parametro < w[0].indice_do_parametro)
        {
            posicionais = None;
        }
        if posicionais.as_ref().is_none_or(Vec::is_empty) && nomeados.is_empty() {
            return None;
        }
        let todos: Vec<Dados> = posicionais.unwrap_or_default().into_iter().chain(nomeados).collect();
        let mut a_tirar: Vec<usize> = todos.iter().map(|d| d.indice_do_argumento).collect();
        a_tirar.sort_unstable();
        let mut m = Mudanca::default();
        let antes = |m: &mut Mudanca, s: usize| m.adicionar(uri, Span { start: s, end: s }, "super.");
        for d in &todos {
            let inserir_super = |m: &mut Mudanca| match d.final_ {
                None => antes(m, d.nome.start),
                Some(kw) => {
                    let depois = self.token_seguinte(kw.end).map_or(d.nome.start, |s| s.start);
                    if depois == d.nome.start {
                        m.adicionar(uri, Span { start: kw.start, end: depois }, "super.");
                    } else {
                        m.adicionar(uri, Span { start: kw.start, end: depois }, String::new());
                        antes(m, d.nome.start);
                    }
                }
            };
            match &d.tipo_a_tirar {
                None => inserir_super(&mut m),
                Some((primaria, faixa_dos_parametros)) => {
                    match primaria {
                        None => antes(&mut m, d.nome.start),
                        Some(pr) => match d.final_ {
                            None => m.adicionar(uri, *pr, "super."),
                            Some(kw) => {
                                let depois = self.token_seguinte(kw.end).map_or(pr.start, |s| s.start);
                                if depois == pr.start {
                                    m.adicionar(uri, Span { start: kw.start, end: pr.end }, "super.");
                                } else {
                                    m.adicionar(uri, Span { start: kw.start, end: depois }, String::new());
                                    m.adicionar(uri, *pr, "super.");
                                }
                            }
                        },
                    }
                    if d.inicializador_nulo {
                        m.adicionar(uri, Span { start: d.nome.end, end: d.nome.end }, " = null");
                    }
                    if let Some(fp) = faixa_dos_parametros {
                        m.adicionar(uri, *fp, String::new());
                    }
                }
            }
            if let Some(fp) = d.faixa_do_padrao {
                m.adicionar(uri, fp, String::new());
            }
        }
        let nome_do_super = self.filhos(invocacao).iter().any(|&c| self.especie(c) == "SimpleIdentifier");
        let lista_de_argumentos = self.filhos(invocacao).iter().copied().find(|&c| self.especie(c) == "ArgumentList")?;
        if a_tirar.len() == argumentos.len() {
            if !nome_do_super {
                let faixa = if inicializadores.len() == 1 {
                    Span { start: self.arvore.nos[lista].fim, end: self.arvore.nos[invocacao].fim }
                } else {
                    let i = inicializadores.iter().position(|&x| x == invocacao)?;
                    if i == 0 {
                        Span { start: self.arvore.nos[invocacao].inicio, end: self.arvore.nos[inicializadores[1]].inicio }
                    } else {
                        Span { start: self.arvore.nos[inicializadores[i - 1]].fim, end: self.arvore.nos[invocacao].fim }
                    }
                };
                m.adicionar(uri, faixa, String::new());
            } else {
                let s = self.arvore.span(lista_de_argumentos);
                m.adicionar(uri, Span { start: s.start + 1, end: s.end - 1 }, String::new());
            }
        } else {
            // `nodesInList`.
            let mut grupos: Vec<(usize, usize)> = Vec::new();
            for &i in &a_tirar {
                match grupos.last_mut() {
                    Some((_, fim)) if *fim + 1 == i => *fim = i,
                    _ => grupos.push((i, i)),
                }
            }
            for (de, ate) in grupos {
                let faixa = if de == 0 {
                    Span { start: self.arvore.nos[argumentos[de]].inicio, end: self.arvore.nos[argumentos[ate + 1]].inicio }
                } else {
                    Span { start: self.arvore.nos[argumentos[de - 1]].fim, end: self.arvore.nos[argumentos[ate]].fim }
                };
                m.adicionar(uri, faixa, String::new());
            }
        }
        if m.conflito.is_some() {
            return None;
        }
        Some(AcaoDeCodigo {
            titulo: "Convert to using super parameters".into(),
            especie: "refactor.convert.toSuperParameters".into(),
            edicoes: m.arquivos.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(),
            diagnostico: None,
            criar_arquivo: None,
        })
    }
}
