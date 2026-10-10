//! Certificado do inventário usado pelo cleanup estrangeiro dos pousos.
use super::*;
use std::collections::{HashMap, HashSet};

/// Inventário de cleanup ligado ao corpo e às tabelas que foram verificados.
///
/// Criado pela preparação ARC do conjunto de funções; campos privados evitam
/// inventar owners no emissor. Mudanças posteriores exigem nova preparação.
/// Quadros locais acompanham a pilha LIFO validada; aliases e quadros
/// importados continuam fora do protocolo verificado.
///
/// ```
/// use dartforge_emit_native::otimizar::arc::PlanoFuncaoDart;
/// let plano = PlanoFuncaoDart::default();
/// assert!(plano.tabelas.cleanup_estrangeiro.is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupEstrangeiro {
    corpo: String,
    invocacoes: HashMap<ValueId, BlockId>,
    pousos: HashSet<BlockId>,
    saidas: HashMap<BlockId, SaidaPorExcecao>,
    confere_pilha: bool,
    owners: HashMap<BlockId, Vec<ValueId>>,
    quadros: HashMap<BlockId, Vec<ValueId>>,
}

impl CleanupEstrangeiro {
    pub(super) fn novo(f: &Function, plano: &PlanoFuncaoDart) -> Result<Option<Self>, String> {
        if !plano
            .tabelas
            .pousos
            .iter()
            .any(|b| plano.tabelas.saidas.get(b) != Some(&SaidaPorExcecao::Retoma))
        {
            return Ok(None);
        }
        let quadros = super::quadros::na_entrada_dos_pousos(f, &plano.tabelas)?;
        Ok(Some(Self {
            corpo: format!("{f:?}"),
            invocacoes: plano.tabelas.invocacoes.clone(),
            pousos: plano.tabelas.pousos.clone(),
            saidas: plano.tabelas.saidas.clone(),
            confere_pilha: plano.tabelas.confere_pilha,
            owners: plano.owners_no_pouso.clone(),
            quadros,
        }))
    }

    pub(crate) fn conferir(&self, f: &Function, t: &TabelasDaFuncao) -> Result<(), String> {
        if self.corpo != format!("{f:?}")
            || self.invocacoes != t.invocacoes
            || self.pousos != t.pousos
            || self.saidas != t.saidas
            || self.confere_pilha != t.confere_pilha
        {
            return Err(format!("cleanup estrangeiro obsoleto em {}", f.symbol));
        }
        Ok(())
    }

    pub(crate) fn owners(&self, b: BlockId) -> &[ValueId] {
        self.owners
            .get(&b)
            .expect("pouso sem inventário certificado")
    }
    pub(crate) fn quadros(&self, b: BlockId) -> &[ValueId] {
        self.quadros
            .get(&b)
            .expect("pouso sem inventário de quadros certificado")
    }
}
