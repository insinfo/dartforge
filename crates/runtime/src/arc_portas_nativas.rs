// Fronteira entre callbacks Dart Owned e consumidores nativos emprestados.
// O registro é explícito: endereços e palavras não declaram ownership.

#[derive(Clone, Copy)]
struct RegistroRetornoArc {
    versao: i64,
    chamadas: usize,
    uniforme: bool,
}

struct EscopoRetornosArc {
    id: i64,
    chamadas: usize,
    owners: Vec<i64>,
}

#[derive(Default)]
struct PortasNativasArc {
    entradas: HashMap<(usize, usize), RegistroRetornoArc>,
}

impl PortasNativasArc {
    fn registrar(&mut self, codigo: usize, aridade: usize) -> Result<i64, &'static str> {
        self.registrar_contrato(codigo, aridade, false)
    }

    fn registrar_contrato(
        &mut self,
        codigo: usize,
        aridade: usize,
        uniforme: bool,
    ) -> Result<i64, &'static str> {
        if codigo == 0 || !matches!(aridade, 0..=5 | 7) || (uniforme && aridade != 3) {
            return Err("ABI de callback não coberta");
        }
        // O ligador pode fundir código de funções com assinaturas distintas.
        // A declaração pertence à ABI lógica (endereço, aridade), não só ao PC.
        if let Some(r) = self.entradas.get(&(codigo, aridade)) {
            if r.uniforme != uniforme {
                return Err("contrato uniforme conflita com callback de palavras");
            }
            return Ok(r.versao);
        }
        let versao = novo_id_porta_arc();
        self.entradas.insert(
            (codigo, aridade),
            RegistroRetornoArc {
                versao,
                chamadas: 0,
                uniforme,
            },
        );
        Ok(versao)
    }

    fn retirar(&mut self, codigo: usize, versao: i64) -> Result<(), &'static str> {
        let chave = self
            .entradas
            .iter()
            .find_map(|(k, r)| (k.0 == codigo && r.versao == versao).then_some(*k))
            .ok_or("callback não registrado ou versão obsoleta")?;
        if self.entradas[&chave].chamadas != 0 {
            return Err("callback ativo");
        }
        self.entradas.remove(&chave);
        Ok(())
    }
}

#[derive(Default)]
struct EscoposPortasArc {
    escopos: Vec<EscopoRetornosArc>,
}

impl EscoposPortasArc {
    fn fechar(&mut self, id: i64) -> Result<Vec<i64>, &'static str> {
        let e = self.escopos.last().ok_or("escopo nativo ausente")?;
        if e.id != id || e.chamadas != 0 {
            return Err("escopo fora do topo ou callback ativo");
        }
        Ok(self.escopos.pop().unwrap().owners)
    }
}

fn novo_id_porta_arc() -> i64 {
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
fn registros_portas_arc() -> &'static std::sync::Mutex<PortasNativasArc> {
    static REGISTROS: std::sync::OnceLock<std::sync::Mutex<PortasNativasArc>> =
        std::sync::OnceLock::new();
    REGISTROS.get_or_init(|| std::sync::Mutex::new(PortasNativasArc::default()))
}

static HA_PORTAS_RETORNO_ARC: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

thread_local! {
    // Os valores gerenciados pertencem ao heap da thread que recebeu o retorno.
    static PORTAS_NATIVAS_ARC: RefCell<EscoposPortasArc> = RefCell::new(EscoposPortasArc::default());
}

/// Declara retorno Ref Owned de um callback de palavras e devolve sua versão.
///
/// Registro repetido com a mesma ABI é idempotente. Não chama código, coleta
/// ou executa Dart. Não seleciona o modo de memória nem prova o corpo Dart.
///
/// # Safety
/// `codigo` deve ter a ABI da porta indicada por `aridade`, com parâmetros
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
/// extern "C" fn nulo() -> i64 { 0 }
/// // SAFETY: null é Ref sem token físico; função estática de aridade zero.
/// let v = unsafe { dartforge_arc_registrar_retorno_owned_v1(nulo as usize as i64, 0) };
/// dartforge_arc_retirar_retorno_owned_v1(nulo as usize as i64, v);
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arc_registrar_retorno_owned_v1(
    codigo: i64,
    aridade: i64,
) -> i64 {
    let codigo = usize::try_from(codigo as u64).expect("endereço de callback inválido");
    let aridade = usize::try_from(aridade).expect("aridade negativa");
    let mut p = registros_portas_arc()
        .lock()
        .expect("registro de portas ARC envenenado");
    let versao = p
        .registrar(codigo, aridade)
        .expect("contrato de callback ARC violado");
    HA_PORTAS_RETORNO_ARC.store(true, std::sync::atomic::Ordering::Release);
    versao
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
/// // SAFETY: corpo estático que não publica os vetores, retorna Smi sem token.
/// let v = unsafe { dartforge_arc_registrar_uniforme_owned_v1(smi as *const () as usize as i64) };
/// dartforge_arc_retirar_retorno_owned_v1(smi as *const () as usize as i64, v);
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arc_registrar_uniforme_owned_v1(codigo: i64) -> i64 {
    let codigo = usize::try_from(codigo as u64).expect("endereço uniforme inválido");
    let mut p = registros_portas_arc().lock().expect("registro de portas ARC envenenado");
    let versao = p.registrar_contrato(codigo, 3, true).expect("contrato uniforme ARC violado");
    HA_PORTAS_RETORNO_ARC.store(true, std::sync::atomic::Ordering::Release);
    versao
}

struct EscopoUniformeArc(i64);

impl Drop for EscopoUniformeArc {
    fn drop(&mut self) {
        dartforge_arc_escopo_nativo_fechar_v1(self.0);
    }
}

fn arc_invocar_uniforme(
    codigo: usize,
    recv: i64,
    args: i64,
    desc: i64,
) -> Result<i64, &'static str> {
    let escopo = EscopoUniformeArc(dartforge_arc_escopo_nativo_abrir_v1());
    let resultado = if codigo == dartforge_nsm_seletor as *const () as usize {
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
        let guarda = arc_preparar_porta_contrato(codigo, 3, true)?
            .ok_or("entrada uniforme não registrada")?;
        let r = dart_r3_invocar(codigo, recv, args, desc);
        arc_conferir_escopo_porta(&guarda);
        arc_validar_retorno_owned(r);
        // O retorno direto já é Owned: não o transfere ao escopo nem retém.
        // A guarda precisa terminar antes do fechamento do escopo que prende.
        drop(guarda);
        r
    };
    drop(escopo);
    Ok(resultado)
}

/// Invoca entrada uniforme certificada e devolve exatamente um retorno Owned.
///
/// Aceita o fallback nativo noSuchMethod, adaptando seu retorno emprestado.
/// Fecha o escopo de callbacks também em pending, sem limpar a exceção.
/// Código desconhecido e callback genérico não são inferidos como uniformes.
///
/// # Safety
/// `codigo` e sua imagem permanecem vivos durante a chamada. `args` e `desc`
/// são endereços válidos da ABI uniforme: dois contadores não negativos,
/// hashes nomeados e vetor com todos os argumentos mais a tupla RTI.
/// Receptor e elementos Ref permanecem vivos por todo o alcance da chamada.
///
/// # Panics
/// Entrada não certificada ou violação de ownership/escopos; aborta no C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// extern "C" fn nulo(_: i64, _: *const i64, _: *const i64) -> i64 { 0 }
/// let pc = nulo as *const () as usize as i64;
/// // SAFETY: corpo não publica vetores nem cria tokens, devolve Ref null.
/// let v = unsafe { dartforge_arc_registrar_uniforme_owned_v1(pc) };
/// let args = [0_i64]; let desc = [0_i64, 0];
/// // SAFETY: vetores válidos e corpo registrado estático.
/// assert_eq!(unsafe { dartforge_arc_invocar_uniforme_v1(pc, 0,
///     args.as_ptr() as i64, desc.as_ptr() as i64) }, 0);
/// dartforge_arc_retirar_retorno_owned_v1(pc, v);
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arc_invocar_uniforme_v1(
    codigo: i64,
    recv: i64,
    args: i64,
    desc: i64,
) -> i64 {
    let codigo = usize::try_from(codigo as u64).expect("endereço uniforme inválido");
    arc_invocar_uniforme(codigo, recv, args, desc).expect("fronteira uniforme ARC inválida")
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
/// extern "C" fn nulo(_: i64) -> i64 { 0 }
/// // SAFETY: ABI estática de palavras com retorno Ref null.
/// let v = unsafe { dartforge_arc_registrar_retorno_owned_v1(nulo as usize as i64, 1) };
/// dartforge_arc_retirar_retorno_owned_v1(nulo as usize as i64, v);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_retirar_retorno_owned_v1(codigo: i64, versao: i64) {
    let codigo = usize::try_from(codigo as u64).expect("endereço de callback inválido");
    let mut p = registros_portas_arc()
        .lock()
        .expect("registro de portas ARC envenenado");
    p.retirar(codigo, versao)
        .expect("retirada de callback ARC inválida");
    HA_PORTAS_RETORNO_ARC.store(!p.entradas.is_empty(), std::sync::atomic::Ordering::Release);
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
    PORTAS_NATIVAS_ARC.with(|p| {
        let mut p = p.borrow_mut();
        let id = novo_id_porta_arc();
        p.escopos.push(EscopoRetornosArc {
            id,
            chamadas: 0,
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
    let owners = PORTAS_NATIVAS_ARC.with(|p| {
        p.borrow_mut()
            .fechar(id)
            .expect("fechamento de escopo nativo ARC inválido")
    });
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        for owner in owners {
            h.soltar_owner_codigo(owner);
        }
    });
}

struct GuardaPortaArc {
    codigo: usize,
    aridade: usize,
    versao: i64,
    escopo: i64,
}

fn arc_preparar_porta(
    codigo: usize,
    aridade: usize,
) -> Result<Option<GuardaPortaArc>, &'static str> {
    arc_preparar_porta_contrato(codigo, aridade, false)
}

fn arc_preparar_porta_contrato(
    codigo: usize,
    aridade: usize,
    uniforme: bool,
) -> Result<Option<GuardaPortaArc>, &'static str> {
    // A convenção antiga não adquire lock nem cria escopo quando nenhuma
    // imagem declarou callbacks Owned. Isto não seleciona ARC implicitamente.
    if !HA_PORTAS_RETORNO_ARC.load(std::sync::atomic::Ordering::Acquire) {
        return Ok(None);
    }
    let mut p = registros_portas_arc()
        .lock()
        .expect("registro de portas ARC envenenado");
    let Some(r) = p.entradas.get(&(codigo, aridade)).copied() else {
        return if p.entradas.keys().any(|k| k.0 == codigo) {
            Err("assinatura do callback ARC não registrada")
        } else {
            Ok(None)
        };
    };
    if uniforme && !r.uniforme {
        return Err("entrada sem contrato uniforme");
    }
    let ativas = r
        .chamadas
        .checked_add(1)
        .ok_or("callbacks ativos esgotados")?;
    let escopo = PORTAS_NATIVAS_ARC.with(|s| {
        let mut s = s.borrow_mut();
        let e = s
            .escopos
            .last_mut()
            .ok_or("callback Owned exige escopo nativo")?;
        e.chamadas = e
            .chamadas
            .checked_add(1)
            .ok_or("callbacks ativos esgotados")?;
        Ok::<_, &'static str>(e.id)
    })?;
    p.entradas.get_mut(&(codigo, aridade)).unwrap().chamadas = ativas;
    Ok(Some(GuardaPortaArc {
        codigo,
        aridade,
        versao: r.versao,
        escopo,
    }))
}

impl Drop for GuardaPortaArc {
    fn drop(&mut self) {
        let mut p = registros_portas_arc()
            .lock()
            .expect("registro de portas ARC envenenado");
        let r = p
            .entradas
            .get_mut(&(self.codigo, self.aridade))
            .expect("callback ativo retirado");
        assert_eq!(r.versao, self.versao, "callback ativo substituído");
        r.chamadas -= 1;
        PORTAS_NATIVAS_ARC.with(|s| {
            s.borrow_mut()
                .escopos
                .iter_mut()
                .find(|e| e.id == self.escopo)
                .expect("escopo do callback fechado")
                .chamadas -= 1
        });
    }
}

fn arc_validar_retorno_owned(resultado: i64) -> bool {
    if dartforge_exception_pending() != 0 {
        assert_eq!(
            resultado, 0,
            "callback Owned deve devolver null na saída pending"
        );
        return false;
    }
    if smi::e_handle(resultado) {
        assert_ne!(
            HEAP.with(|h| h.borrow().observar_owner_codigo(resultado)) & 2,
            0,
            "callback Owned devolveu referência sem token"
        );
    }
    true
}

fn arc_concluir_porta(guarda: Option<GuardaPortaArc>, resultado: i64) -> i64 {
    if let Some(g) = &guarda {
        arc_conferir_escopo_porta(g);
        if !arc_validar_retorno_owned(resultado) {
            return resultado;
        }
        PORTAS_NATIVAS_ARC.with(|p| {
            let mut p = p.borrow_mut();
            let e = p.escopos.last_mut().expect("escopo do callback ausente");
            assert_eq!(e.id, g.escopo, "escopo reentrante não foi fechado");
            e.owners.push(resultado);
        });
    }
    resultado
}

fn arc_conferir_escopo_porta(guarda: &GuardaPortaArc) {
    PORTAS_NATIVAS_ARC.with(|p| {
        assert_eq!(
            p.borrow().escopos.last().map(|e| e.id),
            Some(guarda.escopo),
            "escopo reentrante não foi fechado"
        )
    });
}

fn arc_descartar_porta(guarda: GuardaPortaArc, resultado: i64) {
    arc_conferir_escopo_porta(&guarda);
    // A guarda continua prendendo a versão e o escopo durante a liberação.
    if arc_validar_retorno_owned(resultado) {
        HEAP.with(|h| h.borrow_mut().soltar_owner_codigo(resultado));
    }
}

#[cfg(test)]
mod testes_portas_arc {
    use super::*;
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

    thread_local! { static ULTIMO: std::cell::Cell<i64> = const { std::cell::Cell::new(0) }; }
    extern "C" fn novo() -> i64 {
        let x = dartforge_arc_box_int_owned_v1(i64::MAX);
        ULTIMO.with(|u| u.set(x));
        x
    }
    extern "C" fn um(x: i64) -> i64 {
        dartforge_arc_retain(x);
        x
    }
    extern "C" fn dois(x: i64, _: i64) -> i64 {
        um(x)
    }
    extern "C" fn tres(x: i64, _: i64, _: i64) -> i64 {
        um(x)
    }
    extern "C" fn quatro(x: i64, _: i64, _: i64, _: i64) -> i64 {
        um(x)
    }
    extern "C" fn cinco(x: i64, _: i64, _: i64, _: i64, _: i64) -> i64 {
        um(x)
    }
    extern "C" fn sete(x: i64, _: i64, _: i64, _: i64, _: i64, _: i64, _: i64) -> i64 {
        um(x)
    }
    extern "C" fn escalar() -> i64 {
        i64::MAX - 1
    }

    fn registrar(f: usize, n: i64) -> i64 {
        // SAFETY: somente as fixtures de palavras/Ref Owned acima são usadas.
        unsafe { dartforge_arc_registrar_retorno_owned_v1(f as i64, n) }
    }

    #[test]
    fn todas_as_portas_transferem_um_token_sem_reter_duas_vezes() {
        let _serial = SERIAL.lock().unwrap();
        std::thread::spawn(|| {
            dartforge_memoria_arc_v1();
            let fs = [
                (novo as *const () as usize, 0),
                (um as *const () as usize, 1),
                (dois as *const () as usize, 2),
                (tres as *const () as usize, 3),
                (quatro as *const () as usize, 4),
                (cinco as *const () as usize, 5),
                (sete as *const () as usize, 7),
            ];
            let entradas: Vec<_> = fs.iter().map(|&(f, n)| (f, registrar(f, n))).collect();
            let e = dartforge_arc_escopo_nativo_abrir_v1();
            let x = novo();
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
            for (f, v) in entradas {
                dartforge_arc_retirar_retorno_owned_v1(f as i64, v);
            }
            // Sem registro explícito, uma palavra não ganha owner nem escopo.
            assert_eq!(dart_r0(escalar as *const () as usize), i64::MAX - 1);
        })
        .join()
        .unwrap();
    }

    extern "C" fn reentrante() -> i64 {
        let e = dartforge_arc_escopo_nativo_abrir_v1();
        let x = dart_r0(novo as *const () as usize);
        dartforge_arc_retain(x);
        dartforge_arc_escopo_nativo_fechar_v1(e);
        dartforge_arc_collect();
        x
    }

    #[test]
    fn escopos_reentrantes_e_quadros_observacionais_nao_se_confundem() {
        let _serial = SERIAL.lock().unwrap();
        std::thread::spawn(|| {
            dartforge_memoria_arc_v1();
            let f = reentrante as *const () as usize;
            let n = novo as *const () as usize;
            let vf = registrar(f, 0);
            let vn = registrar(n, 0);
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
    fn registros_e_escopos_recusam_versao_obsoleta_abi_errada_e_mutacao_ativa() {
        let _serial = SERIAL.lock().unwrap();
        std::thread::spawn(|| {
            let f = novo as *const () as usize;
            let v = registrar(f, 0);
            assert_eq!(registrar(f, 0), v);
            assert!(arc_preparar_porta(f, 0).is_err());
            let e = dartforge_arc_escopo_nativo_abrir_v1();
            assert!(arc_preparar_porta(f, 1).is_err());
            let g = arc_preparar_porta(f, 0).unwrap();
            {
                let mut p = registros_portas_arc().lock().unwrap();
                for n in [6, 8] {
                    assert!(p.registrar(f, n).is_err());
                }
                assert!(p.registrar(0, 0).is_err());
                assert!(p.retirar(f, v).is_err());
                assert!(p.retirar(f, v + 1).is_err());
                assert_eq!(p.entradas[&(f, 0)].chamadas, 1);
            }
            PORTAS_NATIVAS_ARC.with(|p| {
                let mut p = p.borrow_mut();
                assert!(p.fechar(e).is_err());
                assert_eq!(p.escopos.last().unwrap().chamadas, 1);
            });
            drop(g);
            dartforge_arc_retirar_retorno_owned_v1(f as i64, v);
            let nova = registrar(f, 0);
            assert_ne!(nova, v);
            {
                let mut p = registros_portas_arc().lock().unwrap();
                assert!(p.retirar(f, v).is_err());
                assert_eq!(p.entradas[&(f, 0)].versao, nova);
            }
            dartforge_arc_retirar_retorno_owned_v1(f as i64, nova);
            let interno = dartforge_arc_escopo_nativo_abrir_v1();
            PORTAS_NATIVAS_ARC.with(|p| assert!(p.borrow_mut().fechar(e).is_err()));
            dartforge_arc_escopo_nativo_fechar_v1(interno);
            dartforge_arc_escopo_nativo_fechar_v1(e);
        })
        .join()
        .unwrap();
    }

    extern "C" fn falhar() -> i64 {
        dartforge_exception_throw(42, 1);
        0
    }

    #[test]
    fn saida_pending_nao_publica_resultado_nem_limpa_excecao() {
        let _serial = SERIAL.lock().unwrap();
        std::thread::spawn(|| {
            dartforge_memoria_arc_v1();
            let f = falhar as *const () as usize;
            let v = registrar(f, 0);
            let e = dartforge_arc_escopo_nativo_abrir_v1();
            assert_eq!(dart_r0(f), 0);
            assert_ne!(dartforge_exception_pending(), 0);
            PORTAS_NATIVAS_ARC
                .with(|p| assert!(p.borrow().escopos.last().unwrap().owners.is_empty()));
            dartforge_arc_escopo_nativo_fechar_v1(e);
            assert_ne!(dartforge_exception_pending(), 0);
            dartforge_exception_clear();
            dartforge_arc_retirar_retorno_owned_v1(f as i64, v);
        })
        .join()
        .unwrap();
    }

    extern "C" fn nsm_owned(_: i64, _: i64, _: i64, _: i64, _: i64, _: i64, _: i64) -> i64 {
        novo()
    }

    extern "C" fn uniforme_reentrante(_: i64, _: *const i64, _: *const i64) -> i64 {
        let x = dart_r0(novo as *const () as usize);
        dartforge_arc_retain(x);
        x
    }

    extern "C" fn uniforme_pending(_: i64, _: *const i64, _: *const i64) -> i64 {
        let _x = dart_r0(novo as *const () as usize);
        falhar()
    }

    fn registrar_uniforme(f: usize) -> i64 {
        // SAFETY: fixtures não leem/publicam vetores; retornam Owned ou pending.
        unsafe { dartforge_arc_registrar_uniforme_owned_v1(f as i64) }
    }

    #[test]
    fn uniforme_fecha_callbacks_sem_consumir_o_token_de_retorno() {
        let _serial = SERIAL.lock().unwrap();
        std::thread::spawn(|| {
            dartforge_memoria_arc_v1();
            let f = uniforme_reentrante as *const () as usize;
            let n = novo as *const () as usize;
            let v = registrar_uniforme(f);
            let vn = registrar(n, 0);
            let x = arc_invocar_uniforme(f, 0, 0, 0).unwrap();
            PORTAS_NATIVAS_ARC.with(|p| assert!(p.borrow().escopos.is_empty()));
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
    fn uniforme_pending_fecha_owners_internos_e_preserva_excecao() {
        let _serial = SERIAL.lock().unwrap();
        std::thread::spawn(|| {
            dartforge_memoria_arc_v1();
            let f = uniforme_pending as *const () as usize;
            let n = novo as *const () as usize;
            let v = registrar_uniforme(f);
            let vn = registrar(n, 0);
            assert_eq!(arc_invocar_uniforme(f, 0, 0, 0), Ok(0));
            assert_ne!(dartforge_exception_pending(), 0);
            PORTAS_NATIVAS_ARC.with(|p| assert!(p.borrow().escopos.is_empty()));
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(ULTIMO.with(|u| u.get())), 0);
            assert_ne!(dartforge_exception_pending(), 0);
            dartforge_exception_clear();
            dartforge_arc_retirar_retorno_owned_v1(f as i64, v);
            dartforge_arc_retirar_retorno_owned_v1(n as i64, vn);
        }).join().unwrap();
    }

    #[test]
    fn uniforme_nao_infere_contrato_de_callback_generico_e_recusa_upgrade() {
        let _serial = SERIAL.lock().unwrap();
        std::thread::spawn(|| {
            let f = tres as *const () as usize;
            assert!(arc_invocar_uniforme(f, 0, 0, 0).is_err());
            let v = registrar(f, 3);
            assert!(arc_invocar_uniforme(f, 0, 0, 0).is_err());
            {
                let mut p = registros_portas_arc().lock().unwrap();
                assert!(p.registrar_contrato(f, 0, true).is_err());
                assert!(p.registrar_contrato(f, 3, true).is_err());
                assert_eq!(p.entradas[&(f, 3)].versao, v);
                assert_eq!(p.entradas[&(f, 3)].chamadas, 0);
            }
            PORTAS_NATIVAS_ARC.with(|p| assert!(p.borrow().escopos.is_empty()));
            dartforge_arc_retirar_retorno_owned_v1(f as i64, v);
            let f = uniforme_reentrante as *const () as usize;
            let v = registrar_uniforme(f);
            assert_eq!(registrar_uniforme(f), v);
            assert!(registros_portas_arc().lock().unwrap().registrar(f, 3).is_err());
            dartforge_arc_retirar_retorno_owned_v1(f as i64, v);
            assert!(arc_invocar_uniforme(f, 0, 0, 0).is_err());
        }).join().unwrap();
    }

    #[test]
    fn uniforme_adapta_fallback_nativo_para_um_unico_owned() {
        let _serial = SERIAL.lock().unwrap();
        std::thread::spawn(|| {
            dartforge_memoria_arc_v1();
            let f = nsm_owned as *const () as usize;
            let v = registrar(f, 7);
            let nome = b"_dartforgeNoSuchMethod";
            // SAFETY: helper estático de sete palavras com retorno Owned.
            unsafe { dartforge_registrar_ajudante(nome.as_ptr(), nome.len() as i64, f); }
            let recv = novo();
            let args = [0_i64]; let desc = [0_i64, 0];
            let x = arc_invocar_uniforme(dartforge_nsm_seletor as *const () as usize,
                recv, args.as_ptr() as i64, desc.as_ptr() as i64).unwrap();
            PORTAS_NATIVAS_ARC.with(|p| assert!(p.borrow().escopos.is_empty()));
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
    fn no_such_method_owned_sobrevive_ao_pop_do_quadro_legacy_sem_vazar() {
        let _serial = SERIAL.lock().unwrap();
        std::thread::spawn(|| {
            dartforge_memoria_arc_v1();
            let f = nsm_owned as *const () as usize;
            let v = registrar(f, 7);
            let nome = b"_dartforgeNoSuchMethod";
            // SAFETY: nome estático e fixture de palavras com assinatura do helper.
            unsafe {
                dartforge_registrar_ajudante(nome.as_ptr(), nome.len() as i64, f);
            }
            let e = dartforge_arc_escopo_nativo_abrir_v1();
            let recv = dartforge_arc_box_int_owned_v1(i64::MAX - 1);
            let nome = HEAP.with(|h| h.borrow_mut().alocar_str("ausente"));
            let x = invocar_no_such_method(recv, 0, nome, &[], 0, &[], 0).unwrap();
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
    fn registro_da_imagem_e_global_mas_owners_e_escopos_sao_da_thread() {
        let _serial = SERIAL.lock().unwrap();
        let f = novo as *const () as usize;
        let v = registrar(f, 0);
        let pai = dartforge_arc_escopo_nativo_abrir_v1();
        std::thread::spawn(move || {
            dartforge_memoria_arc_v1();
            let e = dartforge_arc_escopo_nativo_abrir_v1();
            assert_ne!(e, pai);
            PORTAS_NATIVAS_ARC.with(|p| assert!(p.borrow_mut().fechar(pai).is_err()));
            let x = dart_r0(f);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(x), 3);
            let g = arc_preparar_porta(f, 0).unwrap();
            // A versão também fica presa enquanto uma chamada de outra thread
            // está em curso; o registro não depende do heap do publicador.
            assert!(
                registros_portas_arc()
                    .lock()
                    .unwrap()
                    .retirar(f, v)
                    .is_err()
            );
            drop(g);
            dartforge_arc_escopo_nativo_fechar_v1(e);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(x), 0);
        })
        .join()
        .unwrap();
        dartforge_arc_escopo_nativo_fechar_v1(pai);
        dartforge_arc_retirar_retorno_owned_v1(f as i64, v);
    }
    #[test]
    fn retorno_owned_ignorado_pela_porta_void_e_liberado_imediatamente() {
        let _serial = SERIAL.lock().unwrap();
        std::thread::spawn(|| {
            dartforge_memoria_arc_v1();
            let fs = [
                (novo as *const () as usize, 0),
                (um as *const () as usize, 1),
                (dois as *const () as usize, 2),
                (tres as *const () as usize, 3),
            ];
            let entradas: Vec<_> = fs.iter().map(|&(f, n)| (f, registrar(f, n))).collect();
            let e = dartforge_arc_escopo_nativo_abrir_v1();
            dart_v0(fs[0].0);
            let y = ULTIMO.with(|u| u.get());
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(y), 0);
            let x = novo();
            dart_v1(fs[1].0, x);
            dart_v2(fs[2].0, x, 0);
            dart_v3(fs[3].0, x, 0, 0);
            PORTAS_NATIVAS_ARC
                .with(|p| assert!(p.borrow().escopos.last().unwrap().owners.is_empty()));
            dartforge_arc_release(x);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(x), 0);
            dartforge_arc_escopo_nativo_fechar_v1(e);
            for (f, v) in entradas {
                dartforge_arc_retirar_retorno_owned_v1(f as i64, v);
            }
        })
        .join()
        .unwrap();
    }
}
