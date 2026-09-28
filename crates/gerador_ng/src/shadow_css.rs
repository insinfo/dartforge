//! Porte de `shadow_css.dart` do ngcompiler 3.0.0-dev.3
//! (`lib/v1/src/compiler/stylesheet_compiler/shadow_css.dart`): o shim do
//! encapsulamento emulado de estilo.
//!
//! O `shimShadowCss` lê a folha com o parser do csslib ([`crate::csslib`]),
//! reescreve cada grupo de seletores (`_ShadowTransformer`) e imprime a
//! árvore compacta (`CssPrinter`). Erro de parse vira só aviso no oficial
//! (`logWarning`) e a saída sai assim mesmo; exceção lançada no caminho
//! derruba o builder, e aqui vira [`Excecao`].
//!
//! O `_LegacyShadowTransformer` (opção `use_legacy_style_encapsulation`)
//! não está portado: a opção é falsa por omissão e o gerador não a lê.
use std::rc::Rc;

use crate::csslib::arvore::*;
use crate::csslib::impressora::CssPrinter;
use crate::csslib::token_kind as tk;
use crate::csslib::visitor::{R, Visitor};
use crate::csslib::{self, Excecao};

/// A pilha da linha que roda o shim: o parser, o visitante e a impressora
/// são recursivos como os do oficial, e o aninhamento vai até o limite do
/// parser (`csslib::parser`), bem além do que a pilha padrão de uma linha
/// comporta. A memória só é usada na medida da profundidade.
const PILHA: usize = 64 << 20;

/// `shimShadowCss(css, contentClass, hostClass)`, sem o encapsulamento
/// legado.
pub fn shim_shadow_css(
    css: &str,
    content_class: &str,
    host_class: &str,
) -> Result<String, Excecao> {
    std::thread::scope(|escopo| {
        std::thread::Builder::new()
            .name("shim-css".into())
            .stack_size(PILHA)
            .spawn_scoped(escopo, || shim_na_linha(css, content_class, host_class))
            .map_err(|_| Excecao("sem linha para o shim"))?
            .join()
            .map_err(|_| Excecao("pânico no shim"))?
    })
}

/// O corpo do `shimShadowCss`.
fn shim_na_linha(css: &str, content_class: &str, host_class: &str) -> Result<String, Excecao> {
    let mut folha = csslib::parse(css)?;
    let mut transformador = ShadowTransformer {
        content_class,
        host_class,
    };
    transformador.visit_tree(&mut folha)?;
    let mut impressora = CssPrinter::default();
    impressora.visit_tree(&mut folha)?;
    Ok(impressora.texto())
}

/// `_isHost`: `:host`.
fn is_host(s: &SeletorSimples) -> bool {
    s.e_pseudo_classe() && s.nome() == "host"
}

/// `_isHostFunction`: `:host()`.
fn is_host_function(s: &SeletorSimples) -> bool {
    s.e_pseudo_classe_funcao() && s.nome() == "host"
}

/// `_isHostContextFunction`: `:host-context()`.
fn is_host_context_function(s: &SeletorSimples) -> bool {
    s.e_pseudo_classe_funcao() && s.nome() == "host-context"
}

/// `_isGlobalContextFunction`.
fn is_global_context_function(s: &SeletorSimples) -> bool {
    s.e_pseudo_classe_funcao() && s.nome() == "global-context"
}

/// `PseudoClassFunctionSelector.selector` (`argument as Selector`).
fn argumento_seletor(s: &SeletorSimples) -> R<Rc<Seletor>> {
    match &s.tipo {
        TipoSimples::PseudoClasseFuncao(ArgumentoPseudo::Seletor(sel)) => Ok(Rc::clone(sel)),
        _ => Err(Excecao("TypeError")),
    }
}

/// `_clone`: cópia das sequências (os seletores simples são os mesmos).
fn clone_all(it: &[Rc<Sequencia>]) -> Vec<Rc<Sequencia>> {
    it.iter().map(|s| s.clonar()).collect()
}

/// `_createElementSelectorSequence`.
fn create_element_selector_sequence(nome: &str) -> Rc<Sequencia> {
    Sequencia::nova(
        SeletorSimples::criado(TipoSimples::Elemento, nome),
        tk::COMBINATOR_NONE,
    )
}

/// `_createClassSelectorSequence`.
fn create_class_selector_sequence(nome: &str) -> Rc<Sequencia> {
    Sequencia::nova(
        SeletorSimples::criado(TipoSimples::Classe, nome),
        tk::COMBINATOR_NONE,
    )
}

/// `_createPseudoClassSelectorSequence`.
fn create_pseudo_class_selector_sequence(nome: &str) -> Rc<Sequencia> {
    Sequencia::nova(
        SeletorSimples::criado(TipoSimples::PseudoClasse, nome),
        tk::COMBINATOR_NONE,
    )
}

/// `_ComplexSelector`: seletores compostos separados por combinadores.
struct ComplexSelector {
    compound_selectors: Vec<CompoundSelector>,
}

impl ComplexSelector {
    /// `_ComplexSelector.from`.
    fn from(seletor: &Seletor) -> R<Self> {
        let mut compound_selectors = Vec::new();
        let seqs = &seletor.sequencias;
        if !seqs.is_empty() {
            let mut start = 0;
            for i in 1..=seqs.len() {
                if i == seqs.len() || !seqs[i].sem_combinador() {
                    compound_selectors.push(CompoundSelector::from(&seqs[start..i])?);
                    start = i;
                }
            }
        }
        Ok(ComplexSelector { compound_selectors })
    }

    /// `containsHostContext`.
    fn contains_host_context(&self) -> bool {
        self.compound_selectors
            .iter()
            .any(|c| c.contains_host_context())
    }

    /// `toSelector`.
    fn para_seletor(&mut self) -> Seletor {
        let mut sequencias = Vec::new();
        for c in &mut self.compound_selectors {
            sequencias.extend(c.para_sequencias().iter().cloned());
        }
        Seletor { sequencias }
    }
}

/// `_CompoundSelector`: uma sequência de seletores simples, na ordem de um
/// composto válido (tipo primeiro, pseudoelemento por último).
struct CompoundSelector {
    combinator: i32,
    sequences: Vec<Rc<Sequencia>>,
}

impl CompoundSelector {
    /// `_CompoundSelector()`.
    fn novo(combinator: i32) -> Self {
        CompoundSelector {
            combinator,
            sequences: Vec::new(),
        }
    }

    /// `_CompoundSelector.from`.
    fn from(sequencias: &[Rc<Sequencia>]) -> R<Self> {
        let combinator = sequencias
            .first()
            .map(|s| s.combinador.get())
            .unwrap_or(tk::COMBINATOR_NONE);
        let mut c = CompoundSelector::novo(combinator);
        c.add_all(sequencias.to_vec())?;
        Ok(c)
    }

    /// `containsHost`.
    fn contains_host(&self) -> bool {
        self.sequences.iter().any(|s| is_host(&s.seletor))
    }

    /// `containsHostContext`.
    fn contains_host_context(&self) -> bool {
        self.sequences
            .iter()
            .any(|s| is_host_context_function(&s.seletor))
    }

    /// `containsGlobalContext`.
    fn contains_global_context(&self) -> bool {
        self.sequences
            .iter()
            .any(|s| is_global_context_function(&s.seletor))
    }

    /// `removeIfNgDeep`: `::ng-deep` vira um seletor de tipo vazio, que
    /// guarda o combinador.
    fn remove_if_ng_deep(&mut self) -> bool {
        let Some(primeiro) = self.sequences.first() else {
            return false;
        };
        let s = &primeiro.seletor;
        if s.e_pseudo_elemento() && s.nome() == "ng-deep" {
            let seq = create_element_selector_sequence("");
            seq.combinador.set(self.combinator);
            self.sequences = vec![seq];
            return true;
        }
        false
    }

    /// `_compare`: negativo se `a` vem antes de `b`, positivo se depois.
    /// O aviso de dois seletores de tipo lê `span!` — nulo nos criados pelo
    /// shim.
    fn compare(a: &Sequencia, b: &Sequencia) -> R<i32> {
        let x = &a.seletor;
        let y = &b.seletor;
        let aviso = || {
            if x.tem_span && y.tem_span {
                Ok(0)
            } else {
                Err(Excecao("TypeError"))
            }
        };
        if (x.e_elemento() && y.e_elemento()) || (x.e_namespace() && y.e_namespace()) {
            // "Compound selector contains multiple type selectors"
            aviso()
        } else if x.e_pseudo_elemento() && y.e_pseudo_elemento() {
            // "Compound selector contains multiple pseudo element selectors"
            aviso()
        } else if x.e_pseudo_elemento() || y.e_elemento() || y.e_namespace() {
            Ok(1)
        } else if y.e_pseudo_elemento() || x.e_elemento() || x.e_namespace() {
            Ok(-1)
        } else {
            Ok(0)
        }
    }

    /// `append`.
    fn append(&mut self, seq: Rc<Sequencia>) {
        self.sequences.push(seq);
    }

    /// `add`: antes do primeiro que deve vir depois dele.
    fn add(&mut self, seq: Rc<Sequencia>) -> R<()> {
        let mut i = 0;
        while i < self.sequences.len() {
            if Self::compare(&seq, &self.sequences[i])? < 0 {
                break;
            }
            i += 1;
        }
        self.sequences.insert(i, seq);
        Ok(())
    }

    /// `addAll`: intercala, mantendo a ordem de cada lado.
    fn add_all(&mut self, adicoes: Vec<Rc<Sequencia>>) -> R<()> {
        let mut novas = Vec::with_capacity(self.sequences.len() + adicoes.len());
        let mut atuais = std::mem::take(&mut self.sequences).into_iter().peekable();
        let mut adicoes = adicoes.into_iter().peekable();
        while let (Some(atual), Some(adicao)) = (atuais.peek(), adicoes.peek()) {
            if Self::compare(adicao, atual)? < 0 {
                novas.extend(adicoes.next());
            } else {
                novas.extend(atuais.next());
            }
        }
        novas.extend(atuais);
        novas.extend(adicoes);
        self.sequences = novas;
        Ok(())
    }

    /// `clone`.
    fn clonar(&self) -> Self {
        CompoundSelector {
            combinator: self.combinator,
            sequences: clone_all(&self.sequences),
        }
    }

    /// `toSequences`: acerta os combinadores (o do composto no primeiro,
    /// nenhum nos outros) e devolve a lista.
    fn para_sequencias(&mut self) -> &[Rc<Sequencia>] {
        if !self.sequences.is_empty() {
            for s in &self.sequences {
                s.combinador.set(tk::COMBINATOR_NONE);
            }
            self.sequences[0].combinador.set(self.combinator);
        }
        &self.sequences
    }
}

/// `_Indices`.
struct Indices {
    /// Primeiro composto depois de um combinador que atravessa sombra.
    deep_index: usize,
    /// Último composto com seletor de hospedeiro (`-1`: nenhum).
    host_index: isize,
}

/// `_ShadowTransformer`.
struct ShadowTransformer<'a> {
    content_class: &'a str,
    host_class: &'a str,
}

impl ShadowTransformer<'_> {
    /// `_createDescendantHostSelectorFor`: para `:host-context()`, o
    /// seletor em que o hospedeiro é descendente do argumento.
    ///
    ///   `:host-context(.x) > .y`  =>  `.x :host > .y`
    fn create_descendant_host_selector_for(
        &self,
        seletor: &mut ComplexSelector,
    ) -> R<ComplexSelector> {
        let mut novo = ComplexSelector {
            compound_selectors: Vec::new(),
        };
        for composto in &mut seletor.compound_selectors {
            if composto.contains_host_context() {
                let mut ancestral = CompoundSelector::novo(composto.combinator);
                let mut descendente = CompoundSelector::novo(tk::COMBINATOR_DESCENDANT);
                let sequencias = clone_all(composto.para_sequencias());
                for seq in sequencias {
                    if is_host_context_function(&seq.seletor) {
                        // Sem cópia: as sequências do argumento entram como
                        // estão.
                        let arg = argumento_seletor(&seq.seletor)?;
                        ancestral.add_all(arg.sequencias.clone())?;
                    } else {
                        descendente.append(seq);
                    }
                }
                descendente.add(create_pseudo_class_selector_sequence("host"))?;
                novo.compound_selectors.push(ancestral);
                novo.compound_selectors.push(descendente);
            } else {
                novo.compound_selectors.push(composto.clonar());
            }
        }
        Ok(novo)
    }

    /// `shimDeepCombinators`: troca `::ng-deep` por descendente e anota o
    /// índice.
    fn shim_deep_combinators(&self, seletor: &mut ComplexSelector, indices: &mut Indices) {
        for i in (0..seletor.compound_selectors.len()).rev() {
            if seletor.compound_selectors[i].remove_if_ng_deep() {
                indices.deep_index = i;
            }
        }
    }

    /// `shimSelectors`.
    ///
    ///   `:host(.x) > .y ::ng-deep .z`  =>  `.x.host > .y.content .z`
    fn shim_selectors(&self, seletor: &mut ComplexSelector) -> R<()> {
        let mut indices = Indices {
            deep_index: seletor.compound_selectors.len(),
            host_index: -1,
        };
        self.shim_deep_combinators(seletor, &mut indices);

        // Escopo em tudo entre o último seletor de hospedeiro e o primeiro
        // `::ng-deep`.
        let mut i = indices.deep_index as isize - 1;
        while i > indices.host_index {
            let composto = &mut seletor.compound_selectors[i as usize];
            if composto.contains_host()
                || composto.contains_host_context()
                || composto.contains_global_context()
            {
                indices.host_index = i;
            } else {
                composto.add(create_class_selector_sequence(self.content_class))?;
            }
            i -= 1;
        }

        // Os seletores de hospedeiro.
        let mut i = indices.host_index;
        while i >= 0 {
            let composto = &mut seletor.compound_selectors[i as usize];
            let mut j = 0;
            while j < composto.sequences.len() {
                let s = Rc::clone(&composto.sequences[j].seletor);
                if is_host_function(&s) || is_host_context_function(&s) {
                    // :host() ou :host-context() => a classe do hospedeiro,
                    // e o argumento entra no composto.
                    composto.sequences[j] = create_class_selector_sequence(self.host_class);
                    let arg = argumento_seletor(&s)?;
                    composto.add_all(clone_all(&arg.sequencias))?;
                } else if is_host(&s) {
                    composto.sequences[j] = create_class_selector_sequence(self.host_class);
                } else if is_global_context_function(&s) {
                    // Some o pseudo e fica o seletor de contexto.
                    composto.sequences.remove(j);
                    let arg = argumento_seletor(&s)?;
                    composto.add_all(clone_all(&arg.sequencias))?;
                }
                j += 1;
            }
            i -= 1;
        }
        Ok(())
    }
}

impl Visitor for ShadowTransformer<'_> {
    /// `visitSelectorGroup`: cada seletor vira o seu shim; com
    /// `:host-context()`, também o do hospedeiro descendente.
    fn visit_selector_group(&mut self, g: &mut GrupoSeletores) -> R<()> {
        let mut complexos = Vec::new();
        for s in &g.seletores {
            let mut complexo = ComplexSelector::from(s)?;
            let tem_contexto = complexo.contains_host_context();
            let descendente = if tem_contexto {
                Some(self.create_descendant_host_selector_for(&mut complexo)?)
            } else {
                None
            };
            complexos.push(complexo);
            complexos.extend(descendente);
        }
        g.seletores.clear();
        for mut c in complexos {
            self.shim_selectors(&mut c)?;
            g.seletores.push(c.para_seletor());
        }
        Ok(())
    }
}
