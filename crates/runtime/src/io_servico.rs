// Runtime nativo: o IOService da VM (`runtime/bin/io_service.cc`) — a
// porta nativa que atende as operações assíncronas de arquivo e diretório
// do `dart:io` (`File.readAsString()`, `Directory.list()`…) numa reserva de
// threads, fora do isolado.
//
// O Dart do patch da VM (`io_service_patch.dart`) envia
// `[id, porta de resposta, pedido, dados]`; a thread que atende responde
// `[id, resultado]` na porta de resposta, que chega à fila do isolado como
// qualquer mensagem. Os pedidos são o `IO_SERVICE_REQUEST_LIST` e os
// resultados o que os `*Request` de `file.cc`/`directory.cc` devolvem: o
// valor, ou uma lista de erro `[kArgumentError]`, `[kFileClosedError]`,
// `[kOSError, código, mensagem]`.
//
// Como a porta concorrente da VM (`Dart_NewConcurrentNativePort`, até 32
// threads), os pedidos correm em paralelo; o Dart dos patches serializa o
// que precisa (um mesmo arquivo nunca tem dois pedidos em voo).

/// `CObject::kSuccess`, `kArgumentError`, `kOSError`, `kFileClosedError`.
const RESPOSTA_SUCESSO: i64 = 0;
const RESPOSTA_ARGUMENTO_INVALIDO: i64 = 1;
const RESPOSTA_ERRO_DO_SO: i64 = 2;
const RESPOSTA_ARQUIVO_FECHADO: i64 = 3;

/// O máximo de threads que atendem o IOService ao mesmo tempo.
const MAXIMO_DE_THREADS_DO_SERVICO: usize = 32;

/// Quantos itens um `ListNext` devolve de uma vez (o `kArraySize` da VM).
const LOTE_DE_LISTAGEM: usize = 128;

/// A reserva de threads do IOService: uma fila de pedidos e as threads que
/// a esvaziam, criadas sob demanda até o máximo.
struct ReservaDoServico {
    fila: std::sync::Mutex<std::collections::VecDeque<Grafo>>,
    sinal: std::sync::Condvar,
    threads: std::sync::atomic::AtomicUsize,
    ociosas: std::sync::atomic::AtomicUsize,
}

fn reserva_do_servico() -> &'static ReservaDoServico {
    static R: std::sync::OnceLock<ReservaDoServico> = std::sync::OnceLock::new();
    R.get_or_init(|| ReservaDoServico {
        fila: std::sync::Mutex::new(std::collections::VecDeque::new()),
        sinal: std::sync::Condvar::new(),
        threads: std::sync::atomic::AtomicUsize::new(0),
        ociosas: std::sync::atomic::AtomicUsize::new(0),
    })
}

impl ReservaDoServico {
    /// Enfileira um pedido e acorda (ou cria) uma thread para ele.
    fn enviar(&'static self, pedido: Grafo) {
        use std::sync::atomic::Ordering;
        self.fila.lock().unwrap_or_else(|e| e.into_inner()).push_back(pedido);
        if self.ociosas.load(Ordering::Acquire) == 0 && self.threads.load(Ordering::Acquire) < MAXIMO_DE_THREADS_DO_SERVICO {
            self.threads.fetch_add(1, Ordering::AcqRel);
            let criada = std::thread::Builder::new().name("dart:io IOService".to_string()).spawn(move || self.atender());
            if criada.is_err() {
                self.threads.fetch_sub(1, Ordering::AcqRel);
            }
        }
        self.sinal.notify_one();
    }

    /// O laço de uma thread: tira um pedido, atende, repete.
    fn atender(&self) {
        use std::sync::atomic::Ordering;
        loop {
            let pedido = {
                let mut fila = self.fila.lock().unwrap_or_else(|e| e.into_inner());
                loop {
                    if let Some(p) = fila.pop_front() {
                        break p;
                    }
                    self.ociosas.fetch_add(1, Ordering::AcqRel);
                    fila = self.sinal.wait(fila).unwrap_or_else(|e| e.into_inner());
                    self.ociosas.fetch_sub(1, Ordering::AcqRel);
                }
            };
            atender_pedido_de_io(pedido);
        }
    }
}

/// `IOServiceCallback`: lê `[id, porta, pedido, dados]`, atende e responde
/// `[id, resultado]`.
fn atender_pedido_de_io(g: Grafo) {
    let mensagem = g.para_portavel();
    let Some([id, porta, pedido, dados]) = mensagem.lista() else {
        return;
    };
    let Some(porta) = (match porta {
        Portavel::Objeto(campos) => campos.first().and_then(Portavel::int),
        _ => None,
    }) else {
        return;
    };
    let resultado = match (pedido.int(), dados.lista()) {
        (Some(p), Some(d)) => responder_pedido_de_io(p, d),
        _ => argumento_invalido(),
    };
    postar(porta, Portavel::Lista(vec![id.clone(), resultado]).para_grafo());
}

fn argumento_invalido() -> Portavel {
    Portavel::Lista(vec![Portavel::Int(RESPOSTA_ARGUMENTO_INVALIDO)])
}

fn arquivo_fechado() -> Portavel {
    Portavel::Lista(vec![Portavel::Int(RESPOSTA_ARQUIVO_FECHADO)])
}

/// O resultado Dart de uma operação: o valor por `f`, ou o erro do sistema.
fn resposta<T>(r: ResultadoIo<T>, f: impl FnOnce(T) -> Portavel) -> Portavel {
    match r {
        Ok(v) => f(v),
        Err(e) => e.para_resposta(),
    }
}

fn verdadeiro_ou_erro(r: ResultadoIo<()>) -> Portavel {
    resposta(r, |()| Portavel::Bool(true))
}

/// Um caminho em bytes (`CObjectUint8Array`, terminado em NUL).
fn caminho_do_pedido(p: &Portavel) -> Option<std::path::PathBuf> {
    let b = p.bytes()?;
    let fim = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    Some(caminho_de_bytes(&b[..fim]))
}

fn texto_do_pedido(p: &Portavel) -> Option<std::path::PathBuf> {
    Some(caminho_de_bytes(p.str()?.as_bytes()))
}

/// Atende o pedido `pedido` (o `IO_SERVICE_REQUEST_LIST` da VM) com `d`.
fn responder_pedido_de_io(pedido: i64, d: &[Portavel]) -> Portavel {
    // Os pedidos sobre um arquivo aberto: `d[0]` é o ponteiro, com a
    // referência que o `getPointer` reteve (solta ao fim do pedido).
    if matches!(pedido, 7..=11 | 17..=22 | 30) {
        let Some(p) = d.first().and_then(Portavel::int).filter(|&p| p != 0) else {
            return if pedido == 7 { Portavel::Int(-1) } else { argumento_invalido() };
        };
        // SAFETY: o Dart só envia ponteiros de `getPointer`, que reteve esta
        // referência para o pedido.
        let arquivo = unsafe { ArquivoNativo::tomar(p) };
        return responder_pedido_de_arquivo(pedido, &arquivo, &d[1..]);
    }
    match pedido {
        // Listagem assíncrona: `d[0]` é o ponteiro da listagem (retido).
        40 | 41 => {
            let Some(p) = d.first().and_then(Portavel::int).filter(|&p| p != 0) else {
                return argumento_invalido();
            };
            // SAFETY: ponteiro de `getPointer` com a referência do pedido.
            let listagem = unsafe { ListagemAssincrona::tomar(p) };
            if pedido == 40 {
                Portavel::Lista(listagem.lote(LOTE_DE_LISTAGEM))
            } else {
                listagem.listagem.lock().unwrap_or_else(|e| e.into_inner()).parar();
                Portavel::Bool(true)
            }
        }
        // Pedidos sem caminho.
        31 => resposta(criar_pipe(), |(r, w)| {
            Portavel::Lista(vec![Portavel::Int(ArquivoNativo::novo(r)), Portavel::Int(ArquivoNativo::novo(w))])
        }),
        // Resolução de nomes e interfaces de rede (`io_soquetes.rs`).
        32 => pedido_de_resolucao(d),
        33 => pedido_de_interfaces(d),
        34 => pedido_de_resolucao_reversa(d),
        _ => responder_pedido_de_caminho(pedido, d).unwrap_or_else(argumento_invalido),
    }
}

/// Os pedidos sobre um arquivo aberto.
fn responder_pedido_de_arquivo(pedido: i64, a: &ArquivoNativo, d: &[Portavel]) -> Portavel {
    if a.fechado() {
        return if pedido == 7 { Portavel::Int(-1) } else { arquivo_fechado() };
    }
    let int = |i: usize| d.get(i).and_then(Portavel::int);
    match pedido {
        7 => {
            a.fechar();
            Portavel::Int(0)
        }
        8 => resposta(a.posicao(), Portavel::Int),
        9 => match int(0) {
            Some(p) => verdadeiro_ou_erro(a.mudar_posicao(p)),
            None => argumento_invalido(),
        },
        10 => match int(0) {
            Some(n) => verdadeiro_ou_erro(a.truncar(n)),
            None => argumento_invalido(),
        },
        11 => resposta(a.tamanho(), Portavel::Int),
        17 => verdadeiro_ou_erro(a.sincronizar()),
        18 => {
            let mut b = [0u8; 1];
            resposta(a.ler(&mut b), |n| Portavel::Int(if n == 1 { i64::from(b[0]) } else { -1 }))
        }
        19 => match int(0) {
            Some(v) => resposta(a.escrever_tudo(&[v as u8]), |()| Portavel::Int(1)),
            None => argumento_invalido(),
        },
        20 | 21 => {
            let Some(n) = int(0).filter(|&n| n >= 0) else {
                return argumento_invalido();
            };
            let mut buf = vec![0u8; n as usize];
            resposta(a.ler(&mut buf), |lidos| {
                buf.truncate(lidos);
                if pedido == 20 {
                    Portavel::Lista(vec![Portavel::Int(RESPOSTA_SUCESSO), Portavel::Bytes(buf)])
                } else {
                    Portavel::Lista(vec![Portavel::Int(RESPOSTA_SUCESSO), Portavel::Int(lidos as i64), Portavel::Bytes(buf)])
                }
            })
        }
        22 => {
            let (Some(inicio), Some(fim)) = (int(1), int(2)) else {
                return argumento_invalido();
            };
            let bytes: Vec<u8> = match d.first() {
                Some(Portavel::Bytes(b)) => b.clone(),
                Some(Portavel::Lista(l)) => {
                    let mut b = Vec::with_capacity(l.len());
                    for x in l {
                        match x.int() {
                            Some(v) => b.push(v as u8),
                            None => return argumento_invalido(),
                        }
                    }
                    b
                }
                _ => return argumento_invalido(),
            };
            if inicio < 0 || fim < inicio || fim as usize > bytes.len() {
                return argumento_invalido();
            }
            resposta(a.escrever_tudo(&bytes[inicio as usize..fim as usize]), |()| Portavel::Int(fim - inicio))
        }
        30 => match (int(0), int(1), int(2)) {
            (Some(t), Some(i), Some(f)) => verdadeiro_ou_erro(a.travar(t, i, f)),
            _ => argumento_invalido(),
        },
        _ => argumento_invalido(),
    }
}

/// Os pedidos sobre caminhos: `d[0]` é o namespace (ignorado: só há o
/// padrão), `d[1]` o caminho. `None`: argumentos de forma errada.
fn responder_pedido_de_caminho(pedido: i64, d: &[Portavel]) -> Option<Portavel> {
    let caminho = || d.get(1).and_then(caminho_do_pedido);
    let texto = |i: usize| d.get(i).and_then(texto_do_pedido);
    let booleano = |i: usize| d.get(i).and_then(Portavel::bool);
    let int = |i: usize| d.get(i).and_then(Portavel::int);
    Some(match pedido {
        0 => Portavel::Bool(arquivo_existe(&caminho()?)),
        1 => verdadeiro_ou_erro(criar_arquivo(&caminho()?, booleano(2)?)),
        2 => verdadeiro_ou_erro(apagar_arquivo(&caminho()?)),
        3 => verdadeiro_ou_erro(renomear_arquivo(&caminho()?, &texto(2)?)),
        4 => verdadeiro_ou_erro(copiar_arquivo(&caminho()?, &texto(2)?)),
        5 => resposta(abrir_arquivo(&caminho()?, int(2)?), |fd| Portavel::Int(ArquivoNativo::novo(fd))),
        6 => resposta(caminho_canonico(&caminho()?), |b| Portavel::Str(String::from_utf8_lossy(&b).into_owned())),
        12 => resposta(metadados_de_arquivo(&caminho()?), |m| Portavel::Int(m.len() as i64)),
        13 => resposta(ultimo_acesso(&caminho()?), Portavel::Int),
        14 => resposta(mudar_instantes(&caminho()?, Some(int(2)?), None), |()| Portavel::Nulo),
        15 => resposta(ultima_modificacao(&caminho()?), Portavel::Int),
        16 => resposta(mudar_instantes(&caminho()?, None, Some(int(2)?)), |()| Portavel::Nulo),
        23 => verdadeiro_ou_erro(criar_link(&caminho()?, &texto(2)?)),
        24 => verdadeiro_ou_erro(apagar_link(&caminho()?)),
        25 => verdadeiro_ou_erro(renomear_link(&caminho()?, &texto(2)?)),
        26 => resposta(alvo_do_link(&caminho()?), |b| Portavel::Str(String::from_utf8_lossy(&b).into_owned())),
        27 => Portavel::Int(tipo_do_caminho(&caminho()?, booleano(2)?)),
        28 => resposta(identicos(&texto(1)?, &texto(2)?), Portavel::Bool),
        29 => resposta(estatisticas(&texto(1)?), |e| {
            Portavel::Lista(vec![Portavel::Int(RESPOSTA_SUCESSO), Portavel::Lista(e.iter().map(|&x| Portavel::Int(x)).collect())])
        }),
        35 => verdadeiro_ou_erro(criar_diretorio(&caminho()?)),
        36 => verdadeiro_ou_erro(apagar_diretorio(&caminho()?, booleano(2)?)),
        37 => resposta(diretorio_existe(&caminho()?), |e| Portavel::Int(if e { DIRETORIO_EXISTE } else { DIRETORIO_NAO_EXISTE })),
        38 => {
            let b = d.get(1)?.bytes()?;
            let fim = b.iter().position(|&c| c == 0).unwrap_or(b.len());
            resposta(criar_diretorio_temporario(&b[..fim]), |n| Portavel::Str(String::from_utf8_lossy(&n).into_owned()))
        }
        39 => {
            let b = d.get(1)?.bytes()?;
            let fim = b.iter().position(|&c| c == 0).unwrap_or(b.len());
            Portavel::Int(ListagemAssincrona::nova(Listagem::nova(b[..fim].to_vec(), booleano(2)?, booleano(3)?)))
        }
        42 => verdadeiro_ou_erro(renomear_diretorio(&caminho()?, &texto(2)?)),
        _ => return None,
    })
}

/// `IOService_NewServicePort`: a `SendPort` de uma porta nativa atendida
/// pela reserva de threads.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_IOService_NewServicePort() -> i64 {
    let id = abrir_porta_nativa(std::sync::Arc::new(|_porta, grafo| reserva_do_servico().enviar(grafo)));
    let Some(f) = ajudante("_dartforgeSendPort") else {
        panic!("bug do compilador: dart:isolate sem `_dartforgeSendPort` registrado");
    };
    // SAFETY: registrada pela biblioteca `dart:isolate` da sobreposição com
    // a assinatura (`int`) → `SendPort`.
    let g: extern "C" fn(i64) -> i64 = unsafe { std::mem::transmute(f) };
    g(id)
}
