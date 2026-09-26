// Runtime nativo: os isolados (`Isolate.spawn`, `Isolate.exit`, a porta de
// controle) — o que a VM faz em `runtime/vm/isolate.cc` e
// `runtime/lib/isolate.cc`.
//
// Um isolado é uma thread com o próprio heap, a própria área de globais e o
// próprio laço de eventos: todo o estado do runtime é por thread. O código
// gerado entrega na entrada a preparação de um isolado
// (`df.preparar_isolado`: os registros das bibliotecas, a RTI e o
// embedder) e a função que chama uma closure; `Isolate.spawn` copia a
// entrada e a mensagem para grafos (`portas.rs`), cria a thread, e nela
// prepara o isolado, manda a mensagem de pronto (`[porta de controle,
// [pausa, término]]`) e roda a entrada como a primeira mensagem
// (`_startIsolate`). Constantes canônicas e tear-offs de topo atravessam
// pela identidade (o destino recebe as dele), como no grupo de isolados da
// VM.
//
// A porta de controle recebe as mensagens OOB da VM (`pause`, `resume`,
// `ping`, `kill`, ouvintes de saída e de erro, erros fatais), que o laço de
// eventos atende entre um evento e outro, antes dos comuns. Ao terminar, o
// isolado avisa os ouvintes de saída e fecha as portas. Um erro não tratado
// vai aos ouvintes de erro (`[erro, rastro]`); sem ouvinte e com erros
// fatais, é impresso como na VM (`Unhandled exception:`) e o isolado
// termina — o processo, não: ele termina com o isolado principal.
//
// `Isolate.exit` e o `kill` desenrolam o isolado (o `UnwindError` da VM):
// a exceção pendente não é capturável (`excecoes.rs`). O `kill` é atendido
// entre eventos: um isolado preso num laço Dart sem eventos só o vê ao
// voltar ao laço de eventos (a VM o interrompe na verificação de pilha).

/// `IsolateMessageHandler` (`isolate_patch.dart`, `_PAUSE`…).
const OOB_PAUSA: i64 = 1;
const OOB_RETOMAR: i64 = 2;
const OOB_PING: i64 = 3;
const OOB_MATAR: i64 = 4;
const OOB_OUVIR_SAIDA: i64 = 5;
const OOB_DESOUVIR_SAIDA: i64 = 6;
const OOB_OUVIR_ERRO: i64 = 7;
const OOB_DESOUVIR_ERRO: i64 = 8;
const OOB_ERROS_FATAIS: i64 = 9;

/// A pilha de cada thread de isolado (reserva; o sistema só usa o que
/// tocar).
const PILHA_DO_ISOLADO: usize = 64 * 1024 * 1024;

/// O que o código gerado entrega na entrada: a preparação de um isolado e
/// a chamada de closure (0 = ainda não registrado).
static PREPARAR_ISOLADO: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static CHAMAR_CLOSURE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// `dartforge_registrar_isolados(preparar, chamar)`: a entrada do programa
/// entrega a preparação de isolado e a chamada de closure.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_registrar_isolados(preparar: extern "C" fn(), chamar: Option<extern "C" fn(i64) -> i64>) {
    PREPARAR_ISOLADO.store(preparar as usize, std::sync::atomic::Ordering::Release);
    CHAMAR_CLOSURE.store(chamar.map_or(0, |c| c as usize), std::sync::atomic::Ordering::Release);
}

/// O estado de controle de um isolado.
struct EstadoDoIsolado {
    controle: i64,
    pausa: i64,
    termino: i64,
    erros_fatais: bool,
    /// As capacidades de retomada das pausas pedidas (pausado se houver).
    pausas: Vec<i64>,
    /// (porta, resposta) de cada ouvinte de saída.
    ouvintes_de_saida: Vec<(i64, Grafo)>,
    ouvintes_de_erro: Vec<i64>,
    /// Um `kill` foi aceito: o isolado termina no próximo retorno ao laço.
    encerrar: bool,
}

thread_local! {
    static ISOLADO: RefCell<Option<EstadoDoIsolado>> = const { RefCell::new(None) };
}

/// Porta de controle → nome de depuração (`Isolate.debugName`), de todos os
/// isolados vivos.
fn nomes_dos_isolados() -> std::sync::MutexGuard<'static, std::collections::HashMap<i64, String>> {
    static N: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<i64, String>>> = std::sync::OnceLock::new();
    N.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}

/// O estado deste isolado, criado na primeira vez (o principal nasce
/// assim: erros fatais, sem pausa, sem ouvintes).
fn com_estado<R>(f: impl FnOnce(&mut EstadoDoIsolado) -> R) -> R {
    ISOLADO.with(|i| {
        let mut i = i.borrow_mut();
        let e = i.get_or_insert_with(|| {
            let controle = abrir_porta_de_controle();
            nomes_dos_isolados().insert(controle, "main".to_string());
            EstadoDoIsolado {
                controle,
                pausa: dartforge_nativo_DartForge_capacidade_nova(),
                termino: dartforge_nativo_DartForge_capacidade_nova(),
                erros_fatais: true,
                pausas: Vec::new(),
                ouvintes_de_saida: Vec::new(),
                ouvintes_de_erro: Vec::new(),
                encerrar: false,
            }
        });
        f(e)
    })
}

/// Se o isolado está pausado (o laço só atende a porta de controle).
fn isolado_pausado() -> bool {
    ISOLADO.with(|i| i.borrow().as_ref().is_some_and(|e| !e.pausas.is_empty()))
}

/// O inteiro de um valor Dart (escalar ou `Ref` de `int`).
fn inteiro_de(v: TaggedValue) -> Option<i64> {
    if !v.is_ref {
        return (v.tag == ValueTag::Int).then_some(v.bits);
    }
    HEAP.with(|h| h.borrow().int_de_ref(v.bits))
}

/// O `_id` de uma `_SendPort` ou `_Capability` (o primeiro campo).
fn id_do_objeto(h: i64) -> Option<i64> {
    let campo = HEAP.with(|heap| match heap.borrow().try_get(h) {
        Some(Value::Object { fields, .. }) => fields.first().copied(),
        _ => None,
    })?;
    inteiro_de(if campo.1 { TaggedValue::reference(campo.0) } else { TaggedValue::scalar(campo.0) })
}

/// Chama a função `nome` da sobreposição de `dart:isolate`.
fn ajudante_de_isolado(nome: &str) -> usize {
    ajudante(nome).unwrap_or_else(|| panic!("bug do compilador: dart:isolate sem `{nome}` registrado"))
}

/// Copia `valor` para outro isolado; `None` se não é enviável (com a
/// descrição em `DartForge_porta_recusa`).
fn copiar_para_outro_isolado(valor: i64) -> Option<Grafo> {
    match copiar_para_grafo(valor, false) {
        Ok(g) => Some(g),
        Err(MensagemIlegal(d)) => {
            lancar_erro_de_argumento(&format!("Illegal argument in isolate message: {d}"));
            None
        }
    }
}

// ---------------------------------------------------------------------------
// A porta de controle.

/// Atende as mensagens de controle pendentes. Devolve `false` se o isolado
/// deve terminar (`kill`).
fn atender_controle() -> bool {
    while let Some(g) = proxima_de_controle() {
        let msg = materializar(&g);
        com_raizes(&[msg], || tratar_mensagem_de_controle(msg));
    }
    !ISOLADO.with(|i| i.borrow().as_ref().is_some_and(|e| e.encerrar))
}

/// `IsolateMessageHandler::HandleLibMessage`: `[0, tipo, …]`.
fn tratar_mensagem_de_controle(msg: i64) {
    let itens: Vec<TaggedValue> = HEAP.with(|h| {
        let h = h.borrow();
        match h.try_get(msg) {
            Some(Value::List(v)) => v.clone(),
            _ => Vec::new(),
        }
    });
    let Some(tipo) = itens.get(1).and_then(|v| inteiro_de(*v)) else { return };
    let objeto = |i: usize| itens.get(i).filter(|v| v.is_ref).map(|v| v.bits).unwrap_or(0);
    let id = |i: usize| id_do_objeto(objeto(i));
    let valor = |i: usize| itens.get(i).map_or(0, |v| if v.is_ref { v.bits } else { HEAP.with(|h| h.borrow_mut().como_ref(*v)) });
    match tipo {
        OOB_PAUSA | OOB_RETOMAR => {
            let (Some(cap), Some(retomar)) = (id(2), id(3)) else { return };
            com_estado(|e| {
                if cap != e.pausa {
                    return;
                }
                if tipo == OOB_PAUSA {
                    e.pausas.push(retomar);
                } else if let Some(k) = e.pausas.iter().position(|&r| r == retomar) {
                    e.pausas.remove(k);
                }
            });
        }
        OOB_PING => {
            let Some(porta) = id(2) else { return };
            // As duas prioridades respondem aqui: o controle é atendido
            // entre eventos, antes do próximo.
            let resposta = valor(4);
            if let Ok(g) = copiar_para_grafo(resposta, false) {
                postar(porta, g);
            }
        }
        OOB_MATAR => {
            let Some(cap) = id(2) else { return };
            com_estado(|e| {
                if cap == e.termino {
                    e.encerrar = true;
                }
            });
        }
        OOB_OUVIR_SAIDA => {
            let Some(porta) = id(2) else { return };
            let Ok(resposta) = copiar_para_grafo(valor(3), false) else { return };
            com_estado(|e| {
                e.ouvintes_de_saida.retain(|(p, _)| *p != porta);
                e.ouvintes_de_saida.push((porta, resposta));
            });
        }
        OOB_DESOUVIR_SAIDA => {
            let Some(porta) = id(2) else { return };
            com_estado(|e| e.ouvintes_de_saida.retain(|(p, _)| *p != porta));
        }
        OOB_OUVIR_ERRO => {
            let Some(porta) = id(2) else { return };
            com_estado(|e| {
                if !e.ouvintes_de_erro.contains(&porta) {
                    e.ouvintes_de_erro.push(porta);
                }
            });
        }
        OOB_DESOUVIR_ERRO => {
            let Some(porta) = id(2) else { return };
            com_estado(|e| e.ouvintes_de_erro.retain(|&p| p != porta));
        }
        OOB_ERROS_FATAIS => {
            let Some(cap) = id(2) else { return };
            let fatal = HEAP.with(|h| {
                let h = h.borrow();
                itens.get(3).is_some_and(|v| if v.is_ref { matches!(h.try_get(v.bits), Some(Value::BoxedBool(true))) } else { v.bits != 0 })
            });
            com_estado(|e| {
                if cap == e.termino {
                    e.erros_fatais = fatal;
                }
            });
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Erros não tratados e o fim do isolado.

/// `ProcessUnhandledException`: a exceção pendente vai aos ouvintes de
/// erro (`[erro, rastro]`); sem ouvinte e com erros fatais, é impressa.
/// Devolve se o isolado deve terminar.
fn relatar_erro_nao_tratado() -> bool {
    let erro = dartforge_exception_peek_ref();
    let rastro = dartforge_stack_trace_get();
    dartforge_exception_clear();
    let (fatal, ouvintes) = com_estado(|e| (e.erros_fatais, e.ouvintes_de_erro.clone()));
    // SAFETY: registrada pela sobreposição de `dart:isolate` com a
    // assinatura (`Object?`, `Object?`) → `List<String>`.
    let descrever: extern "C" fn(i64, i64) -> i64 = unsafe { std::mem::transmute(ajudante_de_isolado("_dartforgeDescreverErro")) };
    let descricao = com_raizes(&[erro, rastro], || descrever(erro, rastro));
    if dartforge_exception_pending() != 0 {
        // O `toString()` do erro falhou: a descrição do runtime.
        dartforge_exception_clear();
        if ouvintes.is_empty() && fatal {
            eprintln!("Unhandled exception:\n{}", HEAP.with(|h| describe_handle(&h.borrow(), erro)));
        }
        return fatal;
    }
    if ouvintes.is_empty() {
        if fatal {
            let textos: Vec<String> = HEAP.with(|h| {
                let h = h.borrow();
                (0..h.list_len(descricao))
                    .map(|i| match h.try_get(h.list_get(descricao, i).bits) {
                        Some(Value::String(t)) => t.para_string(),
                        _ => String::new(),
                    })
                    .collect()
            });
            let rastro = textos.get(1).map(String::as_str).unwrap_or("");
            eprintln!("Unhandled exception:\n{}\n{}", textos.first().map(String::as_str).unwrap_or(""), rastro.trim_end());
        }
    } else if let Ok(g) = copiar_para_grafo(descricao, false) {
        for (k, porta) in ouvintes.iter().enumerate() {
            if k + 1 == ouvintes.len() {
                postar(*porta, g);
                break;
            }
            postar(*porta, g.clone());
        }
    }
    fatal
}

/// O fim de um isolado: os ouvintes de saída recebem a resposta deles e as
/// portas fecham.
fn encerrar_isolado() {
    let (controle, ouvintes) = com_estado(|e| (e.controle, std::mem::take(&mut e.ouvintes_de_saida)));
    for (porta, resposta) in ouvintes {
        postar(porta, resposta);
    }
    nomes_dos_isolados().remove(&controle);
    fechar_portas_do_isolado();
}

/// O laço de eventos de um isolado com o tratamento dos erros não
/// tratados: um erro não fatal não o termina.
fn rodar_laco_do_isolado(chamar: extern "C" fn(i64) -> i64) {
    loop {
        dartforge_laco_de_eventos(chamar);
        if desenrolando() || ISOLADO.with(|i| i.borrow().as_ref().is_some_and(|e| e.encerrar)) {
            return;
        }
        if dartforge_exception_pending() == 0 {
            return;
        }
        if relatar_erro_nao_tratado() {
            return;
        }
    }
}

// ---------------------------------------------------------------------------
// `Isolate.spawn`.

/// O que a thread nova recebe do isolado que a criou.
struct PedidoDeIsolado {
    pronto: i64,
    entrada: Grafo,
    mensagem: Grafo,
    pausado: bool,
    erros_fatais: bool,
    ouvinte_de_saida: Option<i64>,
    ouvinte_de_erro: Option<i64>,
    nome: String,
}

/// A vida de um isolado criado por `Isolate.spawn`, na thread dele.
fn rodar_isolado(p: PedidoDeIsolado) {
    let preparar = PREPARAR_ISOLADO.load(std::sync::atomic::Ordering::Acquire);
    let chamar = CHAMAR_CLOSURE.load(std::sync::atomic::Ordering::Acquire);
    // SAFETY: registrados por `dartforge_registrar_isolados` com essas
    // assinaturas, antes de qualquer `Isolate.spawn`.
    let preparar: extern "C" fn() = unsafe { std::mem::transmute(preparar) };
    let chamar: extern "C" fn(i64) -> i64 = unsafe { std::mem::transmute(chamar) };
    preparar();
    let (controle, pausa, termino) = com_estado(|e| {
        e.erros_fatais = p.erros_fatais;
        if p.pausado {
            e.pausas.push(e.pausa);
        }
        if let Some(porta) = p.ouvinte_de_saida {
            e.ouvintes_de_saida.push((porta, Grafo::escalar(0, ValueTag::Ref)));
        }
        e.ouvintes_de_erro.extend(p.ouvinte_de_erro);
        (e.controle, e.pausa, e.termino)
    });
    nomes_dos_isolados().insert(controle, p.nome.clone());
    if dartforge_exception_pending() != 0 {
        relatar_erro_nao_tratado();
        encerrar_isolado();
        return;
    }
    let entrada = materializar(&p.entrada);
    com_raizes(&[entrada], || {
        let mensagem = materializar(&p.mensagem);
        com_raizes(&[mensagem], || {
            // SAFETY: registradas pela sobreposição de `dart:isolate` com as
            // assinaturas (`int`, `int`, `int`) → `List` e (`Function`,
            // `Object?`) → `void`.
            let pronto: extern "C" fn(i64, i64, i64) -> i64 = unsafe { std::mem::transmute(ajudante_de_isolado("_dartforgeMensagemDePronto")) };
            let iniciar: extern "C" fn(i64, i64) = unsafe { std::mem::transmute(ajudante_de_isolado("_dartforgeIniciarIsolado")) };
            let m = pronto(controle, pausa, termino);
            if let Ok(g) = com_raizes(&[m], || copiar_para_grafo(m, false)) {
                postar(p.pronto, g);
            }
            iniciar(entrada, mensagem);
        });
    });
    if dartforge_exception_pending() == 0 || !relatar_erro_nao_tratado() {
        rodar_laco_do_isolado(chamar);
    }
    encerrar_isolado();
}

/// `Isolate_spawnFunction(readyPort, uri, entryPoint, message, paused,
/// errorsAreFatal, onExit, onError, packageConfig, debugName)`.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn dartforge_nativo_Isolate_spawnFunction(
    pronto: i64,
    uri: i64,
    entrada: i64,
    mensagem: i64,
    pausado: u8,
    erros_fatais: u8,
    ouvinte_de_saida: i64,
    ouvinte_de_erro: i64,
    _configuracao_de_pacotes: i64,
    nome: i64,
) {
    let Some(pronto) = id_do_objeto(pronto) else {
        lancar_erro_de_argumento("readyPort is not a SendPort");
        return;
    };
    if PREPARAR_ISOLADO.load(std::sync::atomic::Ordering::Acquire) == 0 || CHAMAR_CLOSURE.load(std::sync::atomic::Ordering::Acquire) == 0 {
        lancar_erro_interno("Isolate.spawn sem a entrada do programa registrada");
        return;
    }
    let Some(g_entrada) = copiar_para_outro_isolado(entrada) else { return };
    let Some(g_mensagem) = copiar_para_outro_isolado(mensagem) else { return };
    let texto = |h: i64| (h != 0).then(|| HEAP.with(|heap| heap.borrow().texto(h).para_string()));
    let nome = texto(nome).unwrap_or_else(|| format!("{}:spawn()", texto(uri).unwrap_or_default()));
    let pedido = PedidoDeIsolado {
        pronto,
        entrada: g_entrada,
        mensagem: g_mensagem,
        pausado: pausado != 0,
        erros_fatais: erros_fatais != 0,
        ouvinte_de_saida: (ouvinte_de_saida != 0).then(|| id_do_objeto(ouvinte_de_saida)).flatten(),
        ouvinte_de_erro: (ouvinte_de_erro != 0).then(|| id_do_objeto(ouvinte_de_erro)).flatten(),
        nome: nome.clone(),
    };
    let criada = std::thread::Builder::new().name(nome).stack_size(PILHA_DO_ISOLADO).spawn(move || rodar_isolado(pedido));
    if let Err(e) = criada {
        // A VM responde na porta de pronto com o texto do erro.
        postar(pronto, Portavel::Str(format!("Unable to create the isolate thread: {e}")).para_grafo());
    }
}

/// `Isolate_spawnUri`: o executável nativo não carrega outro programa (como
/// o runtime AOT da VM com um `.dart`); a porta de pronto recebe o erro.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn dartforge_nativo_Isolate_spawnUri(
    pronto: i64,
    uri: i64,
    _args: i64,
    _mensagem: i64,
    _pausado: u8,
    _saida: i64,
    _erro: i64,
    _fatais: u8,
    _asserts: i64,
    _ambiente: i64,
    _pacotes: i64,
    _nome: i64,
) {
    let Some(pronto) = id_do_objeto(pronto) else { return };
    let uri = HEAP.with(|h| h.borrow().texto(uri).para_string());
    postar(
        pronto,
        Portavel::Str(format!("Isolate.spawnUri({uri}) is not supported in a program compiled ahead of time")).para_grafo(),
    );
}

/// `Isolate_getPortAndCapabilitiesOfCurrentIsolate`: `[porta de controle,
/// pausa, término]`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Isolate_getPortAndCapabilitiesOfCurrentIsolate() -> i64 {
    let (c, p, t) = com_estado(|e| (e.controle, e.pausa, e.termino));
    // SAFETY: registrada pela sobreposição de `dart:isolate` com a
    // assinatura (`int`, `int`, `int`) → `List`.
    let f: extern "C" fn(i64, i64, i64) -> i64 = unsafe { std::mem::transmute(ajudante_de_isolado("_dartforgePortaECapacidades")) };
    f(c, p, t)
}

/// `Isolate_getDebugName(controlPort)`: o nome do isolado vivo, ou `null`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Isolate_getDebugName(porta: i64) -> i64 {
    let Some(id) = id_do_objeto(porta) else { return 0 };
    let nome = nomes_dos_isolados().get(&id).cloned();
    nome.map_or(0, |n| alocar_str(&n))
}

/// `Isolate_getCurrentRootUriStr`: o URI `file:` do executável (o script,
/// para o embedder da VM).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Isolate_getCurrentRootUriStr() -> i64 {
    let caminho = std::env::current_exe().ok().or_else(|| std::env::args_os().next().map(std::path::PathBuf::from)).unwrap_or_default();
    let absoluto = if caminho.is_absolute() { caminho } else { std::env::current_dir().unwrap_or_default().join(caminho) };
    alocar_str(&uri_de_arquivo(&absoluto))
}

/// O URI `file:` de um caminho absoluto (`Uri.file`), com os bytes fora dos
/// não reservados codificados.
fn uri_de_arquivo(caminho: &std::path::Path) -> String {
    let texto = caminho.to_string_lossy().replace('\\', "/");
    let mut uri = String::from(if texto.starts_with('/') { "file://" } else { "file:///" });
    for b in texto.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~/:".contains(&b) {
            uri.push(char::from(b));
        } else {
            uri.push_str(&format!("%{b:02X}"));
        }
    }
    uri
}

/// `Isolate_sendOOB(port, msg)`: a mensagem vai à fila de controle do
/// isolado dono da porta.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Isolate_sendOOB(porta: i64, mensagem: i64) {
    let Some(id) = id_do_objeto(porta) else {
        lancar_erro_de_argumento("port is not a SendPort");
        return;
    };
    let Some(g) = copiar_para_outro_isolado(mensagem) else { return };
    postar_controle(id, g);
}

/// `Isolate_exit_(finalMessagePort, message)`: manda a mensagem final e
/// desenrola o isolado (o `UnwindError` da VM).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Isolate_exit_(porta: i64, mensagem: i64) -> i64 {
    if porta != 0 {
        let Some(id) = id_do_objeto(porta) else {
            lancar_erro_de_argumento("finalMessagePort is not a SendPort");
            return 0;
        };
        let Some(g) = copiar_para_outro_isolado(mensagem) else { return 0 };
        postar(id, g);
    }
    comecar_desenrolar();
    0
}
