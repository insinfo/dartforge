// Runtime nativo: `Finalizer` (dart:core) e `NativeFinalizer` (dart:ffi) —
// o `FinalizerEntry` da VM (`runtime/vm/object.h`, `gc_marker.cc`) sobre os
// anexos do coletor (`Heap::anexos`).
//
// * Um anexo guarda o valor e a chave de `detach` como referências fracas;
//   o dono (o finalizador) e a ação, fortes. A coleta que acha o valor morto
//   tira o anexo: a ação de um `Finalizer` (a closure `() => callback(token)`
//   montada no Dart) vai para a fila de finalizações prontas, que o laço de
//   eventos atende entre um evento e outro — na VM é uma mensagem ao
//   isolado; a de um `NativeFinalizer` (`callback(token)`, função C) roda na
//   própria coleta, como na VM, e não toca o heap.
// * `detach(chave)` tira os anexos do mesmo dono com a mesma chave.
// * No fim do isolado os `NativeFinalizer` ainda anexados rodam (a VM os
//   garante no encerramento); os `Finalizer`, não (a especificação não
//   promete que rodem).

/// `DartForge_finalizador_anexar(dono, valor, acao, desanexo)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_finalizador_anexar(dono: i64, valor: i64, acao: i64, desanexo: i64) {
    HEAP.with(|h| {
        h.borrow_mut().anexos.push(crate::heap::AnexoDeFinalizador {
            dono,
            valor,
            desanexo,
            acao: crate::heap::AcaoDeFinalizador::Dart(acao),
        })
    });
}

/// `DartForge_finalizador_anexar_nativo(dono, valor, funcao, token,
/// desanexo)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_finalizador_anexar_nativo(dono: i64, valor: i64, funcao: i64, token: i64, desanexo: i64) {
    if funcao == 0 {
        lancar_erro_de_argumento("NativeFinalizer callback must not be nullptr");
        return;
    }
    HEAP.with(|h| {
        h.borrow_mut().anexos.push(crate::heap::AnexoDeFinalizador {
            dono,
            valor,
            desanexo,
            acao: crate::heap::AcaoDeFinalizador::Nativa(funcao as usize, token as usize),
        })
    });
}

/// `DartForge_finalizador_desanexar(dono, desanexo)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_finalizador_desanexar(dono: i64, desanexo: i64) {
    if desanexo == 0 {
        return;
    }
    HEAP.with(|h| h.borrow_mut().anexos.retain(|a| !(a.dono == dono && a.desanexo == desanexo)));
}

/// A próxima finalização pronta (a closure), sem tirá-la da fila: ela
/// continua raiz até [`concluir_finalizacao`].
fn proxima_finalizacao() -> Option<i64> {
    HEAP.with(|h| h.borrow().finalizacoes_prontas.front().copied())
}

fn concluir_finalizacao() {
    HEAP.with(|h| h.borrow_mut().finalizacoes_prontas.pop_front());
}

/// Fim do isolado: os `NativeFinalizer` ainda anexados rodam.
pub fn encerrar_finalizadores_do_isolado() {
    HEAP.with(|h| h.borrow_mut().encerrar_finalizadores());
}
