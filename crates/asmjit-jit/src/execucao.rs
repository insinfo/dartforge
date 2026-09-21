//! Único módulo com `unsafe`: obter o ponteiro do código gerado e chamá-lo.
//!
//! Não há como executar código montado em tempo de execução sem sair do que o
//! compilador Rust consegue verificar. As invariantes que tornam cada bloco
//! correto estão documentadas junto dele e sustentadas pelo tipo
//! [`ProgramaCompilado`], que mantém o bloco executável vivo enquanto o
//! ponteiro existir.
//!
//! O bloco vem de `dynasmrt::ExecutableBuffer`, que segundo a documentação do
//! `dynasmrt::Assembler` garante que **nenhuma memória é gravável e executável
//! ao mesmo tempo**: a montagem acontece num buffer comum e só a confirmação
//! (`commit`) copia para páginas marcadas como executáveis. Isso é relevante
//! para o eixo 4 de `docs/ASMJIT.md`, e está registrado lá.
use crate::medicoes::Medicoes;
use crate::runtime;
use dynasmrt::{AssemblyOffset, ExecutableBuffer};
use std::marker::PhantomData;

/// Programa da fatia escalar já montado em memória e pronto para executar.
///
/// O tipo é deliberadamente `!Send` e `!Sync`: a captura de saída do runtime é
/// por thread, e o código gerado só pode ser chamado pela thread que o montou.
pub struct ProgramaCompilado {
    /// Páginas executáveis com todas as funções do programa, liberadas no `Drop`.
    bloco: ExecutableBuffer,
    /// Início do corpo de `main` dentro do bloco.
    entrada: AssemblyOffset,
    medicoes: Medicoes,
    /// Impede `Send`/`Sync` sem recorrer a um campo de ponteiro cru vivo.
    _mesma_thread: PhantomData<*const ()>,
}

impl ProgramaCompilado {
    /// Constrói o programa a partir do bloco confirmado e do deslocamento da entrada.
    ///
    /// # Panics
    /// Entra em pânico se a entrada cair fora do bloco, o que indicaria que a
    /// tradução e a confirmação discordam sobre o tamanho do código.
    pub(crate) fn novo(
        bloco: ExecutableBuffer,
        entrada: AssemblyOffset,
        medicoes: Medicoes,
    ) -> Self {
        assert!(
            entrada.0 < bloco.size(),
            "entrada fora do bloco executável: {} >= {}",
            entrada.0,
            bloco.size()
        );
        Self {
            bloco,
            entrada,
            medicoes,
            _mesma_thread: PhantomData,
        }
    }

    /// Tempos e contadores da montagem em memória deste programa.
    #[must_use]
    pub fn medicoes(&self) -> &Medicoes {
        &self.medicoes
    }

    /// Bytes de memória executável mapeados para este programa.
    ///
    /// É o tamanho do mapeamento, arredondado para páginas inteiras pelo
    /// alocador do `dynasmrt`, e por isso sempre maior ou igual aos bytes de
    /// código realmente emitidos, que estão em
    /// [`crate::Medicoes::bytes_codigo`]. A diferença é o custo de memória por
    /// recarga, que interessa ao eixo 4 de `docs/ASMJIT.md`.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.bloco.size()
    }

    /// Executa `main`, enviando o que for impresso para o stdout do processo.
    ///
    /// # Panics
    /// Propaga qualquer pânico do runtime Rust chamado pelo código gerado.
    pub fn executar(&self) {
        let ponteiro = self.bloco.ptr(self.entrada);
        // SAFETY: `ponteiro` aponta para o prólogo do corpo de `main` dentro de
        // `self.bloco`, que é memória executável confirmada pelo `dynasmrt` e
        // que continua viva durante toda a chamada, porque `&self` empresta o
        // programa inteiro e a liberação só acontece no `Drop`. O tradutor
        // emitiu aquele corpo com zero parâmetros, sem valor de retorno e com o
        // prólogo/epílogo da convenção nativa do alvo, que é exatamente o tipo
        // `extern "C" fn()`. O construtor já garantiu que o deslocamento cai
        // dentro do bloco.
        #[allow(unsafe_code)]
        let entrada: extern "C" fn() = unsafe { std::mem::transmute(ponteiro) };
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

impl std::fmt::Debug for ProgramaCompilado {
    /// Mostra apenas as medições; o endereço do código não é informação útil.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProgramaCompilado")
            .field("medicoes", &self.medicoes)
            .field("bytes", &self.bloco.size())
            .finish_non_exhaustive()
    }
}
