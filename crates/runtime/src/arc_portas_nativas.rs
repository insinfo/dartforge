// Fronteira entre callbacks Dart Owned e consumidores nativos emprestados.
// O registro é explícito: endereços e palavras não declaram ownership.

#[derive(Clone, Copy)]
struct ArcReturnRecord {
    version: i64,
    calls: usize,
    uniform: bool,
}

struct ArcReturnScope {
    id: i64,
    calls: usize,
    owners: Vec<i64>,
}

#[derive(Default)]
struct ArcNativePorts {
    entries: HashMap<(usize, usize), ArcReturnRecord>,
    tipadas: HashMap<(usize, i64), i64>,
}

// A ABI base 5 omite o contexto Ref inicial; codifica argumentos e retorno.
// Ref transfere owner; i64/f64/bool são escalares sem token gerenciado.
fn check_arc_typed_abi(mut abi: i64) -> Result<(), &'static str> {
    if abi <= 1 || !matches!(abi % 5, 1..=4) { return Err("retorno tipado inválido"); }
    let mut n = 0;
    while abi > 1 {
        if !matches!(abi % 5, 1..=4) { return Err("representação tipada inválida"); }
        abi /= 5; n += 1;
        if n > 13 { return Err("aridade tipada fora do limite"); }
    }
    if abi != 1 { return Err("prefixo da ABI tipada inválido"); }
    Ok(())
}

fn arc_typed_target(
    heap: &Heap, ports: &ArcNativePorts, closure: i64, abi: i64,
) -> Result<i64, &'static str> {
    check_arc_typed_abi(abi)?;
    let object = heap.object(closure).ok_or("receiver tipado não é objeto vivo")?;
    if object.class_id != i64::from(crate::layout::cid::CLOSURE) || object.len() != 4 {
        return Err("receiver tipado não é closure de quatro campos");
    }
    let (code, code_ref) = object.get(2).ok_or("corpo tipado ausente")?;
    let (declared, ref_abi) = object.get(3).ok_or("ABI tipada ausente")?;
    if code_ref || ref_abi || code == 0 || declared != abi {
        return Err("corpo/ABI tipada incompatível com o mapa físico");
    }
    let code_usize = usize::try_from(code as u64).map_err(|_| "endereço tipado inválido")?;
    if !ports.tipadas.contains_key(&(code_usize, abi)) {
        return Err("corpo tipado sem registro ARC desta ABI");
    }
    Ok(code)
}

impl ArcNativePorts {
    fn register_typed(&mut self, code: usize, abi: i64) -> Result<i64, &'static str> {
        check_arc_typed_abi(abi)?;
        if code == 0 { return Err("corpo tipado nulo"); }
        Ok(*self.tipadas.entry((code, abi)).or_insert_with(new_arc_port_id))
    }

    fn remove_typed(&mut self, code: usize, abi: i64, version: i64) -> Result<(), &'static str> {
        if self.tipadas.get(&(code, abi)) != Some(&version) {
            return Err("registro tipado ausente ou versão obsoleta");
        }
        self.tipadas.remove(&(code, abi));
        Ok(())
    }
    fn register(&mut self, code: usize, arity: usize) -> Result<i64, &'static str> {
        self.register_contract(code, arity, false)
    }

    fn register_contract(
        &mut self,
        code: usize,
        arity: usize,
        uniform: bool,
    ) -> Result<i64, &'static str> {
        if code == 0 || !matches!(arity, 0..=5 | 7) || (uniform && arity != 3) {
            return Err("ABI de callback não coberta");
        }
        // O ligador pode fundir código de funções com assinaturas distintas.
        // A declaração pertence à ABI lógica (endereço, aridade), não só ao PC.
        if let Some(r) = self.entries.get(&(code, arity)) {
            if r.uniform != uniform {
                return Err("contrato uniforme conflita com callback de palavras");
            }
            return Ok(r.version);
        }
        let version = new_arc_port_id();
        self.entries.insert(
            (code, arity),
            ArcReturnRecord {
                version,
                calls: 0,
                uniform,
            },
        );
        Ok(version)
    }

    fn deregister(&mut self, code: usize, version: i64) -> Result<(), &'static str> {
        let key = self
            .entries
            .iter()
            .find_map(|(k, r)| (k.0 == code && r.version == version).then_some(*k))
            .ok_or("callback não registrado ou versão obsoleta")?;
        if self.entries[&key].calls != 0 {
            return Err("callback ativo");
        }
        self.entries.remove(&key);
        Ok(())
    }
}

#[derive(Default)]
struct ArcPortScopes {
    scopes: Vec<ArcReturnScope>,
}

impl ArcPortScopes {
    fn do_close(&mut self, id: i64) -> Result<Vec<i64>, &'static str> {
        let e = self.scopes.last().ok_or("escopo nativo ausente")?;
        if e.id != id || e.calls != 0 {
            return Err("escopo fora do topo ou callback ativo");
        }
        Ok(self.scopes.pop().unwrap().owners)
    }
}

fn new_arc_port_id() -> i64 {
    static PROXIMO: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);
    PROXIMO
        .fetch_update(
            std::sync::atomic::Ordering::Relaxed,
            std::sync::atomic::Ordering::Relaxed,
            |n| n.checked_add(1),
        )
        .expect("IDs de portas ARC esgotados")
        + 1
}

// Código pertence à imagem, compartilhada pelos isolados. A contagem global
// impede retirar uma versão que está em execução em qualquer thread.
fn arc_port_records() -> &'static std::sync::Mutex<ArcNativePorts> {
    static RECORDS: std::sync::OnceLock<std::sync::Mutex<ArcNativePorts>> =
        std::sync::OnceLock::new();
    RECORDS.get_or_init(|| std::sync::Mutex::new(ArcNativePorts::default()))
}

static HAS_ARC_RETURN_PORTS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

thread_local! {
    // Os valores gerenciados pertencem ao heap da thread que recebeu o retorno.
    static ARC_NATIVE_PORTS: RefCell<ArcPortScopes> = RefCell::new(ArcPortScopes::default());
}

/// Declara retorno Ref Owned de um callback de palavras e devolve sua versão.
///
/// Registro repetido com a mesma ABI é idempotente. Não chama código, coleta
/// ou executa Dart. Não seleciona o modo de memória nem prova o corpo Dart.
///
/// # Safety
/// `code` deve ter a ABI da porta indicada por `arity`, com parâmetros
/// de uma palavra e resultado Ref Owned na saída normal, null na saída pending.
/// Parâmetros Ref são emprestados; o callback não consome tokens do runtime.
/// O callback deve restaurar os escopos nativos antes de retornar.
/// O produtor deve verificar o corpo e manter a imagem válida até retirar
/// o registro e todas as referências/chamadas ao código. Não admite ABI f64.
///
/// # Panics
/// Código nulo ou aridade não coberta; aborta no C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// extern "C" fn null_val() -> i64 { 0 }
/// // SAFETY: null é Ref without token físico; função estática de arity zero.
/// let v = unsafe { dartforge_arc_registrar_retorno_owned_v1(null_val as usize as i64, 0) };
/// dartforge_arc_retirar_retorno_owned_v1(null_val as usize as i64, v);
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arc_registrar_retorno_owned_v1(
    code: i64,
    arity: i64,
) -> i64 {
    let code = usize::try_from(code as u64).expect("endereço de callback inválido");
    let arity = usize::try_from(arity).expect("aridade negativa");
    let mut p = arc_port_records()
        .lock()
        .expect("registro de portas ARC envenenado");
    let version = p
        .register(code, arity)
        .expect("contrato de callback ARC violado");
    HAS_ARC_RETURN_PORTS.store(true, std::sync::atomic::Ordering::Release);
    version
}

/// Declara uma entrada uniforme Owned com vetores somente de leitura.
///
/// O contrato é distinto do callback genérico de três palavras. O registro
/// não verifica o corpo nem seleciona o modo de memória.
///
/// # Safety
/// O código tem ABI `(Ref, ptr args, ptr desc) -> Ref Owned`, null em pending.
/// Receptor e elementos Ref são emprestados. Não altera, publica ou retém
/// endereços dos vetores; pode copiar seus elementos respeitando ownership.
/// Toda chamada transitiva que recebe esses endereços respeita o mesmo contrato.
/// O produtor verifica o corpo emitido e restaura escopos em todas as saídas.
/// A imagem vive até retirar o registro e todos os consumidores do código.
///
/// # Panics
/// Código nulo ou contrato anterior incompatível; aborta no C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// extern "C" fn smi(_: i64, _: *const i64, _: *const i64) -> i64 { 3 }
/// // SAFETY: body estático que não published os vetores, retorna Smi without token.
/// let v = unsafe { dartforge_arc_registrar_uniforme_owned_v1(smi as *const () as usize as i64) };
/// dartforge_arc_retirar_retorno_owned_v1(smi as *const () as usize as i64, v);
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arc_registrar_uniforme_owned_v1(code: i64) -> i64 {
    let code = usize::try_from(code as u64).expect("endereço uniforme inválido");
    let mut p = arc_port_records().lock().expect("registro de portas ARC envenenado");
    let version = p.register_contract(code, 3, true).expect("contrato uniforme ARC violado");
    HAS_ARC_RETURN_PORTS.store(true, std::sync::atomic::Ordering::Release);
    version
}

/// Registra uma entrada tipada de closure com retorno Owned ou escalar.
///
/// A ABI base 5 codifica argumentos (Ref=1, i64=2, f64=3, bool=4) e retorno,
/// depois do prefixo 1; o contexto Ref inicial é implícito. Não chama código.
///
/// # Safety
/// O corpo foi verificado nessa ABI: Ref emprestadas, retorno Ref Owned ou
/// escalar Trivial; resultado canônico em pending e escopos restaurados.
/// Um i64 escalar não contém word Dart gerenciada. O registro não prova o corpo.
/// Mantenha código/imagem válidos até retirar a versão e terminar todos os
/// consumidores do endereço. Não estabelece pins de recarga JIT.
///
/// # Panics
/// Código nulo, ABI inválida ou mais de 12 argumentos.
///
/// ```
/// use dartforge_runtime::abi::*;
/// extern "C" fn body(_: i64) -> i64 { 0 }
/// let pc = body as *const () as usize as i64;
/// // SAFETY: função estática, contexto borrowed e ret null Owned.
/// let v = unsafe { dartforge_arc_registrar_tipada_v1(pc, 6) };
/// // SAFETY: nenhum consumidor/chamada do endereço está ativo.
/// unsafe { dartforge_arc_retirar_tipada_v1(pc, 6, v); }
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arc_registrar_tipada_v1(code: i64, abi: i64) -> i64 {
    arc_port_records().lock().expect("registro de portas ARC envenenado")
        .register_typed(usize::try_from(code as u64).expect("endereço tipado inválido"), abi)
        .expect("registro tipado ARC inválido")
}

/// Retira exatamente uma versão e ABI de entrada tipada.
///
/// # Safety
/// Não pode haver chamada ativa nem consumidor de endereço obtido por
/// `dartforge_arc_alvo_tipada_v1`. O código deve continuar válido até aqui.
///
/// # Panics
/// Registro ausente ou versão obsoleta; aborta na fronteira C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// extern "C" fn body(_: i64) -> i64 { 0 }
/// let pc = body as *const () as usize as i64;
/// // SAFETY: ABI estática e nenhum consumidor ativo.
/// let v = unsafe { dartforge_arc_registrar_tipada_v1(pc, 6) };
/// unsafe { dartforge_arc_retirar_tipada_v1(pc, 6, v); }
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arc_retirar_tipada_v1(code: i64, abi: i64, version: i64) {
    arc_port_records().lock().expect("registro de portas ARC envenenado")
        .remove_typed(usize::try_from(code as u64).expect("endereço tipado inválido"), abi, version)
        .expect("retirada tipada ARC inválida");
}

/// Confere classe, mapa, ABI e registro ARC antes de devolver código nativo.
///
/// Não retém, aloca, coleta ou executa Dart. O resultado é endereço nativo,
/// não Ref. Sua validade depende do contrato de registro e vida da imagem.
///
/// # Panics
/// Receiver, campos, ABI ou registro inválidos; aborta na fronteira C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// extern "C" fn body(_: i64) -> i64 { 0 }
/// let pc = body as *const () as usize as i64;
/// // SAFETY: ABI estática de contexto Ref e result Ref null Owned.
/// let v = unsafe { dartforge_arc_registrar_tipada_v1(pc, 6) };
/// let c = dartforge_arc_closure_owned_v1(0, 0, pc, 6);
/// assert_eq!(dartforge_arc_alvo_tipada_v1(c, 6), pc);
/// dartforge_arc_release(c);
/// // SAFETY: nenhum consumidor do endereço permanece.
/// unsafe { dartforge_arc_retirar_tipada_v1(pc, 6, v); }
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_alvo_tipada_v1(closure: i64, abi: i64) -> i64 {
    HEAP.with(|heap| arc_typed_target(&heap.borrow(),
        &arc_port_records().lock().expect("registro de portas ARC envenenado"), closure, abi)
        .expect("alvo tipado ARC inválido"))
}

struct ArcUniformScope(i64);

impl Drop for ArcUniformScope {
    fn drop(&mut self) {
        dartforge_arc_escopo_nativo_fechar_v1(self.0);
    }
}

/// `f` dentro de um escopo nativo, sem mudar o resultado: os tokens que
/// portas Dart devolvem durante `f` morrem no fechamento, mesmo com pânico.
pub(crate) fn in_native_scope<T>(f: impl FnOnce() -> T) -> T {
    let scope = ArcUniformScope(dartforge_arc_escopo_nativo_abrir_v1());
    let r = f();
    drop(scope);
    r
}

/// `f` num escopo nativo só quando alguma imagem registrou portas ARC
/// (um evento do laço, a vida de um isolado): sem elas, nada muda.
pub(crate) fn in_native_scope_if_arc<T>(f: impl FnOnce() -> T) -> T {
    if HAS_ARC_RETURN_PORTS.load(std::sync::atomic::Ordering::Acquire) {
        in_native_scope(f)
    } else {
        f()
    }
}

/// Roda `f` (que pode executar Dart e devolve um objeto vivo ao voltar) num
/// escopo nativo e entrega o resultado com um owner do código. Os tokens dos
/// retornos Owned das portas Dart ficam no escopo e são soltos ao fechá-lo,
/// depois da cópia. Com exceção pendente, null e nenhum owner.
pub(crate) fn owner_in_native_scope(f: impl FnOnce() -> i64) -> i64 {
    let scope = ArcUniformScope(dartforge_arc_escopo_nativo_abrir_v1());
    let r = f();
    let result = if dartforge_exception_pending() != 0 {
        0
    } else {
        dartforge_arc_retain(r);
        r
    };
    drop(scope);
    result
}

fn arc_invoke_uniform(
    code: usize,
    recv: i64,
    args: i64,
    desc: i64,
) -> Result<i64, &'static str> {
    let scope = ArcUniformScope(dartforge_arc_escopo_nativo_abrir_v1());
    let result = if code == dartforge_nsm_seletor as *const () as usize {
        // Este corpo nativo lê/copia os vetores, sem os encaminhar ao SDK.
        // O helper Dart usa dart_r7: seu Owned fica emprestado neste escopo.
        let r = dartforge_nsm_seletor(recv, args as *const i64, desc as *const i64);
        if dartforge_exception_pending() != 0 {
            assert_eq!(r, 0, "noSuchMethod pendente deve devolver null");
            0
        } else {
            // A convenção antiga devolve Borrowed, inclusive se o helper era
            // Owned capturado pela fronteira. Copia antes de liberar o escopo.
            dartforge_arc_retain(r);
            r
        }
    } else {
        let guard = arc_prepare_contract_port(code, 3, true)?
            .ok_or("entrada uniforme não registrada")?;
        let r = dart_r3_invoke(code, recv, args, desc);
        arc_check_port_scope(&guard);
        arc_validate_owned_return(r);
        // O retorno direto já é Owned: não o transfere ao escopo nem retém.
        // A guarda precisa terminar antes do fechamento do escopo que prende.
        drop(guard);
        r
    };
    drop(scope);
    Ok(result)
}

/// Invoca entrada uniforme certificada e devolve exatamente um retorno Owned.
///
/// Aceita o fallback nativo noSuchMethod, adaptando seu retorno emprestado.
/// Fecha o escopo de callbacks também em pending, sem limpar a exceção.
/// Código desconhecido e callback genérico não são inferidos como uniformes.
///
/// # Safety
/// `code` e sua imagem permanecem vivos durante a chamada. `args` e `desc`
/// são endereços válidos da ABI uniforme: dois contadores não negativos,
/// hashes nomeados e vetor com todos os argumentos mais a tupla RTI.
/// Receptor e elementos Ref permanecem vivos por todo o alcance da chamada.
///
/// # Panics
/// Entrada não certificada ou violação de ownership/escopos; aborta no C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// extern "C" fn null_val(_: i64, _: *const i64, _: *const i64) -> i64 { 0 }
/// let pc = null_val as *const () as usize as i64;
/// // SAFETY: body não published vetores nem cria tokens, devolve Ref null.
/// let v = unsafe { dartforge_arc_registrar_uniforme_owned_v1(pc) };
/// let args = [0_i64]; let desc = [0_i64, 0];
/// // SAFETY: vetores váread_list e body registrado estático.
/// assert_eq!(unsafe { dartforge_arc_invocar_uniforme_v1(pc, 0,
///     args.as_ptr() as i64, desc.as_ptr() as i64) }, 0);
/// dartforge_arc_retirar_retorno_owned_v1(pc, v);
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arc_invocar_uniforme_v1(
    code: i64,
    recv: i64,
    args: i64,
    desc: i64,
) -> i64 {
    let code = usize::try_from(code as u64).expect("endereço uniforme inválido");
    arc_invoke_uniform(code, recv, args, desc).expect("fronteira uniforme ARC inválida")
}

fn arc_invoke_uniform_closure(closure: i64, args: i64, desc: i64) -> Result<i64, &'static str> {
    // A busca de entrada também pode executar o helper Dart de erro. Prende
    // seus retornos intermediários até publicar a exceção, inclusive se falhar.
    let scope = ArcUniformScope(dartforge_arc_escopo_nativo_abrir_v1());
    let code = dartforge_closure_entry(closure);
    let result = if code == 0 {
        assert_ne!(dartforge_exception_pending(), 0, "closure inválida sem exceção");
        0
    } else {
        arc_invoke_uniform(code as u64 as usize, closure, args, desc)?
    };
    drop(scope);
    Ok(result)
}

/// Invoca uma closure ou objeto chamável com retorno uniforme Owned.
///
/// A busca inválida preserva a exceção e devolve null, sem invocar código.
/// Entradas válidas precisam do registro uniforme; callback genérico não basta.
/// Fecha os escopos de busca e chamada, preservando o único owner do resultado.
///
/// # Safety
/// Closure/receptor e elementos Ref ficam vivos por toda a chamada. `args` e
/// `desc` apontam para vetores válidos da ABI uniforme, somente de leitura.
/// A imagem de código permanece viva enquanto houver consumidores da closure.
///
/// # Panics
/// Destino sem contrato uniforme ou violação de ownership/escopos; aborta no C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// extern "C" fn null_val(_: i64, _: *const i64, _: *const i64) -> i64 { 0 }
/// let pc = null_val as *const () as usize as i64;
/// // SAFETY: body estático readonly com ret Ref null.
/// let v = unsafe { dartforge_arc_registrar_uniforme_owned_v1(pc) };
/// let c = dartforge_arc_closure_owned_v1(pc, 0, 0, 0);
/// let args = [0_i64]; let desc = [0_i64, 0];
/// // SAFETY: closure e vetores live_set; código registrado e estático.
/// assert_eq!(unsafe { dartforge_arc_invocar_closure_uniforme_v1(c,
///     args.as_ptr() as i64, desc.as_ptr() as i64) }, 0);
/// dartforge_arc_release(c);
/// dartforge_arc_retirar_retorno_owned_v1(pc, v);
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arc_invocar_closure_uniforme_v1(closure: i64, args: i64, desc: i64) -> i64 {
    arc_invoke_uniform_closure(closure, args, desc).expect("fronteira closure uniforme ARC inválida")
}

/// Retira exatamente a versão registrada, sem invalidar uma versão posterior.
///
/// Deve ocorrer após retirar os consumidores do endereço. Recusa callbacks
/// em curso; os owners já devolvidos continuam nos escopos nativos.
/// Não coleta, executa Dart ou altera exceção pendente.
///
/// # Panics
/// Registro ausente, versão obsoleta ou callback ativo; aborta no C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// extern "C" fn null_val(_: i64) -> i64 { 0 }
/// // SAFETY: ABI estática de words com ret Ref null.
/// let v = unsafe { dartforge_arc_registrar_retorno_owned_v1(null_val as usize as i64, 1) };
/// dartforge_arc_retirar_retorno_owned_v1(null_val as usize as i64, v);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_retirar_retorno_owned_v1(code: i64, version: i64) {
    let code = usize::try_from(code as u64).expect("endereço de callback inválido");
    let mut p = arc_port_records()
        .lock()
        .expect("registro de portas ARC envenenado");
    p.deregister(code, version)
        .expect("retirada de callback ARC inválida");
    HAS_ARC_RETURN_PORTS.store(!p.entries.is_empty(), std::sync::atomic::Ordering::Release);
}

/// Abre um escopo nativo que recebe os tokens retornados por callbacks Owned.
///
/// Cada retorno é transferido sem retain adicional e fica emprestado ao
/// consumidor nativo até fechar este escopo. Para escapar, copie antes.
/// Escopos reentrantes são independentes. Não coleta nem executa Dart.
///
/// # Panics
/// IDs esgotados; falha interna aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let e = dartforge_arc_escopo_nativo_abrir_v1();
/// dartforge_arc_escopo_nativo_fechar_v1(e);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_escopo_nativo_abrir_v1() -> i64 {
    ARC_NATIVE_PORTS.with(|p| {
        let mut p = p.borrow_mut();
        let id = new_arc_port_id();
        p.scopes.push(ArcReturnScope {
            id,
            calls: 0,
            owners: Vec::new(),
        });
        id
    })
}

/// Fecha o escopo do topo e libera exatamente os tokens que ele recebeu.
///
/// Invalida os empréstimos dos retornos. Não coleta, executa Dart ou limpa
/// exceção pendente. O emissor deverá fechar em todas as saídas, com cleanup
/// de exceções; esta API não instala esse cleanup automaticamente.
///
/// # Panics
/// Escopo ausente, fora do topo ou com callback em curso; aborta no C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let e = dartforge_arc_escopo_nativo_abrir_v1();
/// dartforge_arc_escopo_nativo_fechar_v1(e);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_escopo_nativo_fechar_v1(id: i64) {
    let owners = ARC_NATIVE_PORTS.with(|p| {
        p.borrow_mut()
            .do_close(id)
            .expect("fechamento de escopo nativo ARC inválido")
    });
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        for owner in owners {
            h.release_code_owner(owner);
        }
    });
}

struct ArcPortGuard {
    code: usize,
    arity: usize,
    version: i64,
    scope: i64,
}

fn arc_prepare_port(
    code: usize,
    arity: usize,
) -> Result<Option<ArcPortGuard>, &'static str> {
    arc_prepare_contract_port(code, arity, false)
}

fn arc_prepare_contract_port(
    code: usize,
    arity: usize,
    uniform: bool,
) -> Result<Option<ArcPortGuard>, &'static str> {
    // A convenção antiga não adquire lock nem cria escopo quando nenhuma
    // imagem declarou callbacks Owned. Isto não seleciona ARC implicitamente.
    if !HAS_ARC_RETURN_PORTS.load(std::sync::atomic::Ordering::Acquire) {
        return Ok(None);
    }
    let mut p = arc_port_records()
        .lock()
        .expect("registro de portas ARC envenenado");
    let Some(r) = p.entries.get(&(code, arity)).copied() else {
        return if p.entries.keys().any(|k| k.0 == code) {
            Err("assinatura do callback ARC não registrada")
        } else {
            Ok(None)
        };
    };
    if uniform && !r.uniform {
        return Err("entrada sem contrato uniforme");
    }
    let active_list = r
        .calls
        .checked_add(1)
        .ok_or("callbacks ativos esgotados")?;
    let scope = ARC_NATIVE_PORTS.with(|s| {
        let mut s = s.borrow_mut();
        let e = s
            .scopes
            .last_mut()
            .ok_or("callback Owned exige escopo nativo")?;
        e.calls = e
            .calls
            .checked_add(1)
            .ok_or("callbacks ativos esgotados")?;
        Ok::<_, &'static str>(e.id)
    })?;
    p.entries.get_mut(&(code, arity)).unwrap().calls = active_list;
    Ok(Some(ArcPortGuard {
        code,
        arity,
        version: r.version,
        scope,
    }))
}

impl Drop for ArcPortGuard {
    fn drop(&mut self) {
        let mut p = arc_port_records()
            .lock()
            .expect("registro de portas ARC envenenado");
        let r = p
            .entries
            .get_mut(&(self.code, self.arity))
            .expect("callback ativo retirado");
        assert_eq!(r.version, self.version, "callback ativo substituído");
        r.calls -= 1;
        ARC_NATIVE_PORTS.with(|s| {
            s.borrow_mut()
                .scopes
                .iter_mut()
                .find(|e| e.id == self.scope)
                .expect("escopo do callback fechado")
                .calls -= 1
        });
    }
}

fn arc_validate_owned_return(result: i64) -> bool {
    if dartforge_exception_pending() != 0 {
        assert_eq!(
            result, 0,
            "callback Owned deve devolver null na saída pending"
        );
        return false;
    }
    if smi::e_handle(result) {
        assert!(
            HEAP.with(|h| h.borrow().has_code_owner(result)),
            "callback Owned devolveu referência sem token"
        );
    }
    true
}

fn arc_finish_port(guard: Option<ArcPortGuard>, result: i64) -> i64 {
    if let Some(g) = &guard {
        arc_check_port_scope(g);
        if !arc_validate_owned_return(result) {
            return result;
        }
        ARC_NATIVE_PORTS.with(|p| {
            let mut p = p.borrow_mut();
            let e = p.scopes.last_mut().expect("escopo do callback ausente");
            assert_eq!(e.id, g.scope, "escopo reentrante não foi fechado");
            e.owners.push(result);
        });
    }
    result
}

fn arc_check_port_scope(guard: &ArcPortGuard) {
    ARC_NATIVE_PORTS.with(|p| {
        assert_eq!(
            p.borrow().scopes.last().map(|e| e.id),
            Some(guard.scope),
            "escopo reentrante não foi fechado"
        )
    });
}

fn arc_discard_port(guard: ArcPortGuard, result: i64) {
    arc_check_port_scope(&guard);
    // A guarda continua prendendo a versão e o escopo durante a liberação.
    if arc_validate_owned_return(result) {
        HEAP.with(|h| h.borrow_mut().release_code_owner(result));
    }
}

#[cfg(test)]
mod testes_portas_arc {
    use super::*;
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn typed_target_checks_abi_map_and_version_without_mistaking_word_record() {
        let mut heap = Heap::new(false);
        let mut ports = ArcNativePorts::default();
        let c = heap.new_closure(3, (0, false), 3, 31);
        ports.register(3, 2).unwrap();
        assert!(arc_typed_target(&heap, &ports, c, 31).is_err());
        let v = ports.register_typed(3, 31).unwrap();
        assert_eq!(ports.register_typed(3, 31).unwrap(), v);
        let other = ports.register_typed(3, 41).unwrap();
        assert_ne!(other, v);
        let stats = heap.stats();
        assert_eq!(arc_typed_target(&heap, &ports, c, 31), Ok(3));
        for abi in [-1, 0, 1, 5, 7, 11, 26, i64::MAX, 41] {
            assert!(arc_typed_target(&heap, &ports, c, abi).is_err(), "{abi}");
        }
        let limit = (0..13).fold(1_i64, |a, _| a * 5 + 1);
        for abi in [6, 7, 8, 9, 31, 32, 33, 34] { assert!(check_arc_typed_abi(abi).is_ok()); }
        assert!(check_arc_typed_abi(30).is_err());
        assert!(check_arc_typed_abi(limit).is_ok());
        assert!(check_arc_typed_abi(limit * 5 + 1).is_err());
        for (n, field) in [(2, (3, true)), (3, (31, true)), (2, (0, false)), (3, (41, false))] {
            heap.set_field(c, n, field.0, field.1);
            assert!(arc_typed_target(&heap, &ports, c, 31).is_err());
            heap.set_field(c, 2, 3, false); heap.set_field(c, 3, 31, false);
        }
        for (cid, n) in [(32000, 4), (crate::layout::cid::CLOSURE, 3), (crate::layout::cid::CLOSURE, 5)] {
            let false_value = heap.new_object(i64::from(cid), &vec![(0, false); n]);
            assert!(arc_typed_target(&heap, &ports, false_value, 31).is_err());
        }
        assert!(ports.remove_typed(3, 31, other).is_err());
        ports.remove_typed(3, 31, v).unwrap();
        assert!(arc_typed_target(&heap, &ports, c, 31).is_err());
        let new = ports.register_typed(3, 31).unwrap();
        assert_ne!(new, v);
        assert!(ports.remove_typed(3, 31, v).is_err());
        assert_eq!(heap.stats().collections, stats.collections);
    }

    #[test]
    fn checked_typed_port_returns_independent_owner_in_both_modes() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        extern "C" fn body(_: i64, x: i64) -> i64 { dartforge_arc_retain(x); x }
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            let pc = body as *const () as usize as i64;
            let v = unsafe { dartforge_arc_registrar_tipada_v1(pc, 31) };
            let child = dartforge_arc_box_int_owned_v1(i64::MAX);
            let env = dartforge_arc_objeto_owned_v1(i64::from(crate::layout::cid::CONTEXTO), 1);
            dartforge_arc_gravar_campo_ref_v1(env, 0, child);
            let c = dartforge_arc_closure_owned_v1(0, env, pc, 31);
            dartforge_arc_release(env);
            let stats = HEAP.with(|h| h.borrow().stats());
            let target = dartforge_arc_alvo_tipada_v1(c, 31);
            assert_eq!(target, pc);
            assert_eq!(HEAP.with(|h| h.borrow().stats().allocations), stats.allocations);
            assert_eq!(HEAP.with(|h| h.borrow().stats().collections), stats.collections);
            let test_body: extern "C" fn(i64, i64) -> i64 = unsafe { std::mem::transmute(target as usize) };
            let r = test_body(dartforge_arc_contexto_closure_v1(c), child);
            dartforge_arc_release(child); dartforge_arc_release(c); dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(r), 3);
            assert_eq!(dartforge_unbox_int(r), i64::MAX);
            dartforge_arc_release(r); dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(r), 0);
            unsafe { dartforge_arc_retirar_tipada_v1(pc, 31, v); }
            HEAP.with(|h| h.replace(previous));
        }
    }

    thread_local! { static ULTIMO: std::cell::Cell<i64> = const { std::cell::Cell::new(0) }; }

    #[test]
    fn owned_datagram_escapes_scope_and_pending_creates_no_owner() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        extern "C" fn datagram(data: i64, text: i64, ip: i64, port: i64, tipo: i64) -> i64 {
            assert_eq!(typed_list_bytes(data).unwrap(), [1, 2, 3]);
            let r = dartforge_arc_objeto_owned_v1(500, 5);
            for (i, v) in [(0, data), (1, text), (2, ip)] { dartforge_arc_gravar_campo_ref_v1(r, i, v); }
            dartforge_arc_gravar_campo_escalar_v1(r, 3, port);
            dartforge_arc_gravar_campo_escalar_v1(r, 4, tipo);
            r
        }
        extern "C" fn error(_: i64, _: i64, _: i64, _: i64, _: i64) -> i64 {
            let e = dartforge_arc_box_int_owned_v1(i64::MAX);
            dartforge_arc_lancar_ref_v1(e);
            dartforge_arc_release(e);
            0
        }
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            // Usa o descritor da camada do runtime: no Windows ele identifica
            // um Manipulador, não o SOCKET bruto que std::net devolve.
            let e = OsAddress::from_ip(&[127, 0, 0, 1]).unwrap();
            let fd = create_datagram(&e, false, false, 1).unwrap();
            let dest = std::net::SocketAddr::from(([127, 0, 0, 1], local_port(fd) as u16));
            let socket = dartforge_arc_objeto_owned_v1(123, 1);
            set_socket_on_object(socket, fd, SocketFinalizer::Normal);
            let scopes = ARC_NATIVE_PORTS.with(|p| p.borrow().scopes.len());
            assert_eq!(dartforge_arc_socket_recvfrom_owned_v1(socket), 0);
            assert_eq!(dartforge_exception_pending(), 0);
            let p = native_field(socket);
            NativeSocket::retain_ref(p);
            event_handler().enviar(EventCommand { sock: p, port: 0,
                data: (1 << COMMAND_MASK) | (1 << EVENT_IN) });
            let emitter = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
            for (pc, pending) in [(datagram as *const () as usize, false), (error as *const () as usize, true)] {
                let version = register(pc, 5);
                HELPERS.with(|a| a.borrow_mut().insert("_dartforgeDatagrama".into(), pc));
                emitter.send_to(&[1, 2, 3], dest).unwrap();
                let mut r = 0;
                let limit = std::time::Instant::now() + std::time::Duration::from_secs(2);
                while std::time::Instant::now() < limit {
                    r = dartforge_arc_socket_recvfrom_owned_v1(socket);
                    if r != 0 || dartforge_exception_pending() != 0 { break; }
                    std::thread::yield_now();
                }
                assert_eq!(ARC_NATIVE_PORTS.with(|p| p.borrow().scopes.len()), scopes);
                if pending {
                    assert_eq!(r, 0); assert_ne!(dartforge_exception_pending(), 0);
                    dartforge_exception_clear();
                } else {
                    assert_eq!(dartforge_exception_pending(), 0); assert_ne!(r, 0);
                    let data = dartforge_arc_ler_campo_ref_v1(r, 0);
                    dartforge_arc_collect();
                    assert_eq!(dartforge_arc_observar_heap_v1(r), 3);
                    assert_eq!(typed_list_bytes(data).unwrap(), [1, 2, 3]);
                    dartforge_arc_release(r); dartforge_arc_collect();
                    assert_eq!(dartforge_arc_observar_heap_v1(r), 0);
                    assert_eq!(dartforge_arc_observar_heap_v1(data), 0);
                }
                dartforge_arc_retirar_retorno_owned_v1(pc as i64, version);
            }
            HELPERS.with(|a| a.borrow_mut().remove("_dartforgeDatagrama"));
            dartforge_arc_release(socket); dartforge_arc_collect();
            HEAP.with(|h| { h.replace(previous); });
        }
    }

    extern "C" fn new() -> i64 {
        let x = dartforge_arc_box_int_owned_v1(i64::MAX);
        ULTIMO.with(|u| u.set(x));
        x
    }
    extern "C" fn one(x: i64) -> i64 {
        dartforge_arc_retain(x);
        x
    }
    extern "C" fn dois(x: i64, _: i64) -> i64 {
        one(x)
    }
    extern "C" fn tres(x: i64, _: i64, _: i64) -> i64 {
        one(x)
    }
    extern "C" fn four(x: i64, _: i64, _: i64, _: i64) -> i64 {
        one(x)
    }
    extern "C" fn five(x: i64, _: i64, _: i64, _: i64, _: i64) -> i64 {
        one(x)
    }
    extern "C" fn seven(x: i64, _: i64, _: i64, _: i64, _: i64, _: i64, _: i64) -> i64 {
        one(x)
    }
    extern "C" fn scalar() -> i64 {
        i64::MAX - 1
    }

    fn register(f: usize, n: i64) -> i64 {
        // SAFETY: somente as fixtures de palavras/Ref Owned acima são usadas.
        unsafe { dartforge_arc_registrar_retorno_owned_v1(f as i64, n) }
    }

    #[test]
    fn all_ports_transfer_one_token_without_retaining_twice() {
        let _serial = SERIAL.lock().unwrap();
        std::thread::spawn(|| {
            dartforge_memoria_arc_v1();
            let fs = [
                (new as *const () as usize, 0),
                (one as *const () as usize, 1),
                (dois as *const () as usize, 2),
                (tres as *const () as usize, 3),
                (four as *const () as usize, 4),
                (five as *const () as usize, 5),
                (seven as *const () as usize, 7),
            ];
            let entries: Vec<_> = fs.iter().map(|&(f, n)| (f, register(f, n))).collect();
            let e = dartforge_arc_escopo_nativo_abrir_v1();
            let x = new();
            let y = dart_r0(fs[0].0);
            assert_eq!(dart_r1(fs[1].0, x), x);
            assert_eq!(dart_r2(fs[2].0, x, 0), x);
            assert_eq!(dart_r3(fs[3].0, x, 0, 0), x);
            assert_eq!(dart_r4(fs[4].0, x, 0, 0, 0), x);
            assert_eq!(dart_r5(fs[5].0, x, 0, 0, 0, 0), x);
            assert_eq!(dart_r7(fs[6].0, x, 0, 0, 0, 0, 0, 0), x);
            dartforge_arc_release(x);
            dartforge_arc_collect();
            for h in [x, y] {
                assert_eq!(dartforge_arc_observar_heap_v1(h), 3);
            }
            // O escopo é proprietário, mas seu resultado é um empréstimo nativo.
            dartforge_arc_retain(y);
            dartforge_arc_escopo_nativo_fechar_v1(e);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(x), 0);
            assert_eq!(dartforge_arc_observar_heap_v1(y), 3);
            dartforge_arc_release(y);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(y), 0);
            for (f, v) in entries {
                dartforge_arc_retirar_retorno_owned_v1(f as i64, v);
            }
            // Sem registro explícito, uma palavra não ganha owner nem escopo.
            assert_eq!(dart_r0(scalar as *const () as usize), i64::MAX - 1);
        })
        .join()
        .unwrap();
    }

    extern "C" fn reentrant() -> i64 {
        let e = dartforge_arc_escopo_nativo_abrir_v1();
        let x = dart_r0(new as *const () as usize);
        dartforge_arc_retain(x);
        dartforge_arc_escopo_nativo_fechar_v1(e);
        dartforge_arc_collect();
        x
    }

    #[test]
    fn reentrant_scopes_and_observational_frames_stay_distinct() {
        let _serial = SERIAL.lock().unwrap();
        std::thread::spawn(|| {
            dartforge_memoria_arc_v1();
            let f = reentrant as *const () as usize;
            let n = new as *const () as usize;
            let vf = register(f, 0);
            let vn = register(n, 0);
            let frame = HEAP.with(|h| h.borrow_mut().push_frame_with_slots(0));
            let e = dartforge_arc_escopo_nativo_abrir_v1();
            let x = dart_r0(f);
            // Os owners da fronteira não empilham quadros no heap: o runtime
            // pode fechar o quadro observacional que abriu antes do callback.
            HEAP.with(|h| h.borrow_mut().pop_frame(frame));
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(x), 3);
            dartforge_arc_escopo_nativo_fechar_v1(e);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(x), 0);
            dartforge_arc_retirar_retorno_owned_v1(f as i64, vf);
            dartforge_arc_retirar_retorno_owned_v1(n as i64, vn);
        })
        .join()
        .unwrap();
    }

    #[test]
    fn records_and_scopes_reject_stale_version_wrong_abi_and_active_mutation() {
        let _serial = SERIAL.lock().unwrap();
        std::thread::spawn(|| {
            let f = new as *const () as usize;
            let v = register(f, 0);
            assert_eq!(register(f, 0), v);
            assert!(arc_prepare_port(f, 0).is_err());
            let e = dartforge_arc_escopo_nativo_abrir_v1();
            assert!(arc_prepare_port(f, 1).is_err());
            let g = arc_prepare_port(f, 0).unwrap();
            {
                let mut p = arc_port_records().lock().unwrap();
                for n in [6, 8] {
                    assert!(p.register(f, n).is_err());
                }
                assert!(p.register(0, 0).is_err());
                assert!(p.deregister(f, v).is_err());
                assert!(p.deregister(f, v + 1).is_err());
                assert_eq!(p.entries[&(f, 0)].calls, 1);
            }
            ARC_NATIVE_PORTS.with(|p| {
                let mut p = p.borrow_mut();
                assert!(p.do_close(e).is_err());
                assert_eq!(p.scopes.last().unwrap().calls, 1);
            });
            drop(g);
            dartforge_arc_retirar_retorno_owned_v1(f as i64, v);
            let new_area = register(f, 0);
            assert_ne!(new_area, v);
            {
                let mut p = arc_port_records().lock().unwrap();
                assert!(p.deregister(f, v).is_err());
                assert_eq!(p.entries[&(f, 0)].version, new_area);
            }
            dartforge_arc_retirar_retorno_owned_v1(f as i64, new_area);
            let internal = dartforge_arc_escopo_nativo_abrir_v1();
            ARC_NATIVE_PORTS.with(|p| assert!(p.borrow_mut().do_close(e).is_err()));
            dartforge_arc_escopo_nativo_fechar_v1(internal);
            dartforge_arc_escopo_nativo_fechar_v1(e);
        })
        .join()
        .unwrap();
    }

    extern "C" fn fail() -> i64 {
        dartforge_exception_throw(42, 1);
        0
    }

    #[test]
    fn pending_exit_neither_publishes_result_nor_clears_exception() {
        let _serial = SERIAL.lock().unwrap();
        std::thread::spawn(|| {
            dartforge_memoria_arc_v1();
            let f = fail as *const () as usize;
            let v = register(f, 0);
            let e = dartforge_arc_escopo_nativo_abrir_v1();
            assert_eq!(dart_r0(f), 0);
            assert_ne!(dartforge_exception_pending(), 0);
            ARC_NATIVE_PORTS
                .with(|p| assert!(p.borrow().scopes.last().unwrap().owners.is_empty()));
            dartforge_arc_escopo_nativo_fechar_v1(e);
            assert_ne!(dartforge_exception_pending(), 0);
            dartforge_exception_clear();
            dartforge_arc_retirar_retorno_owned_v1(f as i64, v);
        })
        .join()
        .unwrap();
    }

    extern "C" fn nsm_owned(_: i64, _: i64, _: i64, _: i64, _: i64, _: i64, _: i64) -> i64 {
        new()
    }

    extern "C" fn uniform_reentrant(_: i64, _: *const i64, _: *const i64) -> i64 {
        let x = dart_r0(new as *const () as usize);
        dartforge_arc_retain(x);
        x
    }

    extern "C" fn uniform_pending(_: i64, _: *const i64, _: *const i64) -> i64 {
        let _x = dart_r0(new as *const () as usize);
        fail()
    }

    fn register_uniform(f: usize) -> i64 {
        // SAFETY: fixtures não leem/publicam vetores; retornam Owned ou pending.
        unsafe { dartforge_arc_registrar_uniforme_owned_v1(f as i64) }
    }

    #[test]
    fn uniform_closes_callbacks_without_consuming_return_token() {
        let _serial = SERIAL.lock().unwrap();
        std::thread::spawn(|| {
            dartforge_memoria_arc_v1();
            let f = uniform_reentrant as *const () as usize;
            let n = new as *const () as usize;
            let v = register_uniform(f);
            let vn = register(n, 0);
            let x = arc_invoke_uniform(f, 0, 0, 0).unwrap();
            ARC_NATIVE_PORTS.with(|p| assert!(p.borrow().scopes.is_empty()));
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(x), 3);
            dartforge_arc_release(x);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(x), 0);
            dartforge_arc_retirar_retorno_owned_v1(f as i64, v);
            dartforge_arc_retirar_retorno_owned_v1(n as i64, vn);
        }).join().unwrap();
    }

    #[test]
    fn uniform_pending_closes_internal_owners_and_preserves_exception() {
        let _serial = SERIAL.lock().unwrap();
        std::thread::spawn(|| {
            dartforge_memoria_arc_v1();
            let f = uniform_pending as *const () as usize;
            let n = new as *const () as usize;
            let v = register_uniform(f);
            let vn = register(n, 0);
            assert_eq!(arc_invoke_uniform(f, 0, 0, 0), Ok(0));
            assert_ne!(dartforge_exception_pending(), 0);
            ARC_NATIVE_PORTS.with(|p| assert!(p.borrow().scopes.is_empty()));
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(ULTIMO.with(|u| u.get())), 0);
            assert_ne!(dartforge_exception_pending(), 0);
            dartforge_exception_clear();
            dartforge_arc_retirar_retorno_owned_v1(f as i64, v);
            dartforge_arc_retirar_retorno_owned_v1(n as i64, vn);
        }).join().unwrap();
    }

    #[test]
    fn uniform_does_not_infer_generic_callback_contract_and_rejects_upgrade() {
        let _serial = SERIAL.lock().unwrap();
        std::thread::spawn(|| {
            let f = tres as *const () as usize;
            assert!(arc_invoke_uniform(f, 0, 0, 0).is_err());
            let v = register(f, 3);
            assert!(arc_invoke_uniform(f, 0, 0, 0).is_err());
            {
                let mut p = arc_port_records().lock().unwrap();
                assert!(p.register_contract(f, 0, true).is_err());
                assert!(p.register_contract(f, 3, true).is_err());
                assert_eq!(p.entries[&(f, 3)].version, v);
                assert_eq!(p.entries[&(f, 3)].calls, 0);
            }
            ARC_NATIVE_PORTS.with(|p| assert!(p.borrow().scopes.is_empty()));
            dartforge_arc_retirar_retorno_owned_v1(f as i64, v);
            let f = uniform_reentrant as *const () as usize;
            let v = register_uniform(f);
            assert_eq!(register_uniform(f), v);
            assert!(arc_port_records().lock().unwrap().register(f, 3).is_err());
            dartforge_arc_retirar_retorno_owned_v1(f as i64, v);
            assert!(arc_invoke_uniform(f, 0, 0, 0).is_err());
        }).join().unwrap();
    }

    #[test]
    fn uniform_adapts_native_fallback_to_single_owned() {
        let _serial = SERIAL.lock().unwrap();
        std::thread::spawn(|| {
            dartforge_memoria_arc_v1();
            let f = nsm_owned as *const () as usize;
            let v = register(f, 7);
            let name = b"_dartforgeNoSuchMethod";
            // SAFETY: helper estático de sete palavras com retorno Owned.
            unsafe { dartforge_registrar_ajudante(name.as_ptr(), name.len() as i64, f); }
            let recv = new();
            let args = [0_i64]; let desc = [0_i64, 0];
            let x = arc_invoke_uniform(dartforge_nsm_seletor as *const () as usize,
                recv, args.as_ptr() as i64, desc.as_ptr() as i64).unwrap();
            ARC_NATIVE_PORTS.with(|p| assert!(p.borrow().scopes.is_empty()));
            assert_eq!(args, [0]); assert_eq!(desc, [0, 0]);
            dartforge_arc_release(recv);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(x), 3);
            dartforge_arc_release(x);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(x), 0);
            dartforge_arc_retirar_retorno_owned_v1(f as i64, v);
        }).join().unwrap();
    }

    #[test]
    fn owned_no_such_method_survives_legacy_frame_pop_without_leaking() {
        let _serial = SERIAL.lock().unwrap();
        std::thread::spawn(|| {
            dartforge_memoria_arc_v1();
            let f = nsm_owned as *const () as usize;
            let v = register(f, 7);
            let name = b"_dartforgeNoSuchMethod";
            // SAFETY: nome estático e fixture de palavras com assinatura do helper.
            unsafe {
                dartforge_registrar_ajudante(name.as_ptr(), name.len() as i64, f);
            }
            let e = dartforge_arc_escopo_nativo_abrir_v1();
            let recv = dartforge_arc_box_int_owned_v1(i64::MAX - 1);
            let name = HEAP.with(|h| h.borrow_mut().allocate_str("ausente"));
            let x = invocar_no_such_method(recv, 0, name, &[], 0, &[], 0).unwrap();
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(x), 3);
            assert_eq!(dartforge_unbox_int(x), i64::MAX);
            dartforge_arc_escopo_nativo_fechar_v1(e);
            dartforge_arc_release(recv);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(x), 0);
            dartforge_arc_retirar_retorno_owned_v1(f as i64, v);
        })
        .join()
        .unwrap();
    }
    #[test]
    fn image_registration_is_global_but_owners_and_scopes_are_per_thread() {
        let _serial = SERIAL.lock().unwrap();
        let f = new as *const () as usize;
        let v = register(f, 0);
        let parent_node = dartforge_arc_escopo_nativo_abrir_v1();
        std::thread::spawn(move || {
            dartforge_memoria_arc_v1();
            let e = dartforge_arc_escopo_nativo_abrir_v1();
            assert_ne!(e, parent_node);
            ARC_NATIVE_PORTS.with(|p| assert!(p.borrow_mut().do_close(parent_node).is_err()));
            let x = dart_r0(f);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(x), 3);
            let g = arc_prepare_port(f, 0).unwrap();
            // A versão também fica presa enquanto uma chamada de outra thread
            // está em curso; o registro não depende do heap do publicador.
            assert!(
                arc_port_records()
                    .lock()
                    .unwrap()
                    .deregister(f, v)
                    .is_err()
            );
            drop(g);
            dartforge_arc_escopo_nativo_fechar_v1(e);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(x), 0);
        })
        .join()
        .unwrap();
        dartforge_arc_escopo_nativo_fechar_v1(parent_node);
        dartforge_arc_retirar_retorno_owned_v1(f as i64, v);
    }
    #[test]
    fn owned_return_ignored_by_void_port_is_released_immediately() {
        let _serial = SERIAL.lock().unwrap();
        std::thread::spawn(|| {
            dartforge_memoria_arc_v1();
            let fs = [
                (new as *const () as usize, 0),
                (one as *const () as usize, 1),
                (dois as *const () as usize, 2),
                (tres as *const () as usize, 3),
            ];
            let entries: Vec<_> = fs.iter().map(|&(f, n)| (f, register(f, n))).collect();
            let e = dartforge_arc_escopo_nativo_abrir_v1();
            dart_v0(fs[0].0);
            let y = ULTIMO.with(|u| u.get());
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(y), 0);
            let x = new();
            dart_v1(fs[1].0, x);
            dart_v2(fs[2].0, x, 0);
            dart_v3(fs[3].0, x, 0, 0);
            ARC_NATIVE_PORTS
                .with(|p| assert!(p.borrow().scopes.last().unwrap().owners.is_empty()));
            dartforge_arc_release(x);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(x), 0);
            dartforge_arc_escopo_nativo_fechar_v1(e);
            for (f, v) in entries {
                dartforge_arc_retirar_retorno_owned_v1(f as i64, v);
            }
        })
        .join()
        .unwrap();
    }
}
