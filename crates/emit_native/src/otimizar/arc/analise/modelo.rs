//! Identidade nominal e domínio finito de inclusão (§28.1).
//! A integração com produtores/clones da HIR e resumos SDK ainda é necessária.

use std::collections::BTreeSet;

/// Identidade de origem de uma alocação, independente do ID SSA de sua cópia.
/// O produtor deve preservar `origem` e remapear especialização/contexto nos
/// clones. Construir esta chave não prova estabilidade do produtor nem singleton.
///
/// ```
/// use dartforge_emit_native::otimizar::arc::analise::modelo::SitioArc;
/// let sitio = SitioArc { funcao: "criar".into(), origem: 7, especializacao: "int".into() };
/// assert_eq!(sitio.clone(), sitio);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SitioArc {
    /// Símbolo nominal do corpo de origem, não o endereço da função ligada.
    pub funcao: String,
    /// Identificador persistente atribuído pelo produtor antes de clonar SSA.
    pub origem: u64,
    /// Instância genérica/especialização nominal da origem.
    pub especializacao: String,
}

/// Nó possível do heap ou placeholder parametrizado de um resumo.
/// Parâmetros/globais não são objetos frescos: a instanciação de resumos deve
/// substituí-los pelos aliases reais. Contextos são chaves limitadas pelo
/// produtor; este domínio não explora uma pilha de chamadas ilimitada.
///
/// ```
/// use dartforge_emit_native::otimizar::arc::analise::modelo::NoAbstrato;
/// let arg = NoAbstrato::Parametro { funcao: "ligar".into(), indice: 0 };
/// assert_ne!(arg, NoAbstrato::Global("raiz".into()));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NoAbstrato {
    /// Sítio/contexto pode representar muitas instâncias; não permite sozinho
    /// apagar arestas antigas de campo por atualização forte.
    Alocacao {
        /// Origem persistente e instância genérica.
        sitio: SitioArc,
        /// Contexto abstrato limitado e versionado pelo produtor.
        contexto: String,
    },
    /// Placeholder do argumento, incluindo o receptor quando aplicável.
    Parametro {
        /// Símbolo do resumo que define o argumento.
        funcao: String,
        /// Posição na ABI semântica do resumo.
        indice: u32,
    },
    /// Placeholder de resultado externo, sem hipótese de frescor ou separação.
    ResultadoExterno(String),
    /// Placeholder de raiz global, a substituir pelo seu alcance possível.
    Global(String),
    /// Origem explícita de ambiente, célula, frame ou outra unidade sintética.
    UnidadeSintetica {
        /// Origem produzida para a unidade sintética.
        sitio: SitioArc,
        /// Contexto abstrato da unidade.
        contexto: String,
        /// Papel nominal: não confundir célula, closure e ambiente.
        papel: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Pontos {
    Conhecidos(BTreeSet<NoAbstrato>),
    Topo,
}

/// Conjunto points-to limitado, com topo distinto do conjunto vazio.
/// Null/valores triviais podem ter conjunto vazio. Fatos ausentes, operações
/// opacas e budget excedido exigem topo, que não pode ser reduzido pela união.
/// O limite é imutável por conjunto; opções/versionamento pertencem ao plano.
/// Este domínio não certifica cardinalidade, vida, escape ou políticas ARC.
///
/// ```
/// use dartforge_emit_native::otimizar::arc::analise::modelo::{ConjuntoPontos, NoAbstrato};
/// let mut pt = ConjuntoPontos::vazio(1);
/// assert!(pt.incluir(NoAbstrato::Global("a".into())));
/// assert!(pt.incluir(NoAbstrato::Global("b".into())));
/// assert!(pt.nos().is_none()); // budget excedido não significa ausência de alias
/// assert!(!pt.unir(&ConjuntoPontos::vazio(1)));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConjuntoPontos {
    limite: usize,
    pontos: Pontos,
}

impl ConjuntoPontos {
    /// Cria o conjunto vazio; só usar quando ausência de nós for um fato.
    ///
    /// ```
    /// use dartforge_emit_native::otimizar::arc::analise::modelo::ConjuntoPontos;
    /// assert!(ConjuntoPontos::vazio(0).nos().unwrap().is_empty());
    /// ```
    pub fn vazio(limite: usize) -> Self {
        Self {
            limite,
            pontos: Pontos::Conhecidos(BTreeSet::new()),
        }
    }

    /// Cria topo para informação ausente ou invalidada; nunca equivale a vazio.
    ///
    /// ```
    /// use dartforge_emit_native::otimizar::arc::analise::modelo::ConjuntoPontos;
    /// assert!(ConjuntoPontos::desconhecido(8).nos().is_none());
    /// ```
    pub fn desconhecido(limite: usize) -> Self {
        Self {
            limite,
            pontos: Pontos::Topo,
        }
    }

    /// Consulta todos os nós conhecidos, ou `None` quando há destinos desconhecidos.
    /// O acesso imutável impede truncar ou refinar topo fora do solver.
    ///
    /// ```
    /// use dartforge_emit_native::otimizar::arc::analise::modelo::ConjuntoPontos;
    /// assert_eq!(ConjuntoPontos::vazio(8).nos().unwrap().len(), 0);
    /// ```
    pub fn nos(&self) -> Option<&BTreeSet<NoAbstrato>> {
        match &self.pontos {
            Pontos::Conhecidos(nos) => Some(nos),
            Pontos::Topo => None,
        }
    }

    /// Inclui um nó; ao exceder o limite, eleva todo o conjunto ao topo.
    /// Retorna se o estado mudou, para alimentar uma worklist monotônica.
    ///
    /// ```
    /// use dartforge_emit_native::otimizar::arc::analise::modelo::{ConjuntoPontos, NoAbstrato};
    /// let mut pt = ConjuntoPontos::vazio(0);
    /// assert!(pt.incluir(NoAbstrato::Global("raiz".into())));
    /// assert!(!pt.incluir(NoAbstrato::Global("outra".into())));
    /// ```
    pub fn incluir(&mut self, no: NoAbstrato) -> bool {
        let Pontos::Conhecidos(nos) = &mut self.pontos else {
            return false;
        };
        if nos.contains(&no) {
            return false;
        }
        if nos.len() == self.limite {
            self.pontos = Pontos::Topo;
        } else {
            nos.insert(no);
        }
        true
    }

    /// Une informação possível usando o budget do destino, sem truncamento.
    /// Topo é absorvente mesmo quando a origem tem outro limite. O booleano
    /// indica mudança sem atribuir motivo ou prova ao conjunto resultante.
    ///
    /// ```
    /// use dartforge_emit_native::otimizar::arc::analise::modelo::ConjuntoPontos;
    /// let mut pt = ConjuntoPontos::vazio(8);
    /// assert!(pt.unir(&ConjuntoPontos::desconhecido(0)));
    /// assert!(!pt.unir(&ConjuntoPontos::vazio(100)));
    /// ```
    pub fn unir(&mut self, outro: &Self) -> bool {
        if matches!(self.pontos, Pontos::Topo) {
            return false;
        }
        let Some(nos) = outro.nos() else {
            self.pontos = Pontos::Topo;
            return true;
        };
        let mut mudou = false;
        for no in nos {
            mudou |= self.incluir(no.clone());
            if self.nos().is_none() {
                break;
            }
        }
        mudou
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn conjunto(mascara: u8, limite: usize) -> ConjuntoPontos {
        let mut pt = ConjuntoPontos::vazio(limite);
        for n in 0..3 {
            if mascara & (1 << n) != 0 {
                pt.incluir(NoAbstrato::Global(n.to_string()));
            }
        }
        pt
    }

    #[test]
    fn uniao_limitada_e_monotonica_comutativa_associativa_e_idempotente() {
        for limite in 0..=3 {
            for a in 0..8 {
                for b in 0..8 {
                    for c in 0..8 {
                        let x = conjunto(a, limite);
                        let y = conjunto(b, limite);
                        let z = conjunto(c, limite);
                        let mut xy = x.clone();
                        xy.unir(&y);
                        let mut yx = y.clone();
                        yx.unir(&x);
                        assert_eq!(xy, yx);
                        assert_eq!(xy, conjunto(a | b, limite));
                        assert!(!xy.unir(&xy.clone()));
                        let mut esquerda = xy;
                        esquerda.unir(&z);
                        let mut yz = y;
                        yz.unir(&z);
                        let mut direita = x;
                        direita.unir(&yz);
                        assert_eq!(esquerda, direita);
                    }
                }
            }
        }
    }

    #[test]
    fn desconhecido_nunca_vira_vazio_e_limite_menor_nao_trunca_aliases() {
        let mut pt = ConjuntoPontos::vazio(1);
        assert!(pt.unir(&conjunto(3, 3)));
        assert!(pt.nos().is_none());
        assert!(!pt.unir(&ConjuntoPontos::vazio(8)));
        let mut vazio = ConjuntoPontos::vazio(8);
        assert!(vazio.unir(&ConjuntoPontos::desconhecido(0)));
        assert!(vazio.nos().is_none());
    }

    #[test]
    fn placeholders_e_contextos_nao_sao_colapsados_por_largura_ou_nome() {
        let sitio = SitioArc {
            funcao: "f".into(),
            origem: 1,
            especializacao: "T".into(),
        };
        let mut pt = ConjuntoPontos::vazio(5);
        for no in [
            NoAbstrato::Alocacao {
                sitio: sitio.clone(),
                contexto: "a".into(),
            },
            NoAbstrato::Alocacao {
                sitio: sitio.clone(),
                contexto: "b".into(),
            },
            NoAbstrato::Parametro {
                funcao: "f".into(),
                indice: 1,
            },
            NoAbstrato::ResultadoExterno("f".into()),
            NoAbstrato::UnidadeSintetica {
                sitio,
                contexto: "a".into(),
                papel: "celula".into(),
            },
        ] {
            assert!(pt.incluir(no));
        }
        assert_eq!(pt.nos().unwrap().len(), 5);
    }
}
