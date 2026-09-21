//! A convenção de chamada que o tradutor precisa conhecer, e só ela.
//!
//! Um montador não tem alocador de registradores nem gerador de prólogo: quem
//! decide onde cada argumento chega, quanto de pilha reservar e quais
//! registradores podem ser destruídos é o backend. Este módulo isola essas três
//! decisões para que o tradutor não repita `cfg!(windows)` a cada chamada.
//!
//! A fatia usa apenas `RAX` (acumulador e valor de retorno), `RCX` (rascunho do
//! operando direito) e `RBP` (base do quadro). Nenhum registrador salvo pelo
//! chamado é tocado além de `RBP`, que o prólogo empilha e o epílogo restaura,
//! então não há lista de *callee-saved* para manter.

/// Quantidade de parâmetros que cabem em registrador nas duas ABIs cobertas.
///
/// Win64 usa `RCX, RDX, R8, R9`; a System V AMD64 usa `RDI, RSI, RDX, RCX` e
/// aceitaria seis. A fatia para em quatro para que o mesmo limite valha nos
/// dois alvos: passar o quinto argumento é justamente onde as duas ABIs mais
/// divergem, e uma divergência não testada num backend é pior do que uma
/// recusa explícita.
pub(crate) const MAXIMO_DE_PARAMETROS: usize = 4;

/// Códigos dos registradores de argumento do alvo, na ordem da ABI.
///
/// Os números são os da tabela de registradores dinâmicos do dynasm-rs, que
/// coincidem com a codificação do x86-64: `RCX` é 1, `RDX` é 2, `R8` é 8 e
/// `R9` é 9.
#[cfg(windows)]
pub(crate) const ARGUMENTOS: [u8; MAXIMO_DE_PARAMETROS] = [1, 2, 8, 9];

/// Códigos dos registradores de argumento do alvo, na ordem da ABI.
///
/// `RDI` é 7, `RSI` é 6, `RDX` é 2 e `RCX` é 1.
#[cfg(not(windows))]
pub(crate) const ARGUMENTOS: [u8; MAXIMO_DE_PARAMETROS] = [7, 6, 2, 1];

/// Bytes que o chamador reserva no topo da pilha para o chamado usar.
///
/// A Win64 exige 32 bytes de *shadow space* imediatamente acima de `RSP` no
/// momento do `call`, mesmo quando o chamado não os usa. A System V não tem
/// equivalente. Reservar esse espaço uma única vez no prólogo — e não a cada
/// chamada — é o que permite manter `RSP` fixo durante todo o corpo, e é por
/// isso que nenhuma chamada precisa de ajuste próprio de pilha.
#[cfg(windows)]
pub(crate) const SOMBRA: i32 = 32;

/// Bytes que o chamador reserva no topo da pilha para o chamado usar.
#[cfg(not(windows))]
pub(crate) const SOMBRA: i32 = 0;

/// Tamanho do quadro, em bytes, para `slots` posições de 8 bytes.
///
/// O resultado é sempre múltiplo de 16 e nunca é zero. Na entrada de uma função
/// `RSP` está a 8 módulo 16 por causa do endereço de retorno; depois do
/// `push rbp` ele fica a 0, e subtrair um múltiplo de 16 preserva o alinhamento
/// que as duas ABIs exigem no ponto de chamada.
///
/// # Panics
/// Entra em pânico se o quadro não couber em `i32`, o que exigiria uma função
/// com mais de 250 milhões de posições vivas.
pub(crate) fn tamanho_do_quadro(slots: usize) -> i32 {
    let bytes = slots
        .checked_mul(8)
        .and_then(|b| i32::try_from(b).ok())
        .expect("quadro maior que i32 exigiria centenas de milhões de locais");
    let alinhado = (bytes + 15) & !15;
    alinhado + SOMBRA
}

#[cfg(test)]
mod testes {
    use super::{ARGUMENTOS, MAXIMO_DE_PARAMETROS, SOMBRA, tamanho_do_quadro};

    /// O quadro é sempre múltiplo de 16 e reserva a sombra exigida pela ABI.
    #[test]
    fn o_quadro_preserva_o_alinhamento_de_dezesseis() {
        for slots in 0..40 {
            let tamanho = tamanho_do_quadro(slots);
            assert_eq!(tamanho % 16, 0, "slots = {slots}");
            assert!(
                tamanho >= i32::try_from(slots).unwrap() * 8 + SOMBRA,
                "slots = {slots}"
            );
        }
    }

    /// Nenhum registrador de argumento coincide com `RAX`, `RBP` ou `RSP`.
    ///
    /// O tradutor carrega os argumentos um a um a partir da pilha; se algum
    /// deles fosse `RAX` (0), `RSP` (4) ou `RBP` (5) a sequência destruiria o
    /// acumulador ou o próprio quadro antes do `call`.
    #[test]
    fn os_registradores_de_argumento_nao_colidem_com_o_quadro() {
        assert_eq!(ARGUMENTOS.len(), MAXIMO_DE_PARAMETROS);
        for codigo in ARGUMENTOS {
            assert!(codigo != 0 && codigo != 4 && codigo != 5, "código {codigo}");
        }
    }
}
