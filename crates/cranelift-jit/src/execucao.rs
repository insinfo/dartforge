//! Único módulo com `unsafe`: obter o ponteiro da função compilada e chamá-lo.
//!
//! Não há como executar código gerado em tempo de execução sem sair do que o
//! compilador Rust consegue verificar. As invariantes que tornam cada bloco
//! correto estão documentadas junto dele e sustentadas pelo tipo
//! [`ProgramaCompilado`], que mantém o `JITModule` vivo enquanto o ponteiro
//! existir e o libera uma única vez, no `Drop`.
use crate::medicoes::Medicoes;
use crate::runtime;
use cranelift_jit::JITModule;

/// Programa da fatia escalar já compilado em memória e pronto para executar.
///
/// O tipo é deliberadamente `!Send` e `!Sync` (contém um ponteiro cru): a
/// captura de saída do runtime é por thread e o código gerado só pode ser
/// chamado pela thread que o compilou.
pub struct ProgramaCompilado {
    /// `Option` apenas para permitir mover o módulo no `Drop`; nunca é `None`
    /// enquanto o valor estiver vivo para o chamador.
    modulo: Option<JITModule>,
    entrada: *const u8,
    medicoes: Medicoes,
}

impl ProgramaCompilado {
    /// Constrói o programa a partir do módulo finalizado e do ponteiro de entrada.
    ///
    /// # Panics
    /// Entra em pânico se `entrada` for nulo, o que indicaria que o módulo não
    /// foi finalizado antes da consulta.
    pub(crate) fn novo(modulo: JITModule, entrada: *const u8, medicoes: Medicoes) -> Self {
        assert!(!entrada.is_null(), "entrada do JIT não finalizada");
        Self {
            modulo: Some(modulo),
            entrada,
            medicoes,
        }
    }

    /// Tempos e contadores da compilação em memória deste programa.
    #[must_use]
    pub fn medicoes(&self) -> &Medicoes {
        &self.medicoes
    }

    /// Executa `main`, enviando o que for impresso para o stdout do processo.
    ///
    /// # Panics
    /// Propaga qualquer pânico do runtime Rust chamado pelo código gerado.
    pub fn executar(&self) {
        // SAFETY: `entrada` veio de `JITModule::get_finalized_function` para o
        // símbolo `dartforge_entry`, que o tradutor declara e define com
        // assinatura `void()` na convenção de chamada nativa do ISA do host. O
        // `JITModule` que possui essa memória continua vivo em `self.modulo`
        // durante toda a chamada, porque `&self` empresta o programa inteiro e
        // a liberação só acontece no `Drop`. Portanto o ponteiro aponta para
        // código executável válido com exatamente este tipo.
        #[allow(unsafe_code)]
        let entrada: extern "C" fn() = unsafe { std::mem::transmute(self.entrada) };
        entrada();
    }

    /// Executa `main` capturando o que for impresso, na thread corrente.
    ///
    /// É o caminho usado pelos testes: a saída fica idêntica, byte a byte, à do
    /// executável AOT, porque as funções de impressão são as mesmas.
    ///
    /// # Panics
    /// Propaga qualquer pânico do runtime Rust chamado pelo código gerado.
    #[must_use]
    pub fn executar_capturando(&self) -> String {
        let anterior = runtime::iniciar_captura();
        self.executar();
        runtime::encerrar_captura(anterior)
    }
}

impl Drop for ProgramaCompilado {
    /// Libera as páginas de código; nenhum ponteiro para elas sobrevive a este ponto.
    fn drop(&mut self) {
        if let Some(modulo) = self.modulo.take() {
            // SAFETY: `free_memory` exige que nenhuma função do módulo esteja
            // executando e que nenhum ponteiro obtido dele seja chamado depois.
            // O único ponteiro entregue é `self.entrada`, que nunca escapa deste
            // tipo: `executar` o consome dentro de um `&self` e retorna antes de
            // qualquer `Drop`. Como o programa não é `Send`, também não há outra
            // thread executando código deste módulo.
            #[allow(unsafe_code)]
            unsafe {
                modulo.free_memory();
            }
        }
    }
}

impl std::fmt::Debug for ProgramaCompilado {
    /// Mostra apenas as medições; o ponteiro de código não é informação útil.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProgramaCompilado")
            .field("medicoes", &self.medicoes)
            .finish_non_exhaustive()
    }
}
