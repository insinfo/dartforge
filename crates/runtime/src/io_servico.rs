// Runtime nativo: o IOService da VM (`runtime/bin/io_service.cc`) — a
// porta nativa que atende as operações assíncronas de arquivo e diretório
// do `dart:io` (`File.readAsString()`, `Directory.list()`…) numa reserva de
// threads, fora do isolado.
//
// O Dart do patch da VM (`io_service_patch.dart`) envia
// `[id, port de response, request, data]`; a thread que atende responde
// `[id, result]` na porta de resposta, que chega à fila do isolado como
// qualquer mensagem. Os pedidos são o `IO_SERVICE_REQUEST_LIST` e os
// resultados o que os `*Request` de `file.cc`/`directory.cc` devolvem: o
// valor, ou uma lista de erro `[kArgumentError]`, `[kFileClosedError]`,
// `[kOSError, código, message]`.
//
// Como a porta concorrente da VM (`Dart_NewConcurrentNativePort`, até 32
// threads), os pedidos correm em paralelo; o Dart dos patches serializa o
// que precisa (um mesmo arquivo nunca tem dois pedidos em voo).

/// `CObject::kSuccess`, `kArgumentError`, `kOSError`, `kFileClosedError`.
const RESPONSE_SUCCESS: i64 = 0;
const RESPONSE_INVALID_ARGUMENT: i64 = 1;
const RESPONSE_OS_ERROR: i64 = 2;
const RESPONSE_FILE_CLOSED: i64 = 3;

/// O máximo de threads que atendem o IOService ao mesmo tempo.
const MAX_SERVICE_THREADS: usize = 32;

/// Quantos itens um `ListNext` devolve de uma vez (o `kArraySize` da VM).
const LISTING_BATCH: usize = 128;

/// A reserva de threads do IOService: uma fila de pedidos e as threads que
/// a esvaziam, criadas sob demanda até o máximo.
struct ServicePool {
    queue: std::sync::Mutex<std::collections::VecDeque<Graph>>,
    sig_num: std::sync::Condvar,
    threads: std::sync::atomic::AtomicUsize,
    idle: std::sync::atomic::AtomicUsize,
}

fn service_pool() -> &'static ServicePool {
    static R: std::sync::OnceLock<ServicePool> = std::sync::OnceLock::new();
    R.get_or_init(|| ServicePool {
        queue: std::sync::Mutex::new(std::collections::VecDeque::new()),
        sig_num: std::sync::Condvar::new(),
        threads: std::sync::atomic::AtomicUsize::new(0),
        idle: std::sync::atomic::AtomicUsize::new(0),
    })
}

impl ServicePool {
    /// Enfileira um pedido e acorda (ou cria) uma thread para ele.
    fn enviar(&'static self, request: Graph) {
        use std::sync::atomic::Ordering;
        self.queue.lock().unwrap_or_else(|e| e.into_inner()).push_back(request);
        if self.idle.load(Ordering::Acquire) == 0 && self.threads.load(Ordering::Acquire) < MAX_SERVICE_THREADS {
            self.threads.fetch_add(1, Ordering::AcqRel);
            let created = std::thread::Builder::new().name("dart:io IOService".to_string()).spawn(move || self.serve());
            if created.is_err() {
                self.threads.fetch_sub(1, Ordering::AcqRel);
            }
        }
        self.sig_num.notify_one();
    }

    /// O laço de uma thread: tira um pedido, atende, repete.
    fn serve(&self) {
        use std::sync::atomic::Ordering;
        loop {
            let request = {
                let mut queue = self.queue.lock().unwrap_or_else(|e| e.into_inner());
                loop {
                    if let Some(p) = queue.pop_front() {
                        break p;
                    }
                    self.idle.fetch_add(1, Ordering::AcqRel);
                    queue = self.sig_num.wait(queue).unwrap_or_else(|e| e.into_inner());
                    self.idle.fetch_sub(1, Ordering::AcqRel);
                }
            };
            serve_io_request(request);
        }
    }
}

/// `IOServiceCallback`: lê `[id, port, request, data]`, atende e responde
/// `[id, result]`.
fn serve_io_request(g: Graph) {
    let message = g.to_portable();
    let Some([id, port, request, data]) = message.list() else {
        return;
    };
    let Some(port) = (match port {
        Portable::Objeto(fields) => fields.first().and_then(Portable::int),
        _ => None,
    }) else {
        return;
    };
    let result = match (request.int(), data.list()) {
        (Some(p), Some(d)) => answer_io_request(p, d),
        _ => invalid_argument(),
    };
    post(port, Portable::List(vec![id.clone(), result]).to_graph());
}

fn invalid_argument() -> Portable {
    Portable::List(vec![Portable::Int(RESPONSE_INVALID_ARGUMENT)])
}

fn closed_file() -> Portable {
    Portable::List(vec![Portable::Int(RESPONSE_FILE_CLOSED)])
}

/// O resultado Dart de uma operação: o valor por `f`, ou o erro do sistema.
fn response<T>(r: IoResult<T>, f: impl FnOnce(T) -> Portable) -> Portable {
    match r {
        Ok(v) => f(v),
        Err(e) => e.to_response(),
    }
}

fn true_or_error(r: IoResult<()>) -> Portable {
    response(r, |()| Portable::Bool(true))
}

/// Um caminho em bytes (`CObjectUint8Array`, terminado em NUL).
fn request_path(p: &Portable) -> Option<std::path::PathBuf> {
    let b = p.bytes()?;
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    Some(path_from_bytes(&b[..end]))
}

fn request_text(p: &Portable) -> Option<std::path::PathBuf> {
    Some(path_from_bytes(p.str()?.as_bytes()))
}

/// Atende o pedido `request` (o `IO_SERVICE_REQUEST_LIST` da VM) com `d`.
fn answer_io_request(request: i64, d: &[Portable]) -> Portable {
    // Os pedidos sobre um arquivo aberto: `d[0]` é o ponteiro, com a
    // referência que o `getPointer` reteve (solta ao fim do pedido).
    if matches!(request, 7..=11 | 17..=22 | 30) {
        let Some(p) = d.first().and_then(Portable::int).filter(|&p| p != 0) else {
            return if request == 7 { Portable::Int(-1) } else { invalid_argument() };
        };
        // SAFETY: o Dart só envia ponteiros de `getPointer`, que reteve esta
        // referência para o pedido.
        let file = unsafe { NativeFile::take(p) };
        return answer_file_request(request, &file, &d[1..]);
    }
    match request {
        // Listagem assíncrona: `d[0]` é o ponteiro da listagem (retido).
        40 | 41 => {
            let Some(p) = d.first().and_then(Portable::int).filter(|&p| p != 0) else {
                return invalid_argument();
            };
            // SAFETY: ponteiro de `getPointer` com a referência do pedido.
            let listing = unsafe { AsyncListing::take(p) };
            if request == 40 {
                Portable::List(listing.batch(LISTING_BATCH))
            } else {
                listing.listing.lock().unwrap_or_else(|e| e.into_inner()).stop();
                Portable::Bool(true)
            }
        }
        // Pedidos sem caminho.
        31 => response(create_pipe(), |(r, w)| {
            Portable::List(vec![Portable::Int(NativeFile::new(r)), Portable::Int(NativeFile::new(w))])
        }),
        // Resolução de nomes e interfaces de rede (`io_soquetes.rs`).
        32 => resolve_request(d),
        33 => interfaces_request(d),
        34 => reverse_resolve_request(d),
        _ => answer_path_request(request, d).unwrap_or_else(invalid_argument),
    }
}

/// Os pedidos sobre um arquivo aberto.
fn answer_file_request(request: i64, a: &NativeFile, d: &[Portable]) -> Portable {
    if a.closed_flag() {
        return if request == 7 { Portable::Int(-1) } else { closed_file() };
    }
    let int = |i: usize| d.get(i).and_then(Portable::int);
    match request {
        7 => {
            a.do_close();
            Portable::Int(0)
        }
        8 => response(a.cur_position(), Portable::Int),
        9 => match int(0) {
            Some(p) => true_or_error(a.set_position(p)),
            None => invalid_argument(),
        },
        10 => match int(0) {
            Some(n) => true_or_error(a.truncate_file(n)),
            None => invalid_argument(),
        },
        11 => response(a.size(), Portable::Int),
        17 => true_or_error(a.sync_file()),
        18 => {
            let mut b = [0u8; 1];
            response(a.do_read(&mut b), |n| Portable::Int(if n == 1 { i64::from(b[0]) } else { -1 }))
        }
        19 => match int(0) {
            Some(v) => response(a.write_all_bytes(&[v as u8]), |()| Portable::Int(1)),
            None => invalid_argument(),
        },
        20 | 21 => {
            let Some(n) = int(0).filter(|&n| n >= 0) else {
                return invalid_argument();
            };
            let mut buf = vec![0u8; n as usize];
            response(a.do_read(&mut buf), |read_list| {
                buf.truncate(read_list);
                if request == 20 {
                    Portable::List(vec![Portable::Int(RESPONSE_SUCCESS), Portable::Bytes(buf)])
                } else {
                    Portable::List(vec![Portable::Int(RESPONSE_SUCCESS), Portable::Int(read_list as i64), Portable::Bytes(buf)])
                }
            })
        }
        22 => {
            let (Some(start), Some(end)) = (int(1), int(2)) else {
                return invalid_argument();
            };
            let bytes: Vec<u8> = match d.first() {
                Some(Portable::Bytes(b)) => b.clone(),
                Some(Portable::List(l)) => {
                    let mut b = Vec::with_capacity(l.len());
                    for x in l {
                        match x.int() {
                            Some(v) => b.push(v as u8),
                            None => return invalid_argument(),
                        }
                    }
                    b
                }
                _ => return invalid_argument(),
            };
            if start < 0 || end < start || end as usize > bytes.len() {
                return invalid_argument();
            }
            response(a.write_all_bytes(&bytes[start as usize..end as usize]), |()| Portable::Int(end - start))
        }
        30 => match (int(0), int(1), int(2)) {
            (Some(t), Some(i), Some(f)) => true_or_error(a.lock_file(t, i, f)),
            _ => invalid_argument(),
        },
        _ => invalid_argument(),
    }
}

/// Os pedidos sobre caminhos: `d[0]` é o namespace (ignorado: só há o
/// padrão), `d[1]` o caminho. `None`: argumentos de forma errada.
fn answer_path_request(request: i64, d: &[Portable]) -> Option<Portable> {
    let file_path = || d.get(1).and_then(request_path);
    let text = |i: usize| d.get(i).and_then(request_text);
    let boolean = |i: usize| d.get(i).and_then(Portable::bool);
    let int = |i: usize| d.get(i).and_then(Portable::int);
    Some(match request {
        0 => Portable::Bool(file_exists(&file_path()?)),
        1 => true_or_error(create_file(&file_path()?, boolean(2)?)),
        2 => true_or_error(delete_file(&file_path()?)),
        3 => true_or_error(rename_file(&file_path()?, &text(2)?)),
        4 => true_or_error(copy_file(&file_path()?, &text(2)?)),
        5 => response(open_file(&file_path()?, int(2)?), |fd| Portable::Int(NativeFile::new(fd))),
        6 => response(canonical_path(&file_path()?), |b| Portable::Str(String::from_utf8_lossy(&b).into_owned())),
        12 => response(file_metadata(&file_path()?), |m| Portable::Int(m.len() as i64)),
        13 => response(last_accessed(&file_path()?), Portable::Int),
        14 => response(change_instants(&file_path()?, Some(int(2)?), None), |()| Portable::Null),
        15 => response(last_modified(&file_path()?), Portable::Int),
        16 => response(change_instants(&file_path()?, None, Some(int(2)?)), |()| Portable::Null),
        23 => true_or_error(create_link(&file_path()?, &text(2)?)),
        24 => true_or_error(delete_link(&file_path()?)),
        25 => true_or_error(rename_link(&file_path()?, &text(2)?)),
        26 => response(link_target(&file_path()?), |b| Portable::Str(String::from_utf8_lossy(&b).into_owned())),
        27 => Portable::Int(path_type(&file_path()?, boolean(2)?)),
        28 => response(identical(&text(1)?, &text(2)?), Portable::Bool),
        29 => response(statistics(&text(1)?), |e| {
            Portable::List(vec![Portable::Int(RESPONSE_SUCCESS), Portable::List(e.iter().map(|&x| Portable::Int(x)).collect())])
        }),
        35 => true_or_error(create_directory(&file_path()?)),
        36 => true_or_error(delete_directory(&file_path()?, boolean(2)?)),
        37 => response(directory_exists(&file_path()?), |e| Portable::Int(if e { DIRECTORY_EXISTS } else { DIRECTORY_NOT_FOUND })),
        38 => {
            let b = d.get(1)?.bytes()?;
            let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
            response(create_temp_directory(&b[..end]), |n| Portable::Str(String::from_utf8_lossy(&n).into_owned()))
        }
        39 => {
            let b = d.get(1)?.bytes()?;
            let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
            Portable::Int(AsyncListing::new_area(Listing::new_area(b[..end].to_vec(), boolean(2)?, boolean(3)?)))
        }
        42 => true_or_error(rename_directory(&file_path()?, &text(2)?)),
        _ => return None,
    })
}

/// `IOService_NewServicePort`: a `SendPort` de uma porta nativa atendida
/// pela reserva de threads.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_IOService_NewServicePort() -> i64 {
    let id = open_native_port(std::sync::Arc::new(|_port, graph| service_pool().enviar(graph)));
    let Some(f) = ajudante("_dartforgeSendPort") else {
        panic!("bug do compilador: dart:isolate sem `_dartforgeSendPort` registrado");
    };
    // SAFETY: registrada pela biblioteca `dart:isolate` da sobreposição com
    // a assinatura (`int`) → `SendPort`.
    let g = { let dart_target: usize = f; move |a0: i64| -> i64 { dart_r1(dart_target, a0) } };
    g(id)
}
