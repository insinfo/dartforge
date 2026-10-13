//! Identidade nominal e domínio finito de inclusão (§28.1).
//! A integração com produtores/clones da HIR e resumos SDK ainda é necessária.

use std::collections::BTreeSet;

/// Identidade de origem de uma alocação, independente do ID SSA de sua cópia.
/// O produtor deve preservar `origin` e remapear especialização/contexto nos
/// clones. Construir esta chave não prova estabilidade do produtor nem singleton.
///
/// ```
/// use dartforge_emit_native::optimize::arc::analise::modelo::SitioArc;
/// let sitio = SitioArc { funcao: "criar".into(), origem: 7, especializacao: "int".into() };
/// assert_eq!(sitio.clone(), sitio);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArcSite {
    /// Símbolo nominal do corpo de origem, não o endereço da função ligada.
    pub function: String,
    /// Identificador persistente atribuído pelo produtor antes de clonar SSA.
    pub origin: u64,
    /// Instância genérica/especialização nominal da origem.
    pub specialization: String,
}

/// Nó possível do heap ou placeholder parametrizado de um resumo.
/// Parâmetros/globais não são objetos frescos: a instanciação de resumos deve
/// substituí-los pelos aliases reais. Contextos são chaves limitadas pelo
/// produtor; este domínio não explora uma pilha de chamadas ilimitada.
///
/// ```
/// use dartforge_emit_native::optimize::arc::analise::modelo::NoAbstrato;
/// let arg = NoAbstrato::Parametro { funcao: "ligar".into(), indice: 0 };
/// assert_ne!(arg, NoAbstrato::Global("raiz".into()));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AbstractNode {
    /// Sítio/contexto pode representar muitas instâncias; não permite sozinho
    /// apagar arestas antigas de campo por atualização forte.
    Allocation {
        /// Origem persistente e instância genérica.
        site: ArcSite,
        /// Contexto abstrato limitado e versionado pelo produtor.
        context: String,
    },
    /// Placeholder do argumento, incluindo o receptor quando aplicável.
    Parameter {
        /// Símbolo do resumo que define o argumento.
        function: String,
        /// Posição na ABI semântica do resumo.
        index: u32,
    },
    /// Placeholder de resultado externo, sem hipótese de frescor ou separação.
    ExternalResult(String),
    /// Placeholder de raiz global, a substituir pelo seu alcance possível.
    Global(String),
    /// Origem explícita de ambiente, célula, frame ou outra unidade sintética.
    SyntheticUnit {
        /// Origem produzida para a unidade sintética.
        site: ArcSite,
        /// Contexto abstrato da unidade.
        context: String,
        /// Papel nominal: não confundir célula, closure e ambiente.
        role: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Points {
    KnownSet(BTreeSet<AbstractNode>),
    Top,
}

/// Conjunto points-to limitado, com topo distinto do conjunto vazio.
/// Null/valores triviais podem ter conjunto vazio. Fatos ausentes, operações
/// opacas e budget excedido exigem topo, que não pode ser reduzido pela união.
/// O limite é imutável por conjunto; opções/versionamento pertencem ao plano.
/// Este domínio não certifica cardinalidade, vida, escape ou políticas ARC.
///
/// ```
/// use dartforge_emit_native::optimize::arc::analise::modelo::{ConjuntoPontos, NoAbstrato};
/// let mut pt = ConjuntoPontos::vazio(1);
/// assert!(pt.incluir(NoAbstrato::Global("a".into())));
/// assert!(pt.incluir(NoAbstrato::Global("b".into())));
/// assert!(pt.nos().is_none()); // budget excedido não significa ausência de alias
/// assert!(!pt.unir(&ConjuntoPontos::vazio(1)));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PointSet {
    limit: usize,
    points: Points,
}

impl PointSet {
    pub(super) fn internal_limit(&self) -> usize {
        self.limit
    }

    /// Cria o conjunto vazio; só usar quando ausência de nós for um fato.
    ///
    /// ```
    /// use dartforge_emit_native::optimize::arc::analise::modelo::ConjuntoPontos;
    /// assert!(ConjuntoPontos::vazio(0).nos().unwrap().is_empty());
    /// ```
    pub fn empty(limit: usize) -> Self {
        Self {
            limit,
            points: Points::KnownSet(BTreeSet::new()),
        }
    }

    /// Cria topo para informação ausente ou invalidada; nunca equivale a vazio.
    ///
    /// ```
    /// use dartforge_emit_native::optimize::arc::analise::modelo::ConjuntoPontos;
    /// assert!(ConjuntoPontos::desconhecido(8).nos().is_none());
    /// ```
    pub fn unknown(limit: usize) -> Self {
        Self {
            limit,
            points: Points::Top,
        }
    }

    /// Consulta todos os nós conhecidos, ou `None` quando há destinos desconhecidos.
    /// O acesso imutável impede truncar ou refinar topo fora do solver.
    ///
    /// ```
    /// use dartforge_emit_native::optimize::arc::analise::modelo::ConjuntoPontos;
    /// assert_eq!(ConjuntoPontos::vazio(8).nos().unwrap().len(), 0);
    /// ```
    pub fn nodes(&self) -> Option<&BTreeSet<AbstractNode>> {
        match &self.points {
            Points::KnownSet(nodes) => Some(nodes),
            Points::Top => None,
        }
    }

    /// Inclui um nó; ao exceder o limite, eleva todo o conjunto ao topo.
    /// Retorna se o estado mudou, para alimentar uma worklist monotônica.
    ///
    /// ```
    /// use dartforge_emit_native::optimize::arc::analise::modelo::{ConjuntoPontos, NoAbstrato};
    /// let mut pt = ConjuntoPontos::vazio(0);
    /// assert!(pt.incluir(NoAbstrato::Global("raiz".into())));
    /// assert!(!pt.incluir(NoAbstrato::Global("outra".into())));
    /// ```
    pub fn include(&mut self, node: AbstractNode) -> bool {
        let Points::KnownSet(nodes) = &mut self.points else {
            return false;
        };
        if nodes.contains(&node) {
            return false;
        }
        if nodes.len() == self.limit {
            self.points = Points::Top;
        } else {
            nodes.insert(node);
        }
        true
    }

    /// Une informação possível usando o budget do destino, sem truncamento.
    /// Topo é absorvente mesmo quando a origem tem outro limite. O booleano
    /// indica mudança sem atribuir motivo ou prova ao conjunto resultante.
    ///
    /// ```
    /// use dartforge_emit_native::optimize::arc::analise::modelo::ConjuntoPontos;
    /// let mut pt = ConjuntoPontos::vazio(8);
    /// assert!(pt.unir(&ConjuntoPontos::desconhecido(0)));
    /// assert!(!pt.unir(&ConjuntoPontos::vazio(100)));
    /// ```
    pub fn join_set(&mut self, other: &Self) -> bool {
        if matches!(self.points, Points::Top) {
            return false;
        }
        let Some(nodes) = other.nodes() else {
            self.points = Points::Top;
            return true;
        };
        let mut changed = false;
        for node in nodes {
            changed |= self.include(node.clone());
            if self.nodes().is_none() {
                break;
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(mask: u8, limit: usize) -> PointSet {
        let mut pt = PointSet::empty(limit);
        for n in 0..3 {
            if mask & (1 << n) != 0 {
                pt.include(AbstractNode::Global(n.to_string()));
            }
        }
        pt
    }

    #[test]
    fn bounded_union_is_monotonic_commutative_associative_and_idempotent() {
        for limit in 0..=3 {
            for a in 0..8 {
                for b in 0..8 {
                    for c in 0..8 {
                        let x = set(a, limit);
                        let y = set(b, limit);
                        let z = set(c, limit);
                        let mut xy = x.clone();
                        xy.join_set(&y);
                        let mut yx = y.clone();
                        yx.join_set(&x);
                        assert_eq!(xy, yx);
                        assert_eq!(xy, set(a | b, limit));
                        assert!(!xy.join_set(&xy.clone()));
                        let mut left = xy;
                        left.join_set(&z);
                        let mut yz = y;
                        yz.join_set(&z);
                        let mut right = x;
                        right.join_set(&yz);
                        assert_eq!(left, right);
                    }
                }
            }
        }
    }

    #[test]
    fn unknown_never_becomes_empty_and_smaller_limit_does_not_truncate_aliases() {
        let mut pt = PointSet::empty(1);
        assert!(pt.join_set(&set(3, 3)));
        assert!(pt.nodes().is_none());
        assert!(!pt.join_set(&PointSet::empty(8)));
        let mut empty = PointSet::empty(8);
        assert!(empty.join_set(&PointSet::unknown(0)));
        assert!(empty.nodes().is_none());
    }

    #[test]
    fn placeholders_and_contexts_are_not_collapsed_by_width_or_name() {
        let site = ArcSite {
            function: "f".into(),
            origin: 1,
            specialization: "T".into(),
        };
        let mut pt = PointSet::empty(5);
        for node in [
            AbstractNode::Allocation {
                site: site.clone(),
                context: "a".into(),
            },
            AbstractNode::Allocation {
                site: site.clone(),
                context: "b".into(),
            },
            AbstractNode::Parameter {
                function: "f".into(),
                index: 1,
            },
            AbstractNode::ExternalResult("f".into()),
            AbstractNode::SyntheticUnit {
                site,
                context: "a".into(),
                role: "celula".into(),
            },
        ] {
            assert!(pt.include(node));
        }
        assert_eq!(pt.nodes().unwrap().len(), 5);
    }
}
