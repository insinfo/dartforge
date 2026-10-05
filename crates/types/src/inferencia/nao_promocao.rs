//! Why-not-promoted: as mensagens de contexto que explicam por que uma
//! expressão não está promovida (`computeWhyNotPromotedMessages` e o
//! `_WhyNotPromotedVisitor`, `an611:src/generated/resolver.dart:699-739` e
//! `:5561-5736`; o `whyNotPromoted` do fluxo,
//! `_fe_analyzer_shared/lib/src/flow_analysis/flow_analysis.dart:5581-5822`).
//!
//! Na leitura de cada referência (variável local ou propriedade) guarda-se
//! o mapa `tipo -> motivo` daquele ponto, como o `whyNotPromoted(target)`
//! chamado logo depois da expressão; `this` (inclusive o implícito) é
//! consultado na hora do erro. Quem relata um erro que o analyzer acompanha
//! dessas mensagens chama [`BodyInferrer::anexar_nao_promocao`].

use super::corpo::{Base, Corpo};
use super::fluxo::MotivoDeNaoPromocao;
use super::BodyInferrer;
use crate::promocao_de_campos::{self, PorQueNaoPromove, PromocaoDaBiblioteca};
use crate::resolved::LocalId;
use crate::table::TypeId;
use dartforge_diagnostics::{Contexto, Span};
use dartforge_elements::model::{ClassId, ClassKind, FunctionElementId, FunctionKind, LibraryId, Program, UnitId};
use dartforge_frontend::ast::{ExprId, ExprKind};
use dartforge_intern::SymbolId;
use std::rc::Rc;

/// `ElementKind.displayName` de uma classe.
fn especie(program: &Program, c: ClassId) -> &'static str {
    match program.class(c).kind {
        ClassKind::Class | ClassKind::MixinApplication => "class",
        ClassKind::Mixin => "mixin",
        ClassKind::Enum => "enum",
        ClassKind::ExtensionType => "extension type",
    }
}

/// Uma mensagem de contexto num ponto de `u` (o arquivo, quando não é o do
/// erro).
fn contexto(program: &Program, unidade_do_erro: UnitId, u: UnitId, span: Span, mensagem: String) -> Contexto {
    Contexto { arquivo: (u != unidade_do_erro).then(|| program.caminho_da_unidade(u).into()), span, mensagem: mensagem.into() }
}

impl BodyInferrer<'_> {
    /// O `FieldPromotability` de uma biblioteca (calculado uma vez).
    pub(crate) fn promocao_da_biblioteca(&mut self, lib: LibraryId) -> Rc<PromocaoDaBiblioteca> {
        if let Some(p) = self.promocao_por_biblioteca.get(&lib) {
            return p.clone();
        }
        let p = Rc::new(promocao_de_campos::calcular(self.program, self.interner, lib));
        self.promocao_por_biblioteca.insert(lib, p.clone());
        p
    }

    /// `_handleProperty`: `fieldPromotionEnabled` no corpo (`lib`) e
    /// `isPropertyPromotable` do membro (o getter de um campo, ou o getter
    /// abstrato, cujo campo tem `isPromotable`).
    pub(crate) fn propriedade_promovivel(&mut self, lib: LibraryId, f: FunctionElementId) -> bool {
        if !promocao_de_campos::habilitada(self.program, lib) {
            return false;
        }
        let fe = self.program.function(f);
        match fe.kind {
            FunctionKind::ImplicitAccessor => match fe.variable {
                Some(v) => {
                    let dono = self.program.variable(v).library;
                    self.promocao_da_biblioteca(dono).campos.contains(&v)
                }
                None => false,
            },
            FunctionKind::Getter => {
                let dono = fe.library;
                self.promocao_da_biblioteca(dono).getters.contains(&f)
            }
            _ => false,
        }
    }

    /// O `whyPropertyIsNotPromotable` (com o nome público tratado antes,
    /// como o fluxo faz): `None` é o motivo não inerente (conflito, ou o
    /// recurso desligado).
    fn por_que_nao_promove(&mut self, nome: SymbolId, f: FunctionElementId) -> Option<PorQueNaoPromove> {
        if !self.interner.resolve(nome).starts_with('_') {
            return Some(PorQueNaoPromove::NaoEPrivado);
        }
        let fe = self.program.function(f);
        // Um método, ou um getter escrito (o campo dele é sintético).
        if fe.kind != FunctionKind::ImplicitAccessor {
            return Some(PorQueNaoPromove::NaoECampo);
        }
        let Some(v) = fe.variable else { return Some(PorQueNaoPromove::NaoECampo) };
        let ve = self.program.variable(v);
        if ve.class.is_none() && ve.extension.is_none() {
            return Some(PorQueNaoPromove::NaoECampo);
        }
        let (externo, final_, dono) = (ve.external, ve.final_, ve.library);
        if self.promocao_da_biblioteca(dono).campos.contains(&v) {
            return None;
        }
        if externo {
            return Some(PorQueNaoPromove::Externo);
        }
        if !final_ {
            return Some(PorQueNaoPromove::NaoFinal);
        }
        None
    }

    /// `whyNotPromoted` da leitura `e` da variável `id`: do histórico de
    /// não promoção, cada tipo de que o tipo atual não é subtipo (o
    /// primeiro motivo de cada tipo fica).
    pub(crate) fn registrar_nao_promocao_local(&mut self, cx: &Corpo, e: ExprId, id: LocalId) {
        let chave = (cx.unit, e);
        let Some(m) = cx.fluxo.modelo(id) else {
            self.nao_promocoes.remove(&chave);
            return;
        };
        let atual = m.cadeia.last().copied().unwrap_or(cx.local(id).tipo);
        let mut historico = m.historico.clone();
        let mut r: Vec<(TypeId, MotivoDeNaoPromocao)> = Vec::new();
        while let Some(no) = historico {
            if !self.sub(atual, no.tipo) && !r.iter().any(|(t, _)| *t == no.tipo) {
                r.push((no.tipo, no.motivo.clone()));
            }
            historico = no.anterior.clone();
        }
        if r.is_empty() {
            self.nao_promocoes.remove(&chave);
        } else {
            self.nao_promocoes.insert(chave, r);
        }
    }

    /// `whyNotPromoted` da leitura `e` de uma propriedade não promovível: os
    /// tipos promovidos (no fluxo de agora) de cada geração anterior, da mais
    /// nova à mais antiga, todos com o mesmo motivo.
    pub(crate) fn registrar_nao_promocao_de_propriedade(&mut self, cx: &Corpo, e: ExprId, chave_da_propriedade: (Base, u32, SymbolId), f: FunctionElementId) {
        let chave = (cx.unit, e);
        let propria = cx.geracao_da_leitura.get(&e).copied();
        let mut listas: Vec<Vec<TypeId>> = Vec::new();
        if let Some(geracoes) = cx.geracoes.get(&chave_da_propriedade) {
            for &g in geracoes.iter().rev() {
                if Some(g) == propria {
                    continue;
                }
                if let Some(m) = cx.fluxo.modelo(g) {
                    if !m.cadeia.is_empty() {
                        listas.push(m.cadeia.clone());
                    }
                }
            }
        }
        if listas.is_empty() {
            self.nao_promocoes.remove(&chave);
            return;
        }
        let nome = chave_da_propriedade.2;
        let inerente = self.por_que_nao_promove(nome, f);
        let habilitada = promocao_de_campos::habilitada(self.program, cx.lib);
        let motivo = MotivoDeNaoPromocao::Propriedade { nome, membro: f, inerente, habilitada };
        let mut r: Vec<(TypeId, MotivoDeNaoPromocao)> = Vec::new();
        for l in listas {
            for t in l {
                if !r.iter().any(|(u, _)| *u == t) {
                    r.push((t, motivo.clone()));
                }
            }
        }
        self.nao_promocoes.insert(chave, r);
    }

    /// `whyNotPromotedImplicitThis` (e o de `this`/`super` escritos): cada
    /// tipo promovido de `this`, com `ThisNotPromoted`.
    fn motivos_de_this(&self, cx: &Corpo) -> Vec<(TypeId, MotivoDeNaoPromocao)> {
        let Some(id) = cx.local_this else { return Vec::new() };
        let Some(m) = cx.fluxo.modelo(id) else { return Vec::new() };
        m.cadeia.iter().map(|&t| (t, MotivoDeNaoPromocao::This)).collect()
    }

    /// `computeWhyNotPromotedMessages(errorEntity, whyNotPromoted(alvo))`:
    /// as mensagens do primeiro tipo não potencialmente anulável do mapa.
    /// `alvo` `None` é o `this` implícito; `erro` é o `errorEntity`.
    pub(crate) fn mensagens_de_nao_promocao(&mut self, cx: &Corpo, alvo: Option<ExprId>, erro: Span) -> Vec<Contexto> {
        let motivos = match alvo {
            Some(e) => {
                let a = &self.program.unit(cx.unit).ast;
                let mut x = e;
                while let ExprKind::Parenthesized(i) = &a.expr(x).kind {
                    x = *i;
                }
                match &a.expr(x).kind {
                    ExprKind::This | ExprKind::Super => self.motivos_de_this(cx),
                    _ => self.nao_promocoes.get(&(cx.unit, x)).cloned().unwrap_or_default(),
                }
            }
            None => self.motivos_de_this(cx),
        };
        for (t, m) in motivos {
            if !self.e_nao_anulavel(t) {
                continue;
            }
            return self.mensagens_do_motivo(cx, &m, erro);
        }
        Vec::new()
    }

    /// O `_WhyNotPromotedVisitor`.
    fn mensagens_do_motivo(&mut self, cx: &Corpo, m: &MotivoDeNaoPromocao, erro: Span) -> Vec<Contexto> {
        match m {
            MotivoDeNaoPromocao::Escrita { nome, span } => {
                let texto = format!("Variable '{}' could not be promoted due to an assignment", self.interner.resolve(*nome));
                vec![Contexto { arquivo: None, span: *span, mensagem: texto.into() }]
            }
            MotivoDeNaoPromocao::This => vec![Contexto { arquivo: None, span: erro, mensagem: "'this' can't be promoted".into() }],
            MotivoDeNaoPromocao::Propriedade { nome, membro, inerente, habilitada } => {
                let program = self.program;
                let fe = program.function(*membro);
                // `receiverElement is PropertyAccessorElement`: só getters.
                if !matches!(fe.kind, FunctionKind::Getter | FunctionKind::ImplicitAccessor) {
                    return Vec::new();
                }
                let texto = self.interner.resolve(*nome).to_string();
                let onde = program.nome_nao_sintetico_da_funcao(*membro);
                let indisponivel = format!("'{texto}' couldn't be promoted because field promotion is only available in Dart 3.2 and above.");
                let mut r = Vec::new();
                match inerente {
                    Some(pq) => {
                        let mensagem = match pq {
                            PorQueNaoPromove::NaoECampo => format!("'{texto}' refers to a getter so it couldn't be promoted."),
                            PorQueNaoPromove::NaoEPrivado => format!("'{texto}' refers to a public property so it couldn't be promoted."),
                            PorQueNaoPromove::Externo => format!("'{texto}' refers to an external field so it couldn't be promoted."),
                            PorQueNaoPromove::NaoFinal => format!("'{texto}' refers to a non-final field so it couldn't be promoted."),
                        };
                        if let Some((u, s)) = onde {
                            r.push(contexto(program, cx.unit, u, s, mensagem));
                            if !habilitada {
                                r.push(contexto(program, cx.unit, u, s, indisponivel));
                            }
                        }
                    }
                    None => {
                        let info = self.promocao_da_biblioteca(fe.library);
                        if let Some(i) = info.info.get(nome) {
                            let conflito = |tipo: &str, c: Option<ClassId>| {
                                let (k, n) = match c {
                                    Some(c) => (especie(program, c), self.interner.resolve(program.class(c).name).to_string()),
                                    None => ("class", String::new()),
                                };
                                format!("'{texto}' couldn't be promoted because there is a conflicting {tipo} in {k} '{n}'")
                            };
                            for &v in &i.campos {
                                if let Some((u, s)) = program.nome_da_variavel(v) {
                                    r.push(contexto(program, cx.unit, u, s, conflito("non-promotable field", program.variable(v).class)));
                                }
                            }
                            for &g in &i.getters {
                                if let Some((u, s)) = program.nome_nao_sintetico_da_funcao(g) {
                                    r.push(contexto(program, cx.unit, u, s, conflito("getter", program.function(g).class)));
                                }
                            }
                            for &c in &i.classes_nsm {
                                if let Some((u, s)) = program.nome_da_classe(c) {
                                    r.push(contexto(program, cx.unit, u, s, conflito("noSuchMethod forwarder", Some(c))));
                                }
                            }
                        }
                        if !habilitada {
                            if let Some((u, s)) = onde {
                                r.push(contexto(program, cx.unit, u, s, indisponivel));
                            }
                        }
                    }
                }
                r
            }
        }
    }

    /// Anexa as mensagens do why-not-promoted de `alvo` aos diagnósticos
    /// relatados desde `desde` (o erro que o analyzer acompanha delas).
    pub(crate) fn anexar_nao_promocao(&mut self, desde: usize, cx: &Corpo, alvo: Option<ExprId>, erro: Span) {
        if self.diagnostics.len() <= desde {
            return;
        }
        let mensagens = self.mensagens_de_nao_promocao(cx, alvo, erro);
        if mensagens.is_empty() {
            return;
        }
        for d in &mut self.diagnostics[desde..] {
            d.contexto.extend(mensagens.iter().cloned());
        }
    }
}
