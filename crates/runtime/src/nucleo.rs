// Runtime nativo, núcleo: entrada `main`, estado por thread (heap, nomes e
// subtipos de classe), objetos, igualdade, caixas e records. Os fragmentos
// `crates/runtime/src/*.rs` listados em `FRAGMENTOS` (`crates/runtime/build.rs`)
// são concatenados num único programa Rust; este é o primeiro.

// Harness standalone: handles gerenciados e ABI C com raízes explícitas.

// SAFETY: o emissor define esta entrada com assinatura C void(void).
//
// Só no executável AOT. Quando este arquivo é compilado como módulo do crate
// `dartforge-runtime` (cfg `dartforge_runtime_embutido`, posta pelo build.rs),
// quem chama a entrada é o JIT, e um `main` C colidiria com o do binário Rust.
#[cfg(not(any(dartforge_runtime_embutido, dartforge_runtime_dll)))]
unsafe extern "C" {
    fn dartforge_entry();
}

/// Invoca uma vez o programa ligado ao runtime Rust.
#[cfg(not(any(dartforge_runtime_embutido, dartforge_runtime_dll)))]
#[unsafe(no_mangle)]
pub extern "C" fn main() -> i32 {
    // SAFETY: o objeto foi emitido para esta ABI e ligado pelo mesmo driver nativo.
    unsafe { dartforge_entry() };
    let code = finalizar_programa();
    if code != 0 {
        std::process::exit(code);
    }
    0
}

/// A entrada do programa com o SDK da fonte (P5c): o runtime e o SDK moram
/// numa DLL (cfg `dartforge_runtime_dll`, sem o `main` C), e o `main` do
/// executável — que o emissor escreve — chama esta função com a
/// `dartforge_entry` dele.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_iniciar(input: extern "C" fn(), to_string_fn: extern "C" fn(i64) -> i64) -> i32 {
    PARA_TEXTO.with(|p| p.set(Some(to_string_fn)));
    // O isolado principal usa as listas livres do alocador (`alocador.rs`).
    enable_allocation_cache();
    if debug() {
        // Depuração: o pânico do runtime mostra a pilha de funções Dart
        // (`DARTFORGE_RASTRO=1` na compilação).
        let pattern = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |i| {
            show_trace();
            pattern(i);
        }));
    }
    // O isolado principal aceita os pedidos no ponto seguro (a recarga do
    // JIT) enquanto a entrada roda — o `main` e o laço de eventos.
    mark_main_isolate();
    input();
    unmark_main_isolate();
    let code = finalizar_programa();
    if code != 0 {
        std::process::exit(code);
    }
    0
}

/// O que acontece depois que `dartforge_entry` retorna, nos DOIS perfis.
///
/// Exceção pendente: escreve `Unhandled exception:`, a mensagem e o rastro em
/// stderr e devolve 255, como a VM.
/// Senão, com `DARTFORGE_GC_STATS=1`, escreve as estatísticas do coletor, e
/// devolve o código de saída global (`exitCode` do `dart:io`; 0 sem ele). Quem chama encerra o processo com o código (o `main` acima, no
/// AOT; o executor, no JIT). Não chama `exit` aqui: é o único trecho do
/// runtime que os dois perfis dirigem, e fica escrito uma vez só.
pub fn finalizar_programa() -> i32 {
    // O isolado principal terminou: os `NativeFinalizer` anexados rodam.
    shutdown_isolate_finalizers();
    // `Isolate.exit` no isolado principal: ele terminou, sem erro.
    let pending = EXCEPTION.with(|slot| slot.borrow().is_some()) && !unwinding_now();
    if pending {
        // O rastro antes do `toString()` do erro, que roda código Dart (um
        // `throw` tratado lá dentro trocaria o rastro guardado).
        let trace = exception_trace_text();
        let (bits, tag) = EXCEPTION.with(|slot| {
            let value = slot.borrow().expect("exceção verificada acima");
            (exception_bits(value), exception_tag(value))
        });
        HEAP.with(|heap| {
            let heap = heap.borrow();
            let detail = match tag {
                1 => format!("{bits}"),
                2 => format!("{}", bits != 0),
                _ => match PARA_TEXTO.with(|p| p.get()) {
                    // SDK da fonte: o `toString()` Dart do objeto lançado.
                    Some(f) => {
                        drop(heap);
                        take_exception();
                        let t = with_roots(&[bits], || dart_r1(f as usize, bits));
                        let s = HEAP.with(|h| h.borrow().text(t).map(|text| text.para_string()));
                        // O `toString()` também falhou: a descrição do
                        // runtime (a classe do objeto).
                        s.unwrap_or_else(|| {
                            take_exception();
                            HEAP.with(|h| describe_handle(&h.borrow(), bits))
                        })
                    }
                    None => describe_handle(&heap, bits),
                },
            };
            // O formato e o código da VM (`Unhandled exception:`, a mensagem,
            // o rastro; 255). O rastro é o que o runtime tem da exceção.
            use std::io::Write;
            let mut err = std::io::stderr().lock();
            let _ = writeln!(err, "Unhandled exception:\n{detail}");
            if !trace.is_empty() {
                let _ = write!(err, "{trace}");
                if !trace.ends_with('\n') {
                    let _ = writeln!(err);
                }
            }
        });
        return 255;
    }
    // `DARTFORGE_GC_STATS=1` escreve as estatísticas; `=require` também, e
    // devolve 70 se a execução não exercitou o coletor: nenhuma coleta, ou
    // uma imagem com raízes por mapas e nenhuma raiz lida de mapa
    // (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.5, a prova de que coletou:
    // um teste que nunca coletou no ponto perigoso não pode passar).
    let statistics = std::env::var("DARTFORGE_GC_STATS").unwrap_or_default();
    let mut nothing_exercised = false;
    if statistics == "1" || statistics == "exigir" {
        HEAP.with(|heap| {
            let s = heap.borrow().stats();
            let (frames, map_roots, checked) = crate::heap::walk_counts();
            eprintln!("{{\"dartforge_gc\":{{\"allocations\":{},\"collections\":{},\"reclaimed\":{},\"live_objects\":{},\"reserved_slots\":{},\"root_slots\":{},\"peak_root_slots\":{},\"live_roots\":{},\"peak_roots\":{},\"live_bytes\":{},\"peak_live_bytes\":{},\"permanent_roots\":{},\"smi_caixas_evitadas\":{},\"map_frames\":{},\"map_roots\":{},\"checked_roots\":{}}}}}",
                s.allocations, s.collections, s.reclaimed, s.live_objects, s.reserved_slots,
                s.root_slots, s.peak_root_slots, s.live_roots, s.peak_roots,
                s.estimated_bytes, s.peak_estimated_bytes, s.permanent_roots, s.boxes_avoided,
                frames, map_roots, checked);
            nothing_exercised = s.collections == 0 || (crate::heap::has_stack_maps() && map_roots == 0);
        });
    }
    if statistics == "exigir" && nothing_exercised {
        eprintln!("dartforge: DARTFORGE_GC_STATS=exigir e a execução não exercitou o coletor (nenhuma coleta, ou nenhuma raiz lida de mapa)");
        return 70;
    }
    if std::env::var("DARTFORGE_GC_MEMORIA").is_ok_and(|v| !v.is_empty() && v != "0") {
        HEAP.with(|heap| eprint!("[memória] fim do programa\n{}", heap.borrow().memory_report()));
    }
    global_exit_code()
}

use crate::heap::{CONTEXTO, Heap, Texto, StringMut};
use std::cell::RefCell;
use crate::hash::{HashMap, HashSet};

thread_local! {
    /// SDK da fonte: o `toString()` Dart de um valor (a
    /// `dartforge_dispatch_toString` do programa), para a exceção não
    /// capturada.
    static PARA_TEXTO: std::cell::Cell<Option<extern "C" fn(i64) -> i64>> = const { std::cell::Cell::new(None) };
    static HEAP: RefCell<Heap> = RefCell::new(Heap::for_isolate(std::env::var_os("DARTFORGE_GC_STRESS").is_some()));
    static CLASS_NAMES: RefCell<HashMap<i64, String>> = RefCell::new(HashMap::default());
    static SUBCLASSES: RefCell<HashMap<i64, Vec<i64>>> = RefCell::new(HashMap::default());
    /// As respostas de [`is_subclass`] já calculadas: toda checagem de tipo
    /// passa por aqui (a covariância de cada `Map.[]=`, por exemplo), e a
    /// busca no grafo alocava um conjunto por consulta. Limpo a cada
    /// registro de subclasse (a carga e cada recarga).
    static SUBTYPE_COMPUTED: RefCell<HashMap<(i64, i64), u8>> = RefCell::new(HashMap::default());
    /// Na frente de [`SUBTYPE_COMPUTED`], uma tabela de acesso direto
    /// (classe, alvo, resposta): o `v is String`/`is num`/`is Map` do
    /// `_JsonStringifier.writeJsonValue` e do `somaValor` do bench de JSON
    /// pagavam o hash da tupla e o `borrow` do mapa a cada teste. Limpa
    /// junto com o mapa.
    static SUBTYPE_FAST: std::cell::Cell<[(i64, i64, u8); FAST_SUBTYPE_SLOTS]> =
        const { std::cell::Cell::new([(-1, -1, 0); FAST_SUBTYPE_SLOTS]) };
    /// Os mapas de bits de subtipo que o código gerado lê em linha
    /// (`df.subclasse`, `llvm/mod.rs`; [`crate::heap::Context::subtypes`]).
    static SUBTYPE_MAPS: RefCell<SubtypeMaps> = RefCell::new(SubtypeMaps::default());
}

/// Para cada classe alvo `C` já consultada, o mapa de bits das classes
/// `cid <: C` (o type testing stub da VM, que responde `is C` sem ir ao
/// runtime): o `is C` do código gerado lê um bit, sem chamada. O mapa de
/// `C` é montado na primeira consulta que vai ao runtime
/// ([`dartforge_is_subclass`]), pelo grafo inverso (as subclasses de `C`),
/// e todos são descartados a cada relação nova de subclasse (a carga de um
/// módulo, uma recarga): nenhum mapa responde por um grafo antigo.
#[derive(Default)]
struct SubtypeMaps {
    /// Os mapas, pelo alvo; `pointers` é o que o [`crate::heap::Context`]
    /// publica (nulo = não montado).
    maps: Vec<Option<Box<[u64]>>>,
    pointers: Vec<*const u64>,
    /// A largura de todos os mapas em bits (múltiplo de 64): acima do maior
    /// id com relação registrada. 0 = a calcular.
    width: usize,
    /// O grafo inverso (supertipo direto → subtipos diretos), do
    /// [`SUBCLASSES`] vigente; vazio = a calcular.
    inverse: HashMap<i64, Vec<i64>>,
}

/// O maior alvo com mapa de bits (os ids de classe são densos e pequenos).
const LARGEST_MAPPED_TARGET: i64 = 1 << 16;

impl SubtypeMaps {
    /// Descarta todos os mapas (o grafo mudou) e tira os ponteiros do
    /// contexto.
    fn discard(&mut self) {
        crate::heap::CONTEXTO.with(|c| {
            c.n_subtypes.set(0);
            c.subtype_width.set(0);
            c.subtypes.set(std::ptr::null());
        });
        self.maps.clear();
        self.pointers.clear();
        self.width = 0;
        self.inverse.clear();
    }
    /// Monta o mapa do alvo `c` (0 ≤ c < [`LARGEST_MAPPED_TARGET`]) e publica.
    ///
    /// Nada que o código gerado possa ter lido é realocado ou solto aqui: a
    /// tabela de ponteiros nasce com um lugar para cada classe da largura
    /// (um alvo acima dela não tem subtipo registrado, e fica no runtime),
    /// e só [`SubtypeMaps::discard`] a solta — a partir de
    /// `dartforge_register_subclass`, uma chamada que o LLVM trata como
    /// escrita qualquer. `dartforge_is_subclass` é declarado `memory(read)`
    /// (`llvm/externs.rs`): um laço pode guardar a tabela lida antes dele.
    fn build(&mut self, c: i64) {
        let target = c as usize;
        if self.maps.get(target).is_some_and(Option::is_some) {
            return;
        }
        if self.width == 0 {
            SUBCLASSES.with(|m| {
                let m = m.borrow();
                // Só os ids densos contam para a largura: há ids fora da
                // faixa (a classe `_Type` do runtime, `0x3FFF_FF01`), que
                // ficam no runtime.
                let mut larger = 0i64;
                let dense = |id: i64| if (0..LARGEST_MAPPED_TARGET).contains(&id) { id } else { 0 };
                for (&sub, supers) in m.iter() {
                    larger = larger.max(dense(sub));
                    for &s in supers {
                        larger = larger.max(dense(s));
                        self.inverse.entry(s).or_default().push(sub);
                    }
                }
                self.width = (usize::try_from(larger).unwrap_or(0) + 1).div_ceil(64) * 64;
            });
            self.maps = (0..self.width).map(|_| None).collect();
            self.pointers = vec![std::ptr::null(); self.width];
            let (p, n, l) = (self.pointers.as_ptr(), self.pointers.len(), self.width);
            crate::heap::CONTEXTO.with(|ctx| {
                if !crate::heap::inline_class() {
                    return;
                }
                ctx.subtypes.set(p);
                ctx.n_subtypes.set(n);
                ctx.subtype_width.set(l);
            });
        }
        if target >= self.pointers.len() {
            return;
        }
        let words = self.width / 64;
        let mut map = vec![0u64; words].into_boxed_slice();
        let mut enable = |cid: i64| {
            if let Ok(i) = usize::try_from(cid)
                && i < words * 64
            {
                map[i / 64] |= 1 << (i % 64);
            }
        };
        if c == 0 {
            // `Object` é supertipo de toda classe.
            map.fill(u64::MAX);
        } else {
            let mut seen_set = HashSet::default();
            let mut queue = vec![c];
            seen_set.insert(c);
            while let Some(x) = queue.pop() {
                enable(x);
                if let Some(subs) = self.inverse.get(&x) {
                    for &s in subs {
                        if seen_set.insert(s) {
                            queue.push(s);
                        }
                    }
                }
            }
        }
        self.pointers[target] = map.as_ptr();
        self.maps[target] = Some(map);
    }
}

/// Vagas de [`SUBTYPE_FAST`] (potência de 2).
const FAST_SUBTYPE_SLOTS: usize = 512;

/// A vaga de (classe, alvo) em [`SUBTYPE_FAST`].
fn subtype_slot(class_id: i64, target_class: i64) -> usize {
    (class_id.wrapping_mul(0x9E37_79B9).wrapping_add(target_class) as usize) & (FAST_SUBTYPE_SLOTS - 1)
}

/// O heap para as funções do caminho rápido que o emissor declara com
/// efeitos restritos (`memory(none)`, `memory(inaccessiblemem: read)`,
/// `llvm/externs.rs`): sem o `RefCell::borrow`, que gravaria o contador de
/// empréstimos — uma escrita que essas declarações não permitem, mesmo
/// desfeita no fim. Quem usa só lê o heap (o `&mut` serve para tirar de um
/// vetor o ponteiro mutável dos elementos, sem gravar nada).
fn heap_without_borrow<R>(f: impl FnOnce(&mut Heap) -> R) -> R {
    HEAP.with(|h| {
        // SAFETY: só o código gerado chama estas funções, e ele nunca roda
        // dentro de um empréstimo do heap (o runtime não chama código Dart
        // segurando um: o `borrow` desse código entraria em pânico). O
        // heap já foi inicializado pelo primeiro acesso do programa.
        f(unsafe { &mut *h.as_ptr() })
    })
}

/// Registra o nome de uma classe pelo id para exibição em toString/print.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_register_class_name(class_id: i64, ptr: *const u8, len: i64) {
    let len = usize::try_from(len).expect("comprimento inválido");
    let bytes = if len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(ptr, len) }
    };
    let name = std::str::from_utf8(bytes).expect("UTF-8").to_string();
    CLASS_NAMES.with(|map| map.borrow_mut().insert(class_id, name));
}

/// Registra relação de subtipagem direta: sub_id <: super_id.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_register_subclass(sub_id: i64, super_id: i64) {
    SUBCLASSES.with(|map| {
        // Sem duplicar: a publicação de uma recarga refaz os registros.
        let mut supers = map.borrow_mut();
        let list = supers.entry(sub_id).or_default();
        if !list.contains(&super_id) {
            list.push(super_id);
            SUBTYPE_COMPUTED.with(|c| c.borrow_mut().clear());
            SUBTYPE_FAST.with(|c| c.set([(-1, -1, 0); FAST_SUBTYPE_SLOTS]));
            SUBTYPE_MAPS.with(|m| m.borrow_mut().discard());
        }
    });
}

/// Consulta pertinência de subtipagem nominal em tempo de execução.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_is_subclass(class_id: i64, target_class: i64) -> u8 {
    let r = is_subclass(class_id, target_class);
    // O próximo `is` deste alvo sai do mapa de bits, em linha.
    if (0..LARGEST_MAPPED_TARGET).contains(&target_class) {
        SUBTYPE_MAPS.with(|m| {
            if let Ok(mut m) = m.try_borrow_mut() {
                m.build(target_class);
            }
        });
    }
    if debug() {
        eprintln!("[depurar] is_subclass({class_id}, {target_class}) = {r}");
    }
    r
}

fn is_subclass(class_id: i64, target_class: i64) -> u8 {
    if class_id == target_class {
        return 1;
    }
    if target_class == 0 {
        // Object é supertipo de toda classe nominal
        return 1;
    }
    let slot = subtype_slot(class_id, target_class);
    // SAFETY: a tabela é local da thread e nenhuma referência a ela
    // sobrevive a esta leitura (nem a gravação abaixo) — o `Cell` só não
    // dá acesso a um elemento sem copiar o arranjo inteiro.
    let (c, t, r) = SUBTYPE_FAST.with(|x| unsafe { (*x.as_ptr())[slot] });
    if c == class_id && t == target_class {
        return r;
    }
    let r = match SUBTYPE_COMPUTED.with(|c| c.borrow().get(&(class_id, target_class)).copied()) {
        Some(r) => r,
        None => {
            let r = subclass_by_graph(class_id, target_class);
            SUBTYPE_COMPUTED.with(|c| c.borrow_mut().insert((class_id, target_class), r));
            r
        }
    };
    // SAFETY: como na leitura acima.
    SUBTYPE_FAST.with(|x| unsafe { (*x.as_ptr())[slot] = (class_id, target_class, r) });
    r
}

/// A busca de [`is_subclass`] no grafo de supertipos diretos.
fn subclass_by_graph(class_id: i64, target_class: i64) -> u8 {
    SUBCLASSES.with(|map| {
        let map = map.borrow();
        let mut visited = crate::hash::HashSet::default();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(class_id);
        visited.insert(class_id);
        while let Some(curr) = queue.pop_front() {
            if curr == target_class {
                return 1;
            }
            if let Some(supers) = map.get(&curr) {
                for &s in supers {
                    if visited.insert(s) {
                        queue.push_back(s);
                    }
                }
            }
        }
        0
    })
}



/// Aloca objeto inicialmente zerado, com campos ainda sem referências.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_object_new(class_id: i64, field_count: i64) -> i64 {
    let n = usize::try_from(field_count).expect("campos inválidos");
    let cid = i32::try_from(class_id).expect("classe inválida");
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let h = heap.allocate_instance(cid, n);
        // A próxima alocação deste tamanho sai em linha, da TLAB (sem
        // coletar aqui: `h` ainda não tem raiz).
        heap.refill_tlab(crate::layout::palavras_de_instancia(n));
        h
    })
}
// Os campos de um objeto do usuário são lidos e gravados em linha pelo
// código gerado (`llvm/mod.rs`, `GetField`/`SetField`): palavras de 8 bytes
// depois do cabeçalho do bloco (`heap::Header`), com o mapa de
// referências no cabeçalho.

use crate::heap::{CAMPOS_EM_LINHA, EMPTY_OBJECT};

/// O endereço dos campos (palavras de 8 bytes) do objeto `h`; de algo que
/// não é objeto, os campos zerados de [`EMPTY_OBJECT`]. O código gerado não
/// a usa mais (lê o bloco em linha); fica para a ABI.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_object_campos(h: i64) -> i64 {
    heap_without_borrow(|heap| heap.object_fields(h).map_or(EMPTY_OBJECT.fields.as_ptr() as i64, |p| p as i64))
}

/// A barreira de escrita do código gerado (`llvm/mod.rs`): o objeto `h`,
/// velho, recebeu um `Ref` num campo gravado em linha, e a próxima coleta
/// menor precisa percorrê-lo (`Heap::remember_object`). Não aloca.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_lembrar(h: i64) {
    heap_without_borrow(|heap| heap.remember_object(h));
}

/// Liga o ARC (`--memory=arc`, docs/ARC-IMPLEMENTACAO.md): a entrada do
/// programa compilado assim chama esta função antes de todo código Dart. O
/// sufixo é a versão da ABI de memória (docs/ARC-CICLOS-ESPECIFICACAO.md §24):
/// um runtime sem ela não liga com o programa.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_memoria_arc_v1() {
    HEAP.with(|heap| heap.borrow_mut().enable_arc());
    MEMORIA_ARC.with(|m| m.set(true));
}

thread_local! {
    /// O ARC está ligado neste isolado: a pergunta barata dos natives que
    /// gravam referências sem o `Heap` (`nativos_hash.rs`, `nativos_listas.rs`).
    static MEMORIA_ARC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// O ARC está ligado neste isolado.
#[inline(always)]
fn arc_enabled() -> bool {
    MEMORIA_ARC.with(std::cell::Cell::get)
}

/// A gravação crua de um native (sem o `Heap`) trocou `old` por `new`
/// numa posição forte de `h`: o ARC conta a troca (`Heap::arc_raw_write`).
/// Só com [`arc_enabled`]. Não aloca.
#[cold]
fn arc_raw_write(h: i64, old: i64, new: i64) {
    heap_without_borrow(|heap| heap.arc_raw_write(h, old, new));
}

/// No ARC, a gravação do `Ref` `v` na palavra `word ≥ 1` do corpo `REFS`
/// de `h` (o elemento de lista e o armazenamento da `_GrowableList` que o
/// emissor grava em linha no rastreamento): conta a troca e aplica a
/// barreira (`Heap::write_ref`). Não aloca.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_gravar_ref(h: i64, word: i64, v: i64) {
    let word = usize::try_from(word).expect("palavra negativa");
    HEAP.with(|heap| heap.borrow_mut().write_ref(h, word, v));
}

/// No ARC puro, as referências que o código gerado gravou em linha no
/// objeto `h` recém alocado (`AllocObject`, o contexto, a célula e a
/// closure em linha) passam a contar (`Heap::arc_count_initial`). Não
/// aloca.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_inicial(h: i64) {
    HEAP.with(|heap| heap.borrow_mut().arc_count_initial(h));
}

/// Obtém bits do campo pelo índice estável escolhido pelo emissor.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_object_get(handle: i64, index: i64) -> i64 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let Some(o) = heap.object(handle) else {
            return 0;
        };
        let idx = usize::try_from(index).unwrap_or(usize::MAX);
        o.get(idx).map_or(0, |(bits, _)| bits)
    })
}
/// Grava campo e informa explicitamente se seus bits são referência gerenciada.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_object_set(handle: i64, index: i64, bits: i64, is_ref: u8) {
    HEAP.with(|heap| heap.borrow_mut().set(handle, index, bits, is_ref != 0));
}

/// Estado de um campo `late` sem inicializador. O bit é independente dos
/// bits do campo: `0`, `false` e `null` podem ser valores já atribuídos.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_late_field_initialized(handle: i64, index: i64) -> u8 {
    HEAP.with(|heap| u8::from(heap.borrow().late_initialized((handle, index))))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_late_field_mark_initialized(handle: i64, index: i64) {
    HEAP.with(|heap| {
        heap.borrow_mut().mark_late((handle, index));
    });
}

/// O índice `-(index+2)` da mesma tabela lateral representa uma avaliação
/// em curso. `-1` fica reservado ao estado dos locais capturados em `Cell`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_late_field_initializing(handle: i64, index: i64) -> u8 {
    HEAP.with(|heap| u8::from(heap.borrow().late_initialized((handle, -index - 2))))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_late_field_set_initializing(handle: i64, index: i64, active: u8) {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let key = (handle, -index - 2);
        if active != 0 {
            heap.mark_late(key);
        } else {
            heap.unmark_late(key);
        }
    });
}

/// Compara igualdade (== de Dart) entre dois handles de referência.
///
/// Números comparam por valor, com a regra de `num`: `1 == 1.0` (R9); um
/// `Smi` e um `_Mint` nunca têm o mesmo valor (a forma é canônica, R10), mas
/// a comparação é por valor de todo modo. Strings, pelas unidades.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_equal(a: i64, b: i64) -> u8 {
    if a == b { return 1; }
    if a == 0 || b == 0 { return 0; }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        use crate::heap::Value::{Bool, Double, Int, Ref};
        u8::from(match (heap.value(a), heap.value(b)) {
            (Int(x), Int(y)) => x == y,
            (Bool(x), Bool(y)) => x == y,
            (Double(x), Double(y)) => x == y,
            (Int(x), Double(y)) => (x as f64) == y,
            (Double(x), Int(y)) => x == (y as f64),
            (Ref(x), Ref(y)) => heap.is_string(x) && heap.is_string(y) && heap.strings_equal(x, y),
            _ => false,
        })
    })
}

/// `identical(a, b)` sobre referências, com a semântica da VM
/// (`Instance::IsIdenticalTo`): mesmo handle, ou dois `_Mint` de mesmo
/// valor, ou dois `_Double` bit a bit iguais (R9; §2.10). É o caminho lento
/// de `@df.identical_one`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_identical(a: i64, b: i64) -> u8 {
    if a == b { return 1; }
    HEAP.with(|heap| u8::from(heap.borrow().identical_one(a, b)))
}

/// `Box` (R3/R10): `int` numa posição `Ref` — o `Smi` quando cabe em 63
/// bits (não aloca), senão o `_Mint` (caminho lento de `@df.box_int`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_box_int(v: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().box_int(v))
}

/// Lança o `TypeError` de uma coerção implícita que falhou (`null` ou
/// outro tipo onde se esperava o escalar).
fn throw_type_error() {
    let err = dartforge_type_error_new();
    dartforge_exception_throw(err, 3);
}

/// `Unbox` (R3/R10): `int` de uma referência (`Smi` ou `_Mint`); null ou
/// outro tipo lança TypeError. O caminho lento de `@df.desencaixa_int`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_unbox_int(h: i64) -> i64 {
    if crate::heap::smi::is_smi(h) {
        return crate::heap::smi::value(h);
    }
    let v = HEAP.with(|heap| heap.borrow().int_of(h));
    v.unwrap_or_else(|| {
        throw_type_error();
        0
    })
}

/// `Unbox`: `double` de uma referência (um `int` encaixotado não é `double`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_unbox_double(h: i64) -> f64 {
    let v = HEAP.with(|heap| heap.borrow().double_of(h));
    v.unwrap_or_else(|| {
        throw_type_error();
        0.0
    })
}

/// `Unbox`: `bool` de uma referência.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_unbox_bool(h: i64) -> u8 {
    let v = HEAP.with(|heap| heap.borrow().bool_of(h));
    v.map_or_else(
        || {
            throw_type_error();
            0
        },
        u8::from,
    )
}


/// A classe de um valor: o cid do cabeçalho de um objeto, 1 (`Null`) para
/// null e 2 (`_Smi`) para um `Smi` (§2.4). O código gerado a lê em linha
/// (`@df.class`); esta é a do runtime e a do `ClassID.getID`. Só lê o heap.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_value_class(handle: i64) -> i64 {
    heap_without_borrow(|heap| i64::from(heap.class(handle)))
}

/// Um valor da ABI de pares `(bits, tag)` (tags 1 int, 2 bool, 3 referência,
/// 4 double) como [`crate::heap::Value`].
fn pair_value(bits: i64, tag: i64) -> crate::heap::Value {
    use crate::heap::Value;
    match tag {
        1 => Value::Int(bits),
        2 => Value::Bool(bits != 0),
        3 => Value::Ref(bits),
        4 => Value::Double(f64::from_bits(bits as u64)),
        _ => panic!("tag de valor inválida"),
    }
}

/// Cria um record posicional a partir de um array plano de pares (bits, tag):
/// o caminho lento do `AllocRecord` com elemento sem caixa, que encaixota
/// aqui com as raízes certas.
///
/// # Safety
/// `pairs` aponta `2 * len` palavras legíveis.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_record_new(pairs: *const i64, len: i64) -> i64 {
    let len = usize::try_from(len).expect("comprimento inválido");
    let value_list: Vec<crate::heap::Value> = if len == 0 {
        Vec::new()
    } else {
        // SAFETY: o contrato da função.
        let raw = unsafe { std::slice::from_raw_parts(pairs, len * 2) };
        raw.chunks_exact(2).map(|pair| pair_value(pair[0], pair[1])).collect()
    };
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        // Cópias proprietárias dos argumentos e das caixas duram até o
        // record guardar as arestas; nenhuma coleta ocorre após fechar.
        let frame = heap.push_owning_frame(0);
        for v in &value_list {
            if let crate::heap::Value::Ref(r) = v {
                heap.root(frame, *r);
            }
        }
        let mut refs = Vec::with_capacity(len);
        for v in value_list {
            let r = heap.to_ref(v);
            heap.root(frame, r);
            refs.push(r);
        }
        let h = heap.new_record(&refs);
        heap.pop_frame(frame);
        h
    })
}

// ─── Espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md §3.6): esqueleto da P0a ──

#[cfg(test)]
mod tests_owners_record_and_concat {
    use super::*;

    #[test]
    #[allow(unsafe_code)]
    fn record_getter_borrows_field_without_creating_independent_owner() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            let text = HEAP.with(|h| h.borrow_mut().allocate_str("campo emprestado"));
            dartforge_arc_retain(text);
            let pair_list = [text, 3];
            // SAFETY: um par bits/tag válido; seu Ref tem token independente.
            let record = unsafe { dartforge_record_new(pair_list.as_ptr(), 1) };
            dartforge_arc_retain(record);
            dartforge_arc_release(text);
            assert_eq!(dartforge_nativo_DartForge_record_numFields(record), 1);
            assert_eq!(dartforge_nativo_DartForge_record_shape(record), 1);
            assert_eq!(dartforge_nativo_DartForge_record_numFields(0), 0);
            assert_eq!(dartforge_nativo_DartForge_record_shape(0), 0);
            let field = dartforge_nativo_DartForge_record_fieldAt(record, 0);
            assert_eq!(field, text);
            dartforge_arc_collect();
            assert!(HEAP.with(|h| h.borrow().is_live_object(field)));
            dartforge_arc_release(record);
            dartforge_arc_collect();
            assert!(!HEAP.with(|h| h.borrow().is_live_object(field)));
            assert!(!HEAP.with(|h| h.borrow().is_live_object(record)));
            HEAP.with(|h| { h.replace(previous); });
        }
    }

    fn with_arc_heap(f: impl FnOnce()) {
        let previous = HEAP.with(|h| h.replace(Heap::new(true)));
        HEAP.with(|h| h.borrow_mut().enable_arc());
        f();
        HEAP.with(|h| { h.replace(previous); });
    }

    #[test]
    #[allow(unsafe_code)]
    fn abi_record_preserves_aliases_and_boxes_until_publish() {
        with_arc_heap(|| {
            let (frame, text) = HEAP.with(|h| {
                let mut h = h.borrow_mut();
                let frame = h.push_owning_frame(1);
                let text = h.allocate_str("record");
                h.set_root(frame, 0, text);
                (frame, text)
            });
            let pair_list = [text, 3, i64::MAX, 1, text, 3, 2.5f64.to_bits() as i64, 4];
            // SAFETY: quatro pares bits/tag válidos, vivos durante a chamada.
            let record = unsafe { dartforge_record_new(pair_list.as_ptr(), 4) };
            HEAP.with(|h| {
                let mut h = h.borrow_mut();
                h.set_root(frame, 0, record);
                h.collect();
                let fields = h.record(record).unwrap().to_vec();
                assert_eq!(fields[0], text);
                assert_eq!(fields[2], text);
                assert!(matches!(h.value(fields[1]), crate::heap::Value::Int(i64::MAX)));
                assert!(matches!(h.value(fields[3]), crate::heap::Value::Double(x) if x == 2.5));
                h.pop_frame(frame);
                h.collect();
                assert!(!h.is_live_object(record));
                assert!(fields.iter().all(|&r| !h.is_live_object(r)));
            });
        });
    }

    #[test]
    fn concatenating_distinct_forms_preserves_boxes_and_releases_inputs() {
        with_arc_heap(|| {
            let (frame, a, b) = HEAP.with(|h| {
                let mut h = h.borrow_mut();
                let frame = h.push_owning_frame(2);
                let a = h.new_list(crate::layout::cid::LIST, 1, crate::listas::Element::Int);
                h.set_root(frame, 0, a);
                h.list_set(a, 0, crate::heap::Value::Int(i64::MAX));
                let b = h.new_list(crate::layout::cid::LIST, 1, crate::listas::Element::Double);
                h.set_root(frame, 1, b);
                h.list_set(b, 0, crate::heap::Value::Double(2.5));
                (frame, a, b)
            });
            let out = concat_lists(a, b).unwrap();
            HEAP.with(|h| {
                let mut h = h.borrow_mut();
                h.set_root(frame, 0, out);
                h.set_root(frame, 1, 0);
                h.collect();
                assert!(!h.is_live_object(a) && !h.is_live_object(b));
                let data = h.list_data(out);
                let caixas = h.words(data)[1..3].to_vec();
                assert!(matches!(h.value(caixas[0]), crate::heap::Value::Int(i64::MAX)));
                assert!(matches!(h.value(caixas[1]), crate::heap::Value::Double(x) if x == 2.5));
                h.pop_frame(frame);
                h.collect();
                assert!(!h.is_live_object(out) && !h.is_live_object(data));
                assert!(caixas.iter().all(|&r| !h.is_live_object(r)));
            });
        });
    }
}

/// Um record posicional (`_Record`, `REFS`) com os `n` `Ref` em `refs`: o
/// caminho do `AllocRecord` grande demais para a TLAB. Não lança.
///
/// # Safety
/// `refs` aponta `n` palavras legíveis.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_record_novo(refs: *const i64, n: i64) -> i64 {
    let n = usize::try_from(n).expect("comprimento inválido");
    let fields = if n == 0 {
        &[][..]
    } else {
        // SAFETY: o contrato da função.
        unsafe { std::slice::from_raw_parts(refs, n) }
    };
    // `new_record` enraíza os campos durante a alocação.
    HEAP.with(|heap| heap.borrow_mut().new_record(fields))
}

/// [`dartforge_record_novo`] sob ARC, para todo record literal: os `Ref`
/// emprestados ganham as arestas do record e o record um owner do código
/// antes de outra coleta. Não lança nem executa Dart.
///
/// # Safety
/// `refs` aponta `n` palavras legíveis com referências vivas (ou null).
///
/// ```
/// use dartforge_runtime::abi::*;
/// let a = dartforge_arc_box_int_owned_v1(i64::MAX);
/// // SAFETY: one field alive durante a criação.
/// let r = unsafe { dartforge_arc_record_novo_owned_v1([a].as_ptr(), 1) };
/// dartforge_arc_release(a);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observar_heap_v1(a), 1);
/// dartforge_arc_release(r);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observar_heap_v1(r), 0);
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arc_record_novo_owned_v1(refs: *const i64, n: i64) -> i64 {
    let n = usize::try_from(n).expect("comprimento inválido");
    let fields = if n == 0 {
        &[][..]
    } else {
        // SAFETY: o contrato da função.
        unsafe { std::slice::from_raw_parts(refs, n) }
    };
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let r = heap.new_counted_record(fields);
        heap.retain_code_owner(r);
        r
    })
}

// ─── Natives de identidade, closures e records (vindos de `nativos_listas.rs`,
// docs/NATIVO-ESPACO-UNIFICADO.md §4.9) ───────────────────────────────────────

/// `ClassID.getID(o)`: o cid do valor (`ClassID_getID`; o emissor o faz em
/// linha por `@df.class`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_ClassID_getID(o: i64) -> i64 {
    dartforge_value_class(o)
}

/// `identical(a, b)` (`Identical_comparison`; o lowering o faz em linha por
/// `@df.identical_one`, `lower/caixas.rs`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Identical_comparison(a: i64, b: i64) -> u8 {
    dartforge_identical(a, b)
}

/// O receptor de um tear-off de método: o único campo `Ref` do `_Contexto` de
/// um campo só (`AllocEnv [receiver_obj]`, `lower/closures.rs`). Uma closure de
/// ambiente direto ou com outro contexto não tem receptor (só é igual a si
/// mesma).
fn tearoff_receiver(heap: &Heap, c: &crate::caixas::ClosureRef) -> Option<i64> {
    let (ctx, is_ref_kind) = c.contexto;
    if !is_ref_kind || !crate::heap::is_object(ctx) || heap.class(ctx) != crate::layout::cid::CONTEXTO {
        return None;
    }
    let o = heap.object(ctx)?;
    match (o.len(), o.get(0)) {
        (1, Some((r, true))) => Some(r),
        _ => None,
    }
}

/// `_Closure.==` (`Closure_equals`): a mesma função e o mesmo receptor (o
/// tear-off de um método sobre o mesmo objeto); closures comuns só são
/// iguais a si mesmas.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Closure_equals(this: i64, other_one: i64) -> u8 {
    if this == other_one {
        return 1;
    }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let (Some(a), Some(b)) = (heap.closure(this), heap.closure(other_one)) else { return 0 };
        if a.code != b.code {
            return 0;
        }
        match (tearoff_receiver(&heap, &a), tearoff_receiver(&heap, &b)) {
            (Some(x), Some(y)) => u8::from(x == y),
            _ => 0,
        }
    })
}

/// `_Closure._computeHash` (`Closure_computeHash`): coerente com o
/// `Closure_equals` — a função e, no tear-off de um método, o receptor.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Closure_computeHash(this: i64) -> i64 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let Some(c) = heap.closure(this) else { return 0 };
        let mut h = (c.code as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        if let Some(r) = tearoff_receiver(&heap, &c) {
            h ^= (r as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
        }
        // Um `Smi` positivo de 30 bits, como o hash da VM.
        ((h >> 34) & 0x3FFF_FFFF) as i64
    })
}

/// `_Record._numFields`: quantos campos (o record posicional do runtime).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_record_numFields(this: i64) -> i64 {
    HEAP.with(|heap| heap.borrow().record(this).map_or(0, |c| c.len() as i64))
}

/// `_Record._shape`: a forma — para o record posicional, o número de campos.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_record_shape(this: i64) -> i64 {
    dartforge_nativo_DartForge_record_numFields(this)
}

/// `_Record._fieldNames`: os nomes (nenhum no record posicional): uma
/// `_ImmutableList` vazia.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_record_fieldNames(_this: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().new_list(crate::layout::cid::IMMUTABLE_LIST, 0, crate::listas::Element::General))
}

/// `_Record._fieldAt(i)`: o campo (já em posição `Ref`), ou null fora da forma.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_record_fieldAt(this: i64, i: i64) -> i64 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let fields = heap.record(this);
        usize::try_from(i).ok().and_then(|i| fields.and_then(|c| c.get(i).copied())).unwrap_or(0)
    })
}

/// `Object._getHash`/`identityHashCode` (`Object_getHash`, §2.10): o `int` é o
/// próprio valor; `true`/`false`, as constantes da VM (1231/1237); a `String`,
/// o hash do conteúdo (o `_identityHashCode` da VM é o `String_getHashCode`);
/// os demais objetos, o hash de identidade do cabeçalho
/// (`Heap::identity_hash`, estável porque o coletor não move).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Object_getHash(o: i64) -> i64 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.value(o) {
            crate::heap::Value::Int(i) => return i,
            crate::heap::Value::Bool(b) => return if b { 1231 } else { 1237 },
            _ => {}
        }
        if o == 0 {
            return 2011;
        }
        if heap.is_string(o) {
            return heap.hash_de_texto(o).map_or(0, i64::from);
        }
        heap.identity_hash(o).unwrap_or((o >> 3) & 0x3fff_ffff)
    })
}
