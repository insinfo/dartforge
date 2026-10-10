//! Certificado do inventário usado pelo cleanup estrangeiro dos pousos.
use super::*;
use crate::hir::*;
use std::collections::{HashMap, HashSet};

/// Inventário de cleanup ligado ao corpo e às tabelas que foram verificados.
///
/// Criado pela preparação ARC do conjunto de funções; campos privados evitam
/// inventar owners no emissor. Mudanças posteriores exigem nova preparação.
/// Ainda não cobre quadros proprietários abertos no caminho excepcional.
///
/// ```
/// use dartforge_emit_native::otimizar::arc::PlanoFuncaoDart;
/// let plano = PlanoFuncaoDart::default();
/// assert!(plano.tabelas.cleanup_estrangeiro.is_none());
/// ```
#[derive(Debug, Clone)]
pub struct CleanupEstrangeiro {
    corpo: String,
    invocacoes: HashMap<ValueId, BlockId>,
    pousos: HashSet<BlockId>,
    saidas: HashMap<BlockId, SaidaPorExcecao>,
    confere_pilha: bool,
    owners: HashMap<BlockId, Vec<ValueId>>,
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
        if f.blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)|
            matches!(i, Instruction::CallRuntime { name, .. } if name == "dartforge_arc_quadro_abrir_v1")) {
            return Err(format!("cleanup estrangeiro em {} exige fechamento de quadros proprietários", f.symbol));
        }
        Ok(Some(Self {
            corpo: format!("{f:?}"),
            invocacoes: plano.tabelas.invocacoes.clone(),
            pousos: plano.tabelas.pousos.clone(),
            saidas: plano.tabelas.saidas.clone(),
            confere_pilha: plano.tabelas.confere_pilha,
            owners: plano.owners_no_pouso.clone(),
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
}
