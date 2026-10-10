// ABI de tokens e slots proprietários usados pelo código gerado.
// A pilha observacional não substitui estes owners. Operações de ownership
// não coletam nem executam Dart; coleta tem um safepoint explícito separado.
// A fábrica owned de caixas é uma operação de alocação, com safepoint próprio.
// O lançamento Ref publica uma raiz runtime e pode alocar o rastro.

/// Aloca uma instância zerada e entrega exatamente um token Owned ao chamador.
///
/// Classe deve caber em i32 e quantidade de campos em u16, como o cabeçalho
/// físico. Todos os campos começam com bits zero e sem marca de referência,
/// inclusive no mapa estendido. A alocação pode coletar; a retenção ocorre
/// no mesmo empréstimo do heap, antes de devolver o handle ou reabastecer TLAB.
/// Não executa construtor Dart, registra métodos ou certifica layout/recarga.
/// O chamador deve transferir o token ou liberá-lo exatamente uma vez.
///
/// # Panics
/// Classe/quantidade inválida ou falha interna de alocação/contagem; aborta
/// no limite C. Não representa uma exceção Dart.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let objeto = dartforge_arc_objeto_owned_v1(123, 40);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observar_heap_v1(objeto), 3);
/// assert_eq!(dartforge_arc_ler_campo_escalar_v1(objeto, 39), 0);
/// dartforge_arc_release(objeto);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observar_heap_v1(objeto), 0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_objeto_owned_v1(classe: i64, campos: i64) -> i64 {
    let (classe, campos) = parametros_instancia_owned(classe, campos)
        .expect("contrato de alocação de instância ARC violado");
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let objeto = heap.alocar_instancia(classe, campos);
        heap.reter_owner_codigo(objeto);
        heap.reabastecer_tlab(crate::layout::palavras_de_instancia(campos));
        objeto
    })
}

fn parametros_instancia_owned(classe: i64, campos: i64) -> Result<(i32, usize), &'static str> {
    let classe = i32::try_from(classe).map_err(|_| "classe fora de i32")?;
    let campos = u16::try_from(campos).map_err(|_| "quantidade de campos fora de u16")?;
    Ok((classe, usize::from(campos)))
}

/// Registra a tabela estática de métodos e aloca uma instância zerada Owned.
///
/// A função `tabela` é o thunk estático do emissor: devolve uma tabela válida
/// `{cid, n, [hash, entrada]…}`, sem executar Dart, coletar ou lançar. A memória
/// e as entradas precisam permanecer válidas segundo o protocolo de gerações.
/// O registro ocorre antes da alocação; o token tem o mesmo contrato de
/// [`dartforge_arc_objeto_owned_v1`]. Não executa o construtor Dart.
///
/// # Safety
/// `tabela` deve cumprir o protocolo acima: ponteiro legível, quantidade e
/// entradas válidas, memória durável e ausência de Dart/GC/exceções. A função
/// de registro lê a memória devolvida; um ponteiro inválido não é verificável.
///
/// # Panics
/// Classe/quantidade inválida ou falha interna; aborta no limite C. Tabela
/// inválida viola o protocolo nativo, como em `dartforge_object_new_t`.
///
/// ```
/// use dartforge_runtime::abi::*;
/// extern "C" fn tabela() -> *const i64 {
///     static TABELA: [i64; 2] = [32000, 0];
///     TABELA.as_ptr()
/// }
/// // SAFETY: thunk puro, tabela estática válida e sem entradas.
/// let objeto = unsafe { dartforge_arc_objeto_owned_t_v1(32000, 0, tabela) };
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observar_heap_v1(objeto), 3);
/// dartforge_arc_release(objeto);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observar_heap_v1(objeto), 0);
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arc_objeto_owned_t_v1(classe: i64, campos: i64, tabela: extern "C" fn() -> *const i64) -> i64 {
    parametros_instancia_owned(classe, campos).expect("contrato de alocação de instância ARC violado");
    dartforge_registrar_tabela(classe, tabela);
    dartforge_arc_objeto_owned_v1(classe, campos)
}

/// Grava bits escalares num campo de uma instância emprestada.
///
/// Exige receiver vivo e mutável, índice válido e layout escalar certificado
/// pelo lowering. Não interpreta os bits como handle: remove a marca Ref e
/// a aresta forte anterior, mesmo quando os bits novos parecem um endereço.
/// Aplica a barreira do heap. Não aloca, coleta ou executa Dart; invalida
/// empréstimos porque a substituição pode retirar a última aresta do filho.
/// Não faz guardas Dart de tipo/late nem certifica versão ou pin de layout.
///
/// # Panics
/// Violação das precondições ou falha interna de contagem; aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let objeto = dartforge_arc_objeto_owned_v1(123, 40);
/// dartforge_arc_gravar_campo_escalar_v1(objeto, 39, i64::MAX);
/// assert_eq!(dartforge_arc_ler_campo_escalar_v1(objeto, 39), i64::MAX);
/// dartforge_arc_release(objeto);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_gravar_campo_escalar_v1(objeto: i64, indice: i64, bits: i64) {
    HEAP.with(|heap| gravar_campo_auditado(&mut heap.borrow_mut(), objeto, indice, bits, false)
        .expect("contrato de gravação escalar ARC violado"));
}

/// Publica uma referência emprestada como aresta forte de uma instância.
///
/// Não consome token do receiver nem do valor. Exige receiver vivo e mutável,
/// índice válido, valor Ref vivo (ou null/Smi/permanente) e layout Ref
/// certificado. Atualiza bits, mapa e barreira, retendo a nova aresta antes
/// de soltar a anterior. Autoatribuição não muda a multiplicidade.
/// Não aloca, coleta ou executa Dart. Invalida empréstimos pela substituição
/// da aresta; guardas Dart de tipo/late e versão/pins são obrigações do lowering.
///
/// # Panics
/// Violação das precondições ou falha interna de contagem; aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let objeto = dartforge_arc_objeto_owned_v1(123, 40);
/// let filho = dartforge_arc_box_int_owned_v1(i64::MAX);
/// dartforge_arc_gravar_campo_ref_v1(objeto, 39, filho);
/// dartforge_arc_release(filho);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observar_heap_v1(filho), 1);
/// dartforge_arc_gravar_campo_ref_v1(objeto, 39, 0);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observar_heap_v1(filho), 0);
/// dartforge_arc_release(objeto);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_gravar_campo_ref_v1(objeto: i64, indice: i64, valor: i64) {
    HEAP.with(|heap| gravar_campo_auditado(&mut heap.borrow_mut(), objeto, indice, valor, true)
        .expect("contrato de gravação Ref ARC violado"));
}

fn gravar_campo_auditado(heap: &mut Heap, objeto: i64, indice: i64, bits: i64, referencia: bool) -> Result<(), &'static str> {
    let vista = heap.objeto(objeto).ok_or("receiver não é objeto vivo")?;
    let indice = usize::try_from(indice).map_err(|_| "índice negativo")?;
    if indice >= vista.len() { return Err("índice fora do objeto"); }
    heap.definir_campo(objeto, indice, bits, referencia);
    Ok(())
}

/// Copia os bits de um campo escalar de um objeto emprestado.
///
/// Exige objeto vivo, índice válido e campo não marcado como referência no
/// mapa do heap. O lowering deve provar o layout correspondente antes de
/// escolher esta ABI. Não aloca, coleta, retém o objeto, invalida empréstimos
/// nem converte os bits. O catálogo pode declarar resultado escalar porque
/// a implementação confere o mapa, não porque a representação é i64.
///
/// # Panics
/// Violação das precondições de layout/objeto/índice; aborta no limite C.
/// Não substitui as verificações Dart de receiver, tipo ou inicialização late.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let objeto = dartforge_object_new(123, 1);
/// dartforge_arc_retain(objeto);
/// dartforge_object_set(objeto, 0, i64::MAX, 0);
/// assert_eq!(dartforge_arc_ler_campo_escalar_v1(objeto, 0), i64::MAX);
/// dartforge_arc_release(objeto);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_ler_campo_escalar_v1(objeto: i64, indice: i64) -> i64 {
    HEAP.with(|heap| ler_campo_escalar_auditado(&heap.borrow(), objeto, indice)
        .expect("contrato de leitura escalar ARC violado"))
}

fn ler_campo_escalar_auditado(heap: &Heap, objeto: i64, indice: i64) -> Result<i64, &'static str> {
    let objeto = heap.objeto(objeto).ok_or("receiver não é objeto vivo")?;
    let indice = usize::try_from(indice).map_err(|_| "índice negativo")?;
    let (bits, referencia) = objeto.get(indice).ok_or("índice fora do objeto")?;
    if referencia { return Err("campo é referência gerenciada"); }
    Ok(bits)
}

/// Empresta uma referência de campo, sem criar token Owned para o resultado.
///
/// Exige instância viva, índice válido e marca Ref no mapa real do heap.
/// Null/Smi são referências válidas. O resultado depende da aresta do receiver:
/// manter só o receiver vivo não protege o filho após substituir o campo.
/// O lowering deve provar layout e limites de empréstimo; antes de uma
/// invalidação, deve copiar o empréstimo para Owned ou encerrar seu uso.
/// Não aloca, coleta, retém, invalida outros borrows ou executa Dart.
/// Não substitui guardas Dart de tipo/late nem certifica versões/pins.
///
/// # Panics
/// Receiver/índice/mapa incompatível; falha interna aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let objeto = dartforge_arc_objeto_owned_v1(123, 40);
/// let filho = dartforge_arc_box_int_owned_v1(i64::MAX);
/// dartforge_arc_gravar_campo_ref_v1(objeto, 39, filho);
/// dartforge_arc_release(filho);
/// let emprestado = dartforge_arc_ler_campo_ref_v1(objeto, 39);
/// assert_eq!(emprestado, filho);
/// dartforge_arc_retain(emprestado); // Copia o empréstimo antes da substituição.
/// dartforge_arc_gravar_campo_ref_v1(objeto, 39, 0);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observar_heap_v1(emprestado), 3);
/// dartforge_arc_release(emprestado);
/// dartforge_arc_release(objeto);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_ler_campo_ref_v1(objeto: i64, indice: i64) -> i64 {
    HEAP.with(|heap| ler_campo_ref_auditado(&heap.borrow(), objeto, indice)
        .expect("contrato de leitura Ref ARC violado"))
}

fn ler_campo_ref_auditado(heap: &Heap, objeto: i64, indice: i64) -> Result<i64, &'static str> {
    let objeto = heap.objeto(objeto).ok_or("receiver não é objeto vivo")?;
    let indice = usize::try_from(indice).map_err(|_| "índice negativo")?;
    let (bits, referencia) = objeto.get(indice).ok_or("índice fora do objeto")?;
    if !referencia { return Err("campo é escalar"); }
    Ok(bits)
}

/// Lança um valor Ref emprestado e o mantém como raiz da exceção pendente.
///
/// Não consome o token do chamador. O protocolo de exceções mantém uma raiz
/// independente até clear/consumo; a preparação do rastro pode alocar/coletar.
/// O lowering deve fornecer um Ref vivo e conferir a pendência imediatamente.
/// Não converte bits escalares nem aplica a semântica Dart de `throw null`.
///
/// # Panics
/// Falha interna do protocolo de exceções/heap; aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::{dartforge_arc_box_int_owned_v1, dartforge_arc_lancar_ref_v1,
///     dartforge_arc_release, dartforge_exception_pending, dartforge_exception_clear};
/// let valor = dartforge_arc_box_int_owned_v1(i64::MAX);
/// dartforge_arc_lancar_ref_v1(valor);
/// assert_ne!(dartforge_exception_pending(), 0);
/// dartforge_arc_release(valor);
/// dartforge_exception_clear();
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_lancar_ref_v1(valor: i64) {
    dartforge_exception_throw(valor, 3);
}

/// Copia a exceção pendente para um token Owned independente do clear.
///
/// Preserva a identidade de valores Ref. Escalares são encaixotados dentro
/// do mesmo empréstimo do heap que publica o owner, antes de devolver o handle.
/// Sem pendência devolve null; release de null não altera contadores.
/// Não consome a pendência nem modifica seu rastro. Pode alocar/coletar.
///
/// # Panics
/// Falha interna de alocação/ownership; aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let original = dartforge_arc_box_int_owned_v1(i64::MAX);
/// dartforge_arc_lancar_ref_v1(original);
/// let capturada = dartforge_arc_excecao_owned_v1();
/// assert_eq!(capturada, original);
/// dartforge_arc_release(original);
/// dartforge_exception_clear();
/// dartforge_arc_collect();
/// dartforge_arc_release(capturada);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_excecao_owned_v1() -> i64 {
    let Some(valor) = EXCEPTION.with(|slot| *slot.borrow()) else { return 0; };
    HEAP.with(|h| {
        let mut heap = h.borrow_mut();
        let referencia = heap.como_ref(valor);
        heap.reter_owner_codigo(referencia);
        referencia
    })
}

/// Copia o rastro corrente para um token Owned independente do clear.
///
/// Sem rastro corrente cria um StackTrace não vazio, sem instalá-lo como
/// rastro de uma exceção futura. Pode alocar/coletar; publica o owner antes
/// de devolver o handle, sem safepoint entre a leitura e a retenção.
///
/// # Panics
/// Falha interna de alocação/ownership; aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let rastro = dartforge_arc_rastro_owned_v1();
/// dartforge_arc_collect();
/// dartforge_arc_release(rastro);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_rastro_owned_v1() -> i64 {
    let rastro = dartforge_stack_trace_get();
    HEAP.with(|h| h.borrow_mut().reter_owner_codigo(rastro));
    rastro
}

/// Lança um Ref emprestado com um StackTrace explícito emprestado.
///
/// Não consome os dois tokens do chamador. Publica raízes independentes
/// para valor e rastro até clear/consumo. Exige ambos vivos e um StackTrace
/// válido; não aplica a semântica Dart de `throw null`.
/// O lowering deve conferir a pendência imediatamente após a chamada.
///
/// # Panics
/// Falha interna do protocolo de exceções/heap; aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let valor = dartforge_arc_box_int_owned_v1(i64::MAX);
/// let rastro = dartforge_arc_rastro_owned_v1();
/// dartforge_arc_lancar_com_rastro_ref_v1(valor, rastro);
/// let copia = dartforge_arc_rastro_owned_v1();
/// assert_eq!(copia, rastro);
/// dartforge_arc_release(valor);
/// dartforge_arc_release(rastro);
/// dartforge_exception_clear();
/// dartforge_arc_collect();
/// dartforge_arc_release(copia);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_lancar_com_rastro_ref_v1(valor: i64, rastro: i64) {
    dartforge_throw_with_stack_trace(valor, 3, rastro);
}

/// Entrega um int em representação Ref com um token Owned para o chamador.
///
/// Smi não tem contador; valores fora de sua faixa criam um Mint mortal.
/// A alocação pode coletar, mas o token é publicado antes de devolver o handle.
/// O chamador deve transferi-lo ou chamar release exatamente uma vez.
///
/// # Panics
/// Falha interna de alocação/contagem; aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::{dartforge_arc_box_int_owned_v1, dartforge_arc_release, dartforge_arc_collect};
/// let valor = dartforge_arc_box_int_owned_v1(i64::MAX);
/// dartforge_arc_release(valor);
/// dartforge_arc_collect();
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_box_int_owned_v1(valor: i64) -> i64 {
    HEAP.with(|h| {
        let mut heap = h.borrow_mut();
        let caixa = heap.caixa_int(valor);
        heap.reter_owner_codigo(caixa);
        caixa
    })
}

/// Encaixota um double e entrega um token Owned antes de outra coleta.
///
/// Preserva todos os bits, inclusive zero negativo e payload de NaN. A caixa
/// pertence ao chamador até release ou transferência para slot/retorno.
///
/// # Panics
/// Empréstimo reentrante do heap ou falha de alocação; aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let valor = dartforge_arc_box_double_owned_v1(-0.0);
/// assert_eq!(dartforge_arc_observar_heap_v1(valor), 3);
/// dartforge_arc_release(valor);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observar_heap_v1(valor), 0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_box_double_owned_v1(valor: f64) -> i64 {
    HEAP.with(|h| {
        let mut heap = h.borrow_mut();
        let caixa = heap.caixa_double(valor);
        heap.reter_owner_codigo(caixa);
        caixa
    })
}

/// Observa bloco vivo e owner do código sem adquirir referência ao objeto.
///
/// O argumento é um endereço nativo de diagnóstico, não um Ref emprestado:
/// pode designar um handle já liberado. Bit 0 indica bloco vivo deste heap;
/// bit 1 indica ao menos um token do código. Null, Smi, objetos estáticos e
/// endereços fora das páginas registradas não contam como blocos do heap.
/// Não coleta nem executa Dart. O snapshot não garante validade futura;
/// reutilização do endereço pode fazer uma observação posterior ver outro objeto.
///
/// # Panics
/// Empréstimo reentrante do heap; falha interna aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let endereco = dartforge_arc_box_int_owned_v1(i64::MAX);
/// assert_eq!(dartforge_arc_observar_heap_v1(endereco), 3);
/// dartforge_arc_release(endereco);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observar_heap_v1(endereco), 0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_observar_heap_v1(endereco: i64) -> u8 {
    HEAP.with(|h| h.borrow().observar_owner_codigo(endereco))
}

/// Cria um token proprietário do código gerado a partir de um empréstimo.
///
/// Null e Smi não têm contador. Não coleta nem executa Dart. O token fica
/// no inventário do heap até release; slots proprietários são independentes.
///
/// # Panics
/// Handle morto ou overflow; falha interna aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::{dartforge_arc_retain, dartforge_arc_release};
/// dartforge_arc_retain(0);
/// dartforge_arc_release(0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_retain(valor: i64) {
    HEAP.with(|h| h.borrow_mut().reter_owner_codigo(valor));
}

/// Consome um token do código gerado, sem coletar ou executar Dart.
///
/// Não consome owners de quadros, campos ou mensagens. Null/Smi são no-op.
/// Zero de RC apenas agenda trabalho para um safepoint posterior.
///
/// # Panics
/// Handle sem token correspondente; falha interna aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::{dartforge_arc_retain, dartforge_arc_release};
/// dartforge_arc_retain(0);
/// dartforge_arc_release(0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_release(valor: i64) {
    HEAP.with(|h| h.borrow_mut().soltar_owner_codigo(valor));
}

/// Executa coleta explícita num safepoint com as raízes já publicadas.
///
/// Pode executar callbacks nativos de finalização; callbacks Dart são
/// enfileirados, sem execução nesta chamada. Em tracing faz coleta completa.
///
/// # Panics
/// Falha interna de coleta; aborta no limite C.
///
/// ```
/// dartforge_runtime::abi::dartforge_arc_collect();
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_collect() {
    HEAP.with(|h| h.borrow_mut().collect());
}

/// Confere a versão de tokens desta ABI e o modo ARC do heap corrente.
///
/// Retorna 1 somente para versão 1 com ARC ativo, 0 para incompatibilidade.
/// O chamador deve recusar a execução ARC quando o resultado for zero.
/// Não certifica contratos de um módulo nem a ABI de retornos das externs.
///
/// ```
/// assert_eq!(dartforge_runtime::abi::dartforge_arc_verificar_abi(-1), 0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_verificar_abi(versao: i64) -> i8 {
    i8::from(versao == 1 && HEAP.with(|h| h.borrow().arc_ativo()))
}

/// Abre um quadro proprietário com slots inicialmente nulos.
///
/// # Panics
/// Quantidade negativa/incompatível com usize ou falha interna; o limite C
/// aborta, sem desenrolar para o código gerado.
///
/// ```
/// use dartforge_runtime::abi::{dartforge_arc_quadro_abrir_v1, dartforge_arc_quadro_fechar_v1};
/// let quadro = dartforge_arc_quadro_abrir_v1(0);
/// dartforge_arc_quadro_fechar_v1(quadro);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_quadro_abrir_v1(slots: i64) -> i64 {
    let slots = usize::try_from(slots).expect("quantidade inválida de slots ARC");
    HEAP.with(|h| h.borrow_mut().push_frame_proprietario(slots))
}

/// Copia o valor para um slot, retendo antes de soltar o conteúdo antigo.
///
/// O quadro e índice são escalares; valor é Ref gerenciado, inclusive null/Smi.
/// Zero consome o conteúdo anterior. Não atende dívida de coleta.
///
/// # Panics
/// Quadro/slot inválido ou referência morta; falha interna aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let quadro = dartforge_arc_quadro_abrir_v1(1);
/// dartforge_arc_quadro_copiar_v1(quadro, 0, 0);
/// dartforge_arc_quadro_fechar_v1(quadro);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_quadro_copiar_v1(quadro: i64, slot: i64, valor: i64) {
    let slot = usize::try_from(slot).expect("índice inválido de slot ARC");
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        h.conferir_quadro_proprietario(quadro);
        h.set_root(quadro, slot, valor);
    });
}

/// Carrega um slot proprietário e cria um token owned do código.
///
/// O slot mantém sua ocorrência. O resultado precisa de release ou transferência,
/// mesmo se o quadro for fechado antes. Não coleta nem executa Dart.
///
/// # Panics
/// Quadro observacional/inexistente, slot inválido ou falha interna; aborta no C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let quadro = dartforge_arc_quadro_abrir_v1(1);
/// let valor = dartforge_arc_quadro_carregar_v1(quadro, 0);
/// dartforge_arc_quadro_fechar_v1(quadro);
/// dartforge_arc_release(valor);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_quadro_carregar_v1(quadro: i64, slot: i64) -> i64 {
    let slot = usize::try_from(slot).expect("índice inválido de slot ARC");
    HEAP.with(|h| h.borrow_mut().copiar_raiz_para_codigo(quadro, slot))
}

/// Transfere um token do código para um slot proprietário.
///
/// Não retém a origem. Publica o destino antes de soltar seu owner anterior.
/// Null/Smi não exigem token físico. Não coleta nem executa Dart.
///
/// # Panics
/// Quadro/slot inválido ou handle sem token do código; valida antes da mutação.
/// Falha interna aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let quadro = dartforge_arc_quadro_abrir_v1(1);
/// dartforge_arc_quadro_receber_v1(quadro, 0, 0);
/// dartforge_arc_quadro_fechar_v1(quadro);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_quadro_receber_v1(quadro: i64, slot: i64, valor: i64) {
    let slot = usize::try_from(slot).expect("índice inválido de slot ARC");
    HEAP.with(|h| h.borrow_mut().mover_codigo_para_raiz(quadro, slot, valor));
}

/// Transfere token do código ao registro de owner de um global.
///
/// `id` identifica o armazenamento, não é desreferenciado. O código gerado
/// deve publicar o valor real antes, sem safepoint até esta chamada. O registro
/// guarda apenas owners de handles; não fornece o valor de uma carga global.
/// Não retém a origem, coleta ou executa Dart. Null/Smi retiram owner anterior.
///
/// # Panics
/// Handle morto ou sem token do código; falha interna aborta no limite C.
///
/// ```
/// dartforge_runtime::abi::dartforge_arc_global_receber_v1(17, 0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_global_receber_v1(id: i64, valor: i64) {
    HEAP.with(|h| h.borrow_mut().mover_codigo_para_global(id, valor));
}

/// Move um owner entre slots, consumindo origem e conteúdo antigo do destino.
///
/// Mover para o mesmo slot preserva a ocorrência. Não há coleta nem Dart
/// entre publicação do destino e limpeza da origem.
///
/// # Panics
/// Quadros não proprietários, inexistentes ou índices inválidos; aborta no C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let quadro = dartforge_arc_quadro_abrir_v1(2);
/// dartforge_arc_quadro_mover_v1(quadro, 0, quadro, 1);
/// dartforge_arc_quadro_fechar_v1(quadro);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_quadro_mover_v1(
    origem: i64,
    slot_origem: i64,
    destino: i64,
    slot_destino: i64,
) {
    let de = usize::try_from(slot_origem).expect("índice inválido de origem ARC");
    let para = usize::try_from(slot_destino).expect("índice inválido de destino ARC");
    HEAP.with(|h| h.borrow_mut().mover_raiz(origem, de, destino, para));
}

/// Consome os owners do quadro do topo sem coletar ou executar Dart.
///
/// # Panics
/// Quadro não está no topo; falha interna aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let quadro = dartforge_arc_quadro_abrir_v1(1);
/// dartforge_arc_quadro_fechar_v1(quadro);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_quadro_fechar_v1(quadro: i64) {
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        h.conferir_quadro_proprietario(quadro);
        h.pop_frame(quadro);
    });
}

#[cfg(test)]
mod testes_arc_abi_quadros {
    use super::*;

    #[test]
    fn excecao_ref_preserva_mint_apos_release_e_clear_remove_raiz() {
        for arc in [false, true] {
            let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().ativar_arc()); }
            let valor = dartforge_arc_box_int_owned_v1(i64::MAX);
            dartforge_arc_lancar_ref_v1(valor);
            assert_ne!(dartforge_exception_pending(), 0);
            dartforge_arc_release(valor);
            dartforge_arc_collect();
            HEAP.with(|h| assert!(h.borrow().e_objeto_vivo(valor)));
            assert_eq!(dartforge_exception_peek_ref(), valor);
            dartforge_exception_clear();
            assert_eq!(dartforge_exception_pending(), 0);
            dartforge_arc_collect();
            HEAP.with(|h| assert!(!h.borrow().e_objeto_vivo(valor)));
            HEAP.with(|h| { h.replace(anterior); });
        }
    }

    #[test]
    fn captura_owned_preserva_identidade_e_sobrevive_clear_nos_dois_modos() {
        for arc in [false, true] {
            let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().ativar_arc()); }
            dartforge_exception_clear();
            assert_eq!(dartforge_arc_excecao_owned_v1(), 0);
            let original = dartforge_arc_box_int_owned_v1(i64::MAX);
            dartforge_arc_lancar_ref_v1(original);
            let capturada = dartforge_arc_excecao_owned_v1();
            assert_eq!(capturada, original);
            assert_ne!(dartforge_exception_pending(), 0);
            dartforge_arc_release(original);
            dartforge_exception_clear();
            dartforge_arc_collect();
            assert!(HEAP.with(|h| h.borrow().e_objeto_vivo(capturada)));
            assert_eq!(HEAP.with(|h| h.borrow().int_de(capturada)), Some(i64::MAX));
            dartforge_arc_release(capturada);
            dartforge_arc_collect();
            assert!(!HEAP.with(|h| h.borrow().e_objeto_vivo(capturada)));
            HEAP.with(|h| { h.replace(anterior); });
        }
    }

    #[test]
    fn rastro_owned_preserva_identidade_e_morre_apos_ultimo_release() {
        for arc in [false, true] {
            let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().ativar_arc()); }
            dartforge_exception_clear();
            let original = dartforge_arc_box_int_owned_v1(i64::MAX);
            let rastro = dartforge_arc_rastro_owned_v1();
            assert!(CURRENT_STACK_TRACE.with(|slot| slot.borrow().is_none()));
            dartforge_arc_lancar_com_rastro_ref_v1(original, rastro);
            let capturado = dartforge_arc_rastro_owned_v1();
            let valor = dartforge_arc_excecao_owned_v1();
            assert_eq!(capturado, rastro);
            assert_eq!(valor, original);
            assert_eq!(CURRENT_STACK_TRACE.with(|slot| *slot.borrow()), Some(rastro));
            dartforge_arc_release(original);
            dartforge_arc_release(rastro);
            dartforge_exception_clear();
            dartforge_arc_collect();
            assert!(HEAP.with(|h| h.borrow().e_objeto_vivo(capturado)));
            assert!(!texto_do_rastro(capturado).unwrap().is_empty());
            assert_eq!(HEAP.with(|h| h.borrow().int_de(valor)), Some(i64::MAX));
            dartforge_arc_release(capturado);
            dartforge_arc_release(valor);
            dartforge_arc_collect();
            assert!(!HEAP.with(|h| h.borrow().e_objeto_vivo(capturado)));
            assert!(!HEAP.with(|h| h.borrow().e_objeto_vivo(valor)));
            HEAP.with(|h| { h.replace(anterior); });
        }
    }

    #[test]
    fn captura_owned_encaixota_escalar_sem_consumir_pendencia() {
        use crate::heap::Valor;
        for arc in [false, true] {
            let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().ativar_arc()); }
            for (bits, tag, esperado) in [
                (i64::MAX, 1, Valor::Int(i64::MAX)),
                (42, 1, Valor::Int(42)),
                (3.5_f64.to_bits() as i64, 4, Valor::Double(3.5)),
                (1, 2, Valor::Bool(true)),
                (0, 2, Valor::Bool(false)),
                (0, 3, Valor::Ref(0)),
            ] {
                dartforge_exception_throw(bits, tag);
                let capturada = dartforge_arc_excecao_owned_v1();
                assert_eq!(EXCEPTION.with(|slot| *slot.borrow()), Some(esperado));
                dartforge_exception_clear();
                dartforge_arc_collect();
                assert_eq!(HEAP.with(|h| h.borrow().valor(capturada)), esperado);
                dartforge_arc_release(capturada);
                dartforge_arc_collect();
                if matches!(esperado, Valor::Int(i64::MAX) | Valor::Double(_)) {
                    assert!(!HEAP.with(|h| h.borrow().e_objeto_vivo(capturada)));
                }
            }
            HEAP.with(|h| { h.replace(anterior); });
        }
    }

    #[test]
    fn observacao_nativa_nao_cria_owner_e_rejeita_enderecos_fora_do_heap() {
        for validar in [false, true] {
            for arc in [false, true] {
                let anterior = HEAP.with(|h| h.replace(Heap::new(validar)));
                if arc { HEAP.with(|h| h.borrow_mut().ativar_arc()); }
                for endereco in [0, 2, i64::MIN, i64::MAX, smi::de(42).unwrap()] {
                    assert_eq!(dartforge_arc_observar_heap_v1(endereco), 0);
                }
                let endereco = dartforge_arc_box_int_owned_v1(i64::MAX);
                assert_eq!(dartforge_arc_observar_heap_v1(endereco), 3);
                dartforge_arc_retain(endereco);
                dartforge_arc_release(endereco);
                assert_eq!(dartforge_arc_observar_heap_v1(endereco), 3);
                dartforge_arc_release(endereco);
                assert_eq!(dartforge_arc_observar_heap_v1(endereco), 1);
                dartforge_arc_collect();
                assert_eq!(dartforge_arc_observar_heap_v1(endereco), 0);
                HEAP.with(|h| { h.replace(anterior); });
            }
        }
    }

    #[test]
    fn fabrica_owned_entrega_token_e_mint_morre_apos_ultimo_release() {
        for arc in [false, true] {
            let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().ativar_arc()); }
            let valor = dartforge_arc_box_int_owned_v1(i64::MAX);
            assert!(HEAP.with(|h| h.borrow().e_objeto_vivo(valor)));
            assert_eq!(HEAP.with(|h| h.borrow().int_de(valor)), Some(i64::MAX));
            // A cópia representa um retorno retido, independente do token inicial.
            dartforge_arc_retain(valor);
            dartforge_arc_release(valor);
            dartforge_arc_collect();
            assert!(HEAP.with(|h| h.borrow().e_objeto_vivo(valor)));
            dartforge_arc_release(valor);
            dartforge_arc_collect();
            assert!(!HEAP.with(|h| h.borrow().e_objeto_vivo(valor)));
            let imediato = dartforge_arc_box_int_owned_v1(42);
            assert_eq!(imediato, smi::de(42).unwrap());
            dartforge_arc_release(imediato);
            HEAP.with(|h| { h.replace(anterior); });
        }
    }

    #[test]
    fn abi_global_recebe_objeto_mortal_e_libera_ultimo_owner() {
        for arc in [false, true] {
            let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc {
                HEAP.with(|h| h.borrow_mut().ativar_arc());
            }
            // O endereço representa armazenamento do chamador, sem uma raiz
            // observacional que possa esconder a perda do owner proprietário.
            let mut global = 0_i64;
            let id = (&mut global as *mut i64) as i64;
            let quadro = dartforge_arc_quadro_abrir_v1(1);
            let valor = HEAP.with(|h| h.borrow_mut().alocar_str("global mortal"));
            dartforge_arc_quadro_copiar_v1(quadro, 0, valor);
            global = dartforge_arc_quadro_carregar_v1(quadro, 0);
            dartforge_arc_global_receber_v1(id, global);
            dartforge_arc_quadro_fechar_v1(quadro);
            dartforge_arc_collect();
            assert!(HEAP.with(|h| h.borrow().e_objeto_vivo(global)));

            // Reatribuir o mesmo objeto consome o token novo e o owner velho;
            // a identidade não pode pular a transferência ou vazar um retain.
            dartforge_arc_retain(global);
            dartforge_arc_global_receber_v1(id, global);
            dartforge_arc_collect();
            assert!(HEAP.with(|h| h.borrow().e_objeto_vivo(valor)));

            dartforge_arc_retain(global);
            global = smi::de(42).unwrap();
            dartforge_arc_global_receber_v1(id, global);
            dartforge_arc_collect();
            assert_eq!(global, smi::de(42).unwrap());
            assert!(HEAP.with(|h| h.borrow().e_objeto_vivo(valor)));
            dartforge_arc_release(valor);
            dartforge_arc_collect();
            assert!(!HEAP.with(|h| h.borrow().e_objeto_vivo(valor)));
            global = 0;
            dartforge_arc_global_receber_v1(id, global);
            HEAP.with(|h| {
                h.replace(anterior);
            });
        }
    }

    #[test]
    fn abi_carrega_owner_independente_e_recebe_token_no_slot() {
        for arc in [false, true] {
            let anterior = HEAP.with(|h| h.replace(Heap::new(false)));
            if arc {
                HEAP.with(|h| h.borrow_mut().ativar_arc());
            }
            let quadro = dartforge_arc_quadro_abrir_v1(1);
            let valor = HEAP.with(|h| h.borrow_mut().alocar_str("slot e SSA"));
            dartforge_arc_quadro_copiar_v1(quadro, 0, valor);
            let token = dartforge_arc_quadro_carregar_v1(quadro, 0);
            assert_eq!(token, valor);
            dartforge_arc_quadro_receber_v1(quadro, 0, token);
            let token = dartforge_arc_quadro_carregar_v1(quadro, 0);
            dartforge_arc_quadro_fechar_v1(quadro);
            dartforge_arc_collect();
            assert!(HEAP.with(|h| h.borrow().e_objeto_vivo(token)));
            dartforge_arc_release(token);
            dartforge_arc_collect();
            assert!(!HEAP.with(|h| h.borrow().e_objeto_vivo(token)));
            let quadro = dartforge_arc_quadro_abrir_v1(1);
            for escalar in [0, smi::de(42).unwrap()] {
                dartforge_arc_quadro_receber_v1(quadro, 0, escalar);
                assert_eq!(dartforge_arc_quadro_carregar_v1(quadro, 0), escalar);
                dartforge_arc_release(escalar);
            }
            dartforge_arc_quadro_fechar_v1(quadro);
            HEAP.with(|h| {
                h.replace(anterior);
            });
        }
    }

    #[test]
    fn abi_minima_confere_modo_e_sustenta_tokens_ate_release() {
        let anterior = HEAP.with(|h| h.replace(Heap::new(false)));
        assert_eq!(dartforge_arc_verificar_abi(1), 0);
        HEAP.with(|h| h.borrow_mut().ativar_arc());
        assert_eq!(dartforge_arc_verificar_abi(1), 1);
        assert_eq!(dartforge_arc_verificar_abi(2), 0);
        let valor = HEAP.with(|h| h.borrow_mut().alocar_str("token ABI"));
        dartforge_arc_retain(valor);
        dartforge_arc_retain(valor);
        dartforge_arc_collect();
        assert!(HEAP.with(|h| h.borrow().e_objeto_vivo(valor)));
        dartforge_arc_release(valor);
        dartforge_arc_collect();
        assert!(HEAP.with(|h| h.borrow().e_objeto_vivo(valor)));
        dartforge_arc_release(valor);
        dartforge_arc_collect();
        assert!(!HEAP.with(|h| h.borrow().e_objeto_vivo(valor)));
        HEAP.with(|h| {
            h.replace(anterior);
        });
    }

    #[test]
    #[should_panic(expected = "quadro ARC deve ser proprietário")]
    fn abi_recusa_quadro_observacional_antes_de_alterar_slots() {
        let mut heap = Heap::new(false);
        let quadro = heap.push_frame_with_slots(1);
        heap.conferir_quadro_proprietario(quadro);
    }

    #[test]
    fn copia_movimento_e_fecho_preservam_occorrencias_ate_o_ultimo_owner() {
        for arc in [false, true] {
            let anterior = HEAP.with(|h| h.replace(Heap::new(false)));
            if arc {
                HEAP.with(|h| h.borrow_mut().ativar_arc());
            }
            let origem = dartforge_arc_quadro_abrir_v1(2);
            let destino = dartforge_arc_quadro_abrir_v1(1);
            let valor = HEAP.with(|h| h.borrow_mut().alocar_str("owner ABI"));
            dartforge_arc_quadro_copiar_v1(origem, 0, valor);
            dartforge_arc_quadro_copiar_v1(origem, 1, valor);
            dartforge_arc_quadro_mover_v1(origem, 0, destino, 0);
            dartforge_arc_quadro_mover_v1(destino, 0, destino, 0);
            dartforge_arc_quadro_copiar_v1(origem, 1, 0);
            HEAP.with(|h| {
                let mut h = h.borrow_mut();
                h.collect();
                assert!(h.e_objeto_vivo(valor));
            });
            dartforge_arc_quadro_fechar_v1(destino);
            HEAP.with(|h| {
                let mut h = h.borrow_mut();
                h.collect();
                assert!(!h.e_objeto_vivo(valor));
            });
            dartforge_arc_quadro_fechar_v1(origem);
            HEAP.with(|h| {
                h.replace(anterior);
            });
        }
    }
}

#[cfg(test)]
mod testes_caixa_double_owned {
    use super::*;
    #[test]
    fn double_owned_preserva_bits_e_morre_apos_ultimo_token() {
        for arc in [false, true] {
            let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().ativar_arc()); }
            for bits in [(-0.0_f64).to_bits(), 3.25_f64.to_bits(), f64::INFINITY.to_bits(), 0x7ff80000deadbeef] {
                let valor = dartforge_arc_box_double_owned_v1(f64::from_bits(bits));
                dartforge_arc_retain(valor);
                dartforge_arc_release(valor);
                dartforge_arc_collect();
                assert_eq!(HEAP.with(|h| h.borrow().double_de(valor).unwrap().to_bits()), bits);
                assert_eq!(dartforge_arc_observar_heap_v1(valor), 3);
                dartforge_arc_release(valor);
                dartforge_arc_collect();
                assert_eq!(dartforge_arc_observar_heap_v1(valor), 0);
            }
            HEAP.with(|h| { h.replace(anterior); });
        }
    }
}

#[cfg(test)]
mod testes_campo_ref {
    use super::*;

    #[test]
    fn leitura_empresta_aresta_e_copia_owned_sobrevive_a_substituicao() {
        for arc in [false, true] {
            let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().ativar_arc()); }
            let objeto = dartforge_arc_objeto_owned_v1(123, 40);
            let filho = dartforge_arc_box_int_owned_v1(i64::MAX);
            dartforge_arc_gravar_campo_ref_v1(objeto, 0, 0);
            dartforge_arc_gravar_campo_ref_v1(objeto, 1, smi::de(42).unwrap());
            dartforge_arc_gravar_campo_ref_v1(objeto, 39, filho);
            dartforge_arc_gravar_campo_escalar_v1(objeto, 37, filho);
            dartforge_arc_release(filho);
            dartforge_arc_collect();
            let stats = HEAP.with(|h| h.borrow().stats());
            assert_eq!(dartforge_arc_ler_campo_ref_v1(objeto, 0), 0);
            assert_eq!(dartforge_arc_ler_campo_ref_v1(objeto, 1), smi::de(42).unwrap());
            assert_eq!(dartforge_arc_ler_campo_ref_v1(objeto, 39), filho);
            assert_eq!(dartforge_arc_observar_heap_v1(filho), 1);
            HEAP.with(|h| {
                let h = h.borrow();
                for (receiver, indice) in [(objeto, 37), (objeto, 38), (objeto, -1), (objeto, 40), (0, 0), (filho, 0)] {
                    assert!(ler_campo_ref_auditado(&h, receiver, indice).is_err());
                }
                assert_eq!(h.stats().allocations, stats.allocations);
                assert_eq!(h.stats().collections, stats.collections);
            });
            dartforge_arc_gravar_campo_ref_v1(objeto, 39, 0);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(filho), 0);
            let filho = dartforge_arc_box_int_owned_v1(i64::MAX);
            dartforge_arc_gravar_campo_ref_v1(objeto, 39, filho);
            dartforge_arc_release(filho);
            let copia = dartforge_arc_ler_campo_ref_v1(objeto, 39);
            assert_eq!(copia, filho);
            dartforge_arc_retain(copia);
            dartforge_arc_gravar_campo_ref_v1(objeto, 39, 0);
            dartforge_arc_release(objeto);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(objeto), 0);
            assert_eq!(dartforge_arc_observar_heap_v1(copia), 3);
            dartforge_arc_release(copia);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(copia), 0);
            HEAP.with(|h| { h.replace(anterior); });
        }
    }
}

#[cfg(test)]
mod testes_gravacao_campo {
    use super::*;

    #[test]
    fn gravacao_distingue_bits_de_arestas_e_substitui_sem_consumir_tokens() {
        for arc in [false, true] {
            let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().ativar_arc()); }
            let objeto = dartforge_arc_objeto_owned_v1(123, 40);
            let filho = dartforge_arc_box_int_owned_v1(i64::MAX);
            let antes = HEAP.with(|h| h.borrow().stats());
            for indice in [0, 37, 38] {
                dartforge_arc_gravar_campo_ref_v1(objeto, indice, filho);
                dartforge_arc_gravar_campo_ref_v1(objeto, indice, filho);
            }
            for (indice, bits) in [(1, i64::MAX), (32, (-0.0_f64).to_bits() as i64), (39, 0x7ff80000deadbeef)] {
                dartforge_arc_gravar_campo_escalar_v1(objeto, indice, bits);
                assert_eq!(dartforge_arc_ler_campo_escalar_v1(objeto, indice), bits);
            }
            HEAP.with(|h| {
                let h = h.borrow();
                assert_eq!(h.stats().allocations, antes.allocations);
                assert_eq!(h.stats().collections, antes.collections);
                assert_eq!(h.observar_owner_codigo(objeto), 3);
                assert_eq!(h.observar_owner_codigo(filho), 3);
            });
            dartforge_arc_release(filho);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(filho), 1);
            // Os mesmos bits deixam de constituir aresta nos dois mapas.
            for indice in [0, 37] { dartforge_arc_gravar_campo_escalar_v1(objeto, indice, filho); }
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(filho), 1);
            dartforge_arc_gravar_campo_ref_v1(objeto, 38, 0);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(filho), 0);
            for indice in [0, 37] { assert_eq!(dartforge_arc_ler_campo_escalar_v1(objeto, indice), filho); }
            dartforge_arc_gravar_campo_ref_v1(objeto, 33, objeto);
            dartforge_arc_collect();
            dartforge_arc_gravar_campo_escalar_v1(objeto, 33, 0);
            dartforge_arc_release(objeto);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(objeto), 0);
            HEAP.with(|h| { h.replace(anterior); });
        }
    }

    #[test]
    fn receiver_e_indice_invalidos_nao_gravam_nem_coletam() {
        let mut heap = Heap::new(true);
        let objeto = heap.alocar_instancia(123, 40);
        heap.reter_owner_codigo(objeto);
        let caixa = heap.caixa_int(i64::MAX);
        heap.reter_owner_codigo(caixa);
        let antes = heap.objeto(objeto).unwrap().to_vec();
        let stats = heap.stats();
        for referencia in [false, true] {
            for (receiver, indice) in [(objeto, -1), (objeto, 40), (0, 0), (caixa, 0)] {
                assert!(gravar_campo_auditado(&mut heap, receiver, indice, 0, referencia).is_err());
                assert_eq!(heap.objeto(objeto).unwrap().to_vec(), antes);
                assert_eq!(heap.stats().allocations, stats.allocations);
                assert_eq!(heap.stats().collections, stats.collections);
            }
        }
    }
}

#[cfg(test)]
mod testes_instancia_owned {
    use super::*;

    #[test]
    fn instancia_owned_registra_metodos_antes_de_alocar_e_preserva_token() {
        use std::sync::{OnceLock, atomic::{AtomicUsize, Ordering}};
        static CHAMADAS: AtomicUsize = AtomicUsize::new(0);
        extern "C" fn entrada(_: i64, _: *const i64, _: *const i64) -> i64 { 85 }
        extern "C" fn tabela() -> *const i64 {
            static TABELA: OnceLock<[i64; 4]> = OnceLock::new();
            CHAMADAS.fetch_add(1, Ordering::Relaxed);
            TABELA.get_or_init(|| [32001, 1, 42, entrada as *const () as usize as i64]).as_ptr()
        }
        let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
        HEAP.with(|h| h.borrow_mut().ativar_arc());
        for _ in 0..2 {
            // SAFETY: tabela estática válida com uma entrada nativa durável.
            let objeto = unsafe { dartforge_arc_objeto_owned_t_v1(32001, 1, tabela) };
            assert_eq!(metodo_da_classe(32001, 42), Some(entrada as *const () as usize));
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(objeto), 3);
            assert_eq!(dartforge_arc_ler_campo_escalar_v1(objeto, 0), 0);
            dartforge_arc_release(objeto);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(objeto), 0);
        }
        assert_eq!(CHAMADAS.load(Ordering::Relaxed), 1);
        HEAP.with(|h| { h.replace(anterior); });
    }

    #[test]
    fn instancia_zerada_e_token_sobrevivem_a_stress_ate_uma_liberacao() {
        for arc in [false, true] {
            let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().ativar_arc()); }
            for campos in [0, 1, 32, 33, 40] {
                let objeto = dartforge_arc_objeto_owned_v1(123, campos);
                assert_eq!(dartforge_arc_observar_heap_v1(objeto), 3);
                // A próxima fábrica pode coletar antes de publicar seu token.
                let outro = dartforge_arc_objeto_owned_v1(124, 1);
                dartforge_arc_collect();
                HEAP.with(|h| {
                    let h = h.borrow();
                    let vista = h.objeto(objeto).unwrap();
                    assert_eq!(vista.class_id, 123);
                    assert_eq!(vista.len(), campos as usize);
                    assert!(vista.iter().all(|campo| campo == (0, false)));
                });
                let filho = if campos > 0 {
                    let filho = dartforge_arc_box_int_owned_v1(i64::MAX);
                    dartforge_object_set(objeto, campos - 1, filho, 1);
                    dartforge_arc_release(filho);
                    dartforge_arc_collect();
                    assert_eq!(dartforge_arc_observar_heap_v1(filho), 1);
                    assert_eq!(HEAP.with(|h| h.borrow().objeto(objeto).unwrap().campo(campos as usize - 1)), (filho, true));
                    Some(filho)
                } else { None };
                dartforge_arc_release(outro);
                dartforge_arc_release(objeto);
                assert_eq!(dartforge_arc_observar_heap_v1(objeto) & 2, 0);
                dartforge_arc_collect();
                assert_eq!(dartforge_arc_observar_heap_v1(objeto), 0);
                assert_eq!(dartforge_arc_observar_heap_v1(outro), 0);
                if let Some(filho) = filho { assert_eq!(dartforge_arc_observar_heap_v1(filho), 0); }
            }
            HEAP.with(|h| { h.replace(anterior); });
        }
    }

    #[test]
    fn parametros_devem_caber_no_cabecalho_sem_truncar() {
        for classe in [i32::MIN as i64, 0, i32::MAX as i64] {
            for campos in [0, 40, u16::MAX as i64] {
                assert_eq!(parametros_instancia_owned(classe, campos), Ok((classe as i32, campos as usize)));
            }
        }
        for classe in [i32::MIN as i64 - 1, i32::MAX as i64 + 1] {
            assert!(parametros_instancia_owned(classe, 1).is_err());
        }
        for campos in [-1, u16::MAX as i64 + 1, i64::MAX] {
            assert!(parametros_instancia_owned(123, campos).is_err());
        }
    }
}

#[cfg(test)]
mod testes_campo_escalar {
    use super::*;

    #[test]
    fn leitura_confere_mapa_sem_confundir_bits_com_handle_e_sem_coletar() {
        for arc in [false, true] {
            let anterior = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().ativar_arc()); }
            let objeto = HEAP.with(|h| {
                let mut h = h.borrow_mut();
                let filho = h.alocar_str("campo de referência");
                h.reter_owner_codigo(filho);
                let mut campos = vec![(0, false); 40];
                campos[0] = (filho, false);
                campos[1] = (filho, true);
                campos[2] = ((-0.0_f64).to_bits() as i64, false);
                campos[3] = (0x7ff80000deadbeef, false);
                campos[32] = (i64::MAX, false);
                campos[37] = (filho, true);
                let objeto = h.novo_objeto(123, &campos);
                h.reter_owner_codigo(objeto);
                h.collect();
                objeto
            });
            let antes = HEAP.with(|h| h.borrow().stats());
            for i in [0, 2, 3, 32, 39] {
                let esperado = HEAP.with(|h| h.borrow().objeto(objeto).unwrap().campo(i).0);
                assert_eq!(dartforge_arc_ler_campo_escalar_v1(objeto, i as i64), esperado);
            }
            HEAP.with(|h| {
                let h = h.borrow();
                for i in [1, 37] {
                    assert_eq!(ler_campo_escalar_auditado(&h, objeto, i), Err("campo é referência gerenciada"));
                }
                for i in [-1, 40] { assert!(ler_campo_escalar_auditado(&h, objeto, i).is_err()); }
                assert!(ler_campo_escalar_auditado(&h, 0, 0).is_err());
                assert_eq!(h.stats().allocations, antes.allocations);
                assert_eq!(h.stats().collections, antes.collections);
            });
            HEAP.with(|h| { h.replace(anterior); });
        }
    }
}
