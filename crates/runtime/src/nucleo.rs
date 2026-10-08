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
    let codigo = finalizar_programa();
    if codigo != 0 {
        std::process::exit(codigo);
    }
    0
}

/// A entrada do programa com o SDK da fonte (P5c): o runtime e o SDK moram
/// numa DLL (cfg `dartforge_runtime_dll`, sem o `main` C), e o `main` do
/// executável — que o emissor escreve — chama esta função com a
/// `dartforge_entry` dele.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_iniciar(entrada: extern "C" fn(), para_texto: extern "C" fn(i64) -> i64) -> i32 {
    PARA_TEXTO.with(|p| p.set(Some(para_texto)));
    // O isolado principal usa as listas livres do alocador (`alocador.rs`).
    ligar_cache_de_alocacao();
    if depurar() {
        // Depuração: o pânico do runtime mostra a pilha de funções Dart
        // (`DARTFORGE_RASTRO=1` na compilação).
        let padrao = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |i| {
            mostrar_rastro();
            padrao(i);
        }));
    }
    // O isolado principal aceita os pedidos no ponto seguro (a recarga do
    // JIT) enquanto a entrada roda — o `main` e o laço de eventos.
    marcar_isolado_principal();
    entrada();
    desmarcar_isolado_principal();
    let codigo = finalizar_programa();
    if codigo != 0 {
        std::process::exit(codigo);
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
    encerrar_finalizadores_do_isolado();
    // `Isolate.exit` no isolado principal: ele terminou, sem erro.
    let pending = EXCEPTION.with(|slot| slot.borrow().is_some()) && !desenrolando();
    if pending {
        // O rastro antes do `toString()` do erro, que roda código Dart (um
        // `throw` tratado lá dentro trocaria o rastro guardado).
        let rastro = texto_do_rastro_da_excecao();
        let (bits, tag) = EXCEPTION.with(|slot| {
            let value = slot.borrow().expect("exceção verificada acima");
            (bits_da_excecao(value), etiqueta_da_excecao(value))
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
                        tomar_excecao();
                        let t = com_raizes(&[bits], || dart_r1(f as usize, bits));
                        let s = HEAP.with(|h| h.borrow().texto(t).map(|texto| texto.para_string()));
                        // O `toString()` também falhou: a descrição do
                        // runtime (a classe do objeto).
                        s.unwrap_or_else(|| {
                            tomar_excecao();
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
            if !rastro.is_empty() {
                let _ = write!(err, "{rastro}");
                if !rastro.ends_with('\n') {
                    let _ = writeln!(err);
                }
            }
        });
        return 255;
    }
    // `DARTFORGE_GC_STATS=1` escreve as estatísticas; `=exigir` também, e
    // devolve 70 se a execução não exercitou o coletor: nenhuma coleta, ou
    // uma imagem com raízes por mapas e nenhuma raiz lida de mapa
    // (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.5, a prova de que coletou:
    // um teste que nunca coletou no ponto perigoso não pode passar).
    let estatisticas = std::env::var("DARTFORGE_GC_STATS").unwrap_or_default();
    let mut nada_exercitado = false;
    if estatisticas == "1" || estatisticas == "exigir" {
        HEAP.with(|heap| {
            let s = heap.borrow().stats();
            let (quadros, raizes_de_mapa, conferidas) = crate::heap::numeros_do_percurso();
            eprintln!("{{\"dartforge_gc\":{{\"allocations\":{},\"collections\":{},\"reclaimed\":{},\"live_objects\":{},\"reserved_slots\":{},\"root_slots\":{},\"peak_root_slots\":{},\"live_roots\":{},\"peak_roots\":{},\"live_bytes\":{},\"peak_live_bytes\":{},\"permanent_roots\":{},\"smi_caixas_evitadas\":{},\"map_frames\":{},\"map_roots\":{},\"checked_roots\":{}}}}}",
                s.allocations, s.collections, s.reclaimed, s.live_objects, s.reserved_slots,
                s.root_slots, s.peak_root_slots, s.live_roots, s.peak_roots,
                s.estimated_bytes, s.peak_estimated_bytes, s.permanent_roots, s.caixas_evitadas,
                quadros, raizes_de_mapa, conferidas);
            nada_exercitado = s.collections == 0 || (crate::heap::ha_mapas_de_pilha() && raizes_de_mapa == 0);
        });
    }
    if estatisticas == "exigir" && nada_exercitado {
        eprintln!("dartforge: DARTFORGE_GC_STATS=exigir e a execução não exercitou o coletor (nenhuma coleta, ou nenhuma raiz lida de mapa)");
        return 70;
    }
    if std::env::var("DARTFORGE_GC_MEMORIA").is_ok_and(|v| !v.is_empty() && v != "0") {
        HEAP.with(|heap| eprint!("[memória] fim do programa\n{}", heap.borrow().relatorio_de_memoria()));
    }
    codigo_de_saida_global()
}

use crate::heap::{CONTEXTO, Heap, Texto, TextoMut};
use std::cell::RefCell;
use crate::hash::{HashMap, HashSet};

thread_local! {
    /// SDK da fonte: o `toString()` Dart de um valor (a
    /// `dartforge_dispatch_toString` do programa), para a exceção não
    /// capturada.
    static PARA_TEXTO: std::cell::Cell<Option<extern "C" fn(i64) -> i64>> = const { std::cell::Cell::new(None) };
    static HEAP: RefCell<Heap> = RefCell::new(Heap::do_isolado(std::env::var_os("DARTFORGE_GC_STRESS").is_some()));
    static CLASS_NAMES: RefCell<HashMap<i64, String>> = RefCell::new(HashMap::default());
    static SUBCLASSES: RefCell<HashMap<i64, Vec<i64>>> = RefCell::new(HashMap::default());
    /// As respostas de [`is_subclass`] já calculadas: toda checagem de tipo
    /// passa por aqui (a covariância de cada `Map.[]=`, por exemplo), e a
    /// busca no grafo alocava um conjunto por consulta. Limpo a cada
    /// registro de subclasse (a carga e cada recarga).
    static SUBTIPO_CALCULADO: RefCell<HashMap<(i64, i64), u8>> = RefCell::new(HashMap::default());
    /// Na frente de [`SUBTIPO_CALCULADO`], uma tabela de acesso direto
    /// (classe, alvo, resposta): o `v is String`/`is num`/`is Map` do
    /// `_JsonStringifier.writeJsonValue` e do `somaValor` do bench de JSON
    /// pagavam o hash da tupla e o `borrow` do mapa a cada teste. Limpa
    /// junto com o mapa.
    static SUBTIPO_RAPIDO: std::cell::Cell<[(i64, i64, u8); VAGAS_SUBTIPO_RAPIDO]> =
        const { std::cell::Cell::new([(-1, -1, 0); VAGAS_SUBTIPO_RAPIDO]) };
    /// Os mapas de bits de subtipo que o código gerado lê em linha
    /// (`df.subclasse`, `llvm/mod.rs`; [`crate::heap::Contexto::subtipos`]).
    static MAPAS_DE_SUBTIPO: RefCell<MapasDeSubtipo> = RefCell::new(MapasDeSubtipo::default());
}

/// Para cada classe alvo `C` já consultada, o mapa de bits das classes
/// `cid <: C` (o type testing stub da VM, que responde `is C` sem ir ao
/// runtime): o `is C` do código gerado lê um bit, sem chamada. O mapa de
/// `C` é montado na primeira consulta que vai ao runtime
/// ([`dartforge_is_subclass`]), pelo grafo inverso (as subclasses de `C`),
/// e todos são descartados a cada relação nova de subclasse (a carga de um
/// módulo, uma recarga): nenhum mapa responde por um grafo antigo.
#[derive(Default)]
struct MapasDeSubtipo {
    /// Os mapas, pelo alvo; `ponteiros` é o que o [`crate::heap::Contexto`]
    /// publica (nulo = não montado).
    mapas: Vec<Option<Box<[u64]>>>,
    ponteiros: Vec<*const u64>,
    /// A largura de todos os mapas em bits (múltiplo de 64): acima do maior
    /// id com relação registrada. 0 = a calcular.
    largura: usize,
    /// O grafo inverso (supertipo direto → subtipos diretos), do
    /// [`SUBCLASSES`] vigente; vazio = a calcular.
    inverso: HashMap<i64, Vec<i64>>,
}

/// O maior alvo com mapa de bits (os ids de classe são densos e pequenos).
const MAIOR_ALVO_COM_MAPA: i64 = 1 << 16;

impl MapasDeSubtipo {
    /// Descarta todos os mapas (o grafo mudou) e tira os ponteiros do
    /// contexto.
    fn descartar(&mut self) {
        crate::heap::CONTEXTO.with(|c| {
            c.n_subtipos.set(0);
            c.largura_subtipos.set(0);
            c.subtipos.set(std::ptr::null());
        });
        self.mapas.clear();
        self.ponteiros.clear();
        self.largura = 0;
        self.inverso.clear();
    }
    /// Monta o mapa do alvo `c` (0 ≤ c < [`MAIOR_ALVO_COM_MAPA`]) e publica.
    ///
    /// Nada que o código gerado possa ter lido é realocado ou solto aqui: a
    /// tabela de ponteiros nasce com um lugar para cada classe da largura
    /// (um alvo acima dela não tem subtipo registrado, e fica no runtime),
    /// e só [`MapasDeSubtipo::descartar`] a solta — a partir de
    /// `dartforge_register_subclass`, uma chamada que o LLVM trata como
    /// escrita qualquer. `dartforge_is_subclass` é declarado `memory(read)`
    /// (`llvm/externs.rs`): um laço pode guardar a tabela lida antes dele.
    fn montar(&mut self, c: i64) {
        let alvo = c as usize;
        if self.mapas.get(alvo).is_some_and(Option::is_some) {
            return;
        }
        if self.largura == 0 {
            SUBCLASSES.with(|m| {
                let m = m.borrow();
                // Só os ids densos contam para a largura: há ids fora da
                // faixa (a classe `_Type` do runtime, `0x3FFF_FF01`), que
                // ficam no runtime.
                let mut maior = 0i64;
                let denso = |id: i64| if (0..MAIOR_ALVO_COM_MAPA).contains(&id) { id } else { 0 };
                for (&sub, supers) in m.iter() {
                    maior = maior.max(denso(sub));
                    for &s in supers {
                        maior = maior.max(denso(s));
                        self.inverso.entry(s).or_default().push(sub);
                    }
                }
                self.largura = (usize::try_from(maior).unwrap_or(0) + 1).div_ceil(64) * 64;
            });
            self.mapas = (0..self.largura).map(|_| None).collect();
            self.ponteiros = vec![std::ptr::null(); self.largura];
            let (p, n, l) = (self.ponteiros.as_ptr(), self.ponteiros.len(), self.largura);
            crate::heap::CONTEXTO.with(|ctx| {
                if !crate::heap::classe_em_linha() {
                    return;
                }
                ctx.subtipos.set(p);
                ctx.n_subtipos.set(n);
                ctx.largura_subtipos.set(l);
            });
        }
        if alvo >= self.ponteiros.len() {
            return;
        }
        let palavras = self.largura / 64;
        let mut mapa = vec![0u64; palavras].into_boxed_slice();
        let mut ligar = |cid: i64| {
            if let Ok(i) = usize::try_from(cid)
                && i < palavras * 64
            {
                mapa[i / 64] |= 1 << (i % 64);
            }
        };
        if c == 0 {
            // `Object` é supertipo de toda classe.
            mapa.fill(u64::MAX);
        } else {
            let mut vistos = HashSet::default();
            let mut fila = vec![c];
            vistos.insert(c);
            while let Some(x) = fila.pop() {
                ligar(x);
                if let Some(subs) = self.inverso.get(&x) {
                    for &s in subs {
                        if vistos.insert(s) {
                            fila.push(s);
                        }
                    }
                }
            }
        }
        self.ponteiros[alvo] = mapa.as_ptr();
        self.mapas[alvo] = Some(mapa);
    }
}

/// Vagas de [`SUBTIPO_RAPIDO`] (potência de 2).
const VAGAS_SUBTIPO_RAPIDO: usize = 512;

/// A vaga de (classe, alvo) em [`SUBTIPO_RAPIDO`].
fn vaga_subtipo(class_id: i64, target_class: i64) -> usize {
    (class_id.wrapping_mul(0x9E37_79B9).wrapping_add(target_class) as usize) & (VAGAS_SUBTIPO_RAPIDO - 1)
}

/// O heap para as funções do caminho rápido que o emissor declara com
/// efeitos restritos (`memory(none)`, `memory(inaccessiblemem: read)`,
/// `llvm/externs.rs`): sem o `RefCell::borrow`, que gravaria o contador de
/// empréstimos — uma escrita que essas declarações não permitem, mesmo
/// desfeita no fim. Quem usa só lê o heap (o `&mut` serve para tirar de um
/// vetor o ponteiro mutável dos elementos, sem gravar nada).
fn heap_sem_emprestimo<R>(f: impl FnOnce(&mut Heap) -> R) -> R {
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
        let lista = supers.entry(sub_id).or_default();
        if !lista.contains(&super_id) {
            lista.push(super_id);
            SUBTIPO_CALCULADO.with(|c| c.borrow_mut().clear());
            SUBTIPO_RAPIDO.with(|c| c.set([(-1, -1, 0); VAGAS_SUBTIPO_RAPIDO]));
            MAPAS_DE_SUBTIPO.with(|m| m.borrow_mut().descartar());
        }
    });
}

/// Consulta pertinência de subtipagem nominal em tempo de execução.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_is_subclass(class_id: i64, target_class: i64) -> u8 {
    let r = is_subclass(class_id, target_class);
    // O próximo `is` deste alvo sai do mapa de bits, em linha.
    if (0..MAIOR_ALVO_COM_MAPA).contains(&target_class) {
        MAPAS_DE_SUBTIPO.with(|m| {
            if let Ok(mut m) = m.try_borrow_mut() {
                m.montar(target_class);
            }
        });
    }
    if depurar() {
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
    let vaga = vaga_subtipo(class_id, target_class);
    // SAFETY: a tabela é local da thread e nenhuma referência a ela
    // sobrevive a esta leitura (nem a gravação abaixo) — o `Cell` só não
    // dá acesso a um elemento sem copiar o arranjo inteiro.
    let (c, t, r) = SUBTIPO_RAPIDO.with(|x| unsafe { (*x.as_ptr())[vaga] });
    if c == class_id && t == target_class {
        return r;
    }
    let r = match SUBTIPO_CALCULADO.with(|c| c.borrow().get(&(class_id, target_class)).copied()) {
        Some(r) => r,
        None => {
            let r = subclasse_pelo_grafo(class_id, target_class);
            SUBTIPO_CALCULADO.with(|c| c.borrow_mut().insert((class_id, target_class), r));
            r
        }
    };
    // SAFETY: como na leitura acima.
    SUBTIPO_RAPIDO.with(|x| unsafe { (*x.as_ptr())[vaga] = (class_id, target_class, r) });
    r
}

/// A busca de [`is_subclass`] no grafo de supertipos diretos.
fn subclasse_pelo_grafo(class_id: i64, target_class: i64) -> u8 {
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
        let h = heap.alocar_instancia(cid, n);
        // A próxima alocação deste tamanho sai em linha, da TLAB (sem
        // coletar aqui: `h` ainda não tem raiz).
        heap.reabastecer_tlab(crate::layout::palavras_de_instancia(n));
        h
    })
}
// Os campos de um objeto do usuário são lidos e gravados em linha pelo
// código gerado (`llvm/mod.rs`, `GetField`/`SetField`): palavras de 8 bytes
// depois do cabeçalho do bloco (`heap::Cabecalho`), com o mapa de
// referências no cabeçalho.

use crate::heap::{CAMPOS_EM_LINHA, OBJETO_VAZIO};

/// O endereço dos campos (palavras de 8 bytes) do objeto `h`; de algo que
/// não é objeto, os campos zerados de [`OBJETO_VAZIO`]. O código gerado não
/// a usa mais (lê o bloco em linha); fica para a ABI.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_object_campos(h: i64) -> i64 {
    heap_sem_emprestimo(|heap| heap.campos_de_objeto(h).map_or(OBJETO_VAZIO.campos.as_ptr() as i64, |p| p as i64))
}

/// A barreira de escrita do código gerado (`llvm/mod.rs`): o objeto `h`,
/// velho, recebeu um `Ref` num campo gravado em linha, e a próxima coleta
/// menor precisa percorrê-lo (`Heap::lembrar_objeto`). Não aloca.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_lembrar(h: i64) {
    heap_sem_emprestimo(|heap| heap.lembrar_objeto(h));
}

/// Liga o ARC (`--memoria=arc`, docs/ARC-IMPLEMENTACAO.md): a entrada do
/// programa compilado assim chama esta função antes de todo código Dart. O
/// sufixo é a versão da ABI de memória (docs/ARC-CICLOS-ESPECIFICACAO.md §24):
/// um runtime sem ela não liga com o programa.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_memoria_arc_v1() {
    HEAP.with(|heap| heap.borrow_mut().ativar_arc());
    MEMORIA_ARC.with(|m| m.set(true));
}

thread_local! {
    /// O ARC está ligado neste isolado: a pergunta barata dos natives que
    /// gravam referências sem o `Heap` (`nativos_hash.rs`, `nativos_listas.rs`).
    static MEMORIA_ARC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// O ARC está ligado neste isolado.
#[inline(always)]
fn arc_ligado() -> bool {
    MEMORIA_ARC.with(std::cell::Cell::get)
}

/// A gravação crua de um native (sem o `Heap`) trocou `antigo` por `novo`
/// numa posição forte de `h`: o ARC conta a troca (`Heap::arc_gravacao_crua`).
/// Só com [`arc_ligado`]. Não aloca.
#[cold]
fn arc_gravacao_crua(h: i64, antigo: i64, novo: i64) {
    heap_sem_emprestimo(|heap| heap.arc_gravacao_crua(h, antigo, novo));
}

/// No ARC, a gravação do `Ref` `v` na palavra `palavra ≥ 1` do corpo `REFS`
/// de `h` (o elemento de lista e o armazenamento da `_GrowableList` que o
/// emissor grava em linha no rastreamento): conta a troca e aplica a
/// barreira (`Heap::gravar_ref`). Não aloca.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_gravar_ref(h: i64, palavra: i64, v: i64) {
    let palavra = usize::try_from(palavra).expect("palavra negativa");
    HEAP.with(|heap| heap.borrow_mut().gravar_ref(h, palavra, v));
}

/// No ARC puro, as referências que o código gerado gravou em linha no
/// objeto `h` recém alocado (`AllocObject`, o contexto, a célula e a
/// closure em linha) passam a contar (`Heap::arc_contar_iniciais`). Não
/// aloca.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_inicial(h: i64) {
    HEAP.with(|heap| heap.borrow_mut().arc_contar_iniciais(h));
}

/// Obtém bits do campo pelo índice estável escolhido pelo emissor.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_object_get(handle: i64, index: i64) -> i64 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let Some(o) = heap.objeto(handle) else {
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
    HEAP.with(|heap| u8::from(heap.borrow().late_inicializado((handle, index))))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_late_field_mark_initialized(handle: i64, index: i64) {
    HEAP.with(|heap| {
        heap.borrow_mut().marcar_late((handle, index));
    });
}

/// O índice `-(index+2)` da mesma tabela lateral representa uma avaliação
/// em curso. `-1` fica reservado ao estado dos locais capturados em `Cell`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_late_field_initializing(handle: i64, index: i64) -> u8 {
    HEAP.with(|heap| u8::from(heap.borrow().late_inicializado((handle, -index - 2))))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_late_field_set_initializing(handle: i64, index: i64, active: u8) {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let key = (handle, -index - 2);
        if active != 0 {
            heap.marcar_late(key);
        } else {
            heap.desmarcar_late(key);
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
        use crate::heap::Valor::{Bool, Double, Int, Ref};
        u8::from(match (heap.valor(a), heap.valor(b)) {
            (Int(x), Int(y)) => x == y,
            (Bool(x), Bool(y)) => x == y,
            (Double(x), Double(y)) => x == y,
            (Int(x), Double(y)) => (x as f64) == y,
            (Double(x), Int(y)) => x == (y as f64),
            (Ref(x), Ref(y)) => heap.e_texto(x) && heap.e_texto(y) && heap.textos_iguais(x, y),
            _ => false,
        })
    })
}

/// `identical(a, b)` sobre referências, com a semântica da VM
/// (`Instance::IsIdenticalTo`): mesmo handle, ou dois `_Mint` de mesmo
/// valor, ou dois `_Double` bit a bit iguais (R9; §2.10). É o caminho lento
/// de `@df.identico`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_identical(a: i64, b: i64) -> u8 {
    if a == b { return 1; }
    HEAP.with(|heap| u8::from(heap.borrow().identico(a, b)))
}

/// `Box` (R3/R10): `int` numa posição `Ref` — o `Smi` quando cabe em 63
/// bits (não aloca), senão o `_Mint` (caminho lento de `@df.caixa_int`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_box_int(v: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().caixa_int(v))
}

/// Lança o `TypeError` de uma coerção implícita que falhou (`null` ou
/// outro tipo onde se esperava o escalar).
fn lancar_type_error() {
    let err = dartforge_type_error_new();
    dartforge_exception_throw(err, 3);
}

/// `Unbox` (R3/R10): `int` de uma referência (`Smi` ou `_Mint`); null ou
/// outro tipo lança TypeError. O caminho lento de `@df.desencaixa_int`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_unbox_int(h: i64) -> i64 {
    if crate::heap::smi::e_smi(h) {
        return crate::heap::smi::valor(h);
    }
    let v = HEAP.with(|heap| heap.borrow().int_de(h));
    v.unwrap_or_else(|| {
        lancar_type_error();
        0
    })
}

/// `Unbox`: `double` de uma referência (um `int` encaixotado não é `double`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_unbox_double(h: i64) -> f64 {
    let v = HEAP.with(|heap| heap.borrow().double_de(h));
    v.unwrap_or_else(|| {
        lancar_type_error();
        0.0
    })
}

/// `Unbox`: `bool` de uma referência.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_unbox_bool(h: i64) -> u8 {
    let v = HEAP.with(|heap| heap.borrow().bool_de(h));
    v.map_or_else(
        || {
            lancar_type_error();
            0
        },
        u8::from,
    )
}


/// A classe de um valor: o cid do cabeçalho de um objeto, 1 (`Null`) para
/// null e 2 (`_Smi`) para um `Smi` (§2.4). O código gerado a lê em linha
/// (`@df.classe`); esta é a do runtime e a do `ClassID.getID`. Só lê o heap.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_value_class(handle: i64) -> i64 {
    heap_sem_emprestimo(|heap| i64::from(heap.classe(handle)))
}

/// Um valor da ABI de pares `(bits, tag)` (tags 1 int, 2 bool, 3 referência,
/// 4 double) como [`crate::heap::Valor`].
fn valor_de_par(bits: i64, tag: i64) -> crate::heap::Valor {
    use crate::heap::Valor;
    match tag {
        1 => Valor::Int(bits),
        2 => Valor::Bool(bits != 0),
        3 => Valor::Ref(bits),
        4 => Valor::Double(f64::from_bits(bits as u64)),
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
    let valores: Vec<crate::heap::Valor> = if len == 0 {
        Vec::new()
    } else {
        // SAFETY: o contrato da função.
        let raw = unsafe { std::slice::from_raw_parts(pairs, len * 2) };
        raw.chunks_exact(2).map(|par| valor_de_par(par[0], par[1])).collect()
    };
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        // Os `Ref` e cada caixa nova ficam enraizados até o record existir.
        let quadro = heap.push_frame();
        for v in &valores {
            if let crate::heap::Valor::Ref(r) = v {
                heap.root(quadro, *r);
            }
        }
        let mut refs = Vec::with_capacity(len);
        for v in valores {
            let r = heap.como_ref(v);
            heap.root(quadro, r);
            refs.push(r);
        }
        let h = heap.novo_record(&refs);
        heap.pop_frame(quadro);
        h
    })
}

// ─── Espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md §3.6): esqueleto da P0a ──

/// Um record posicional (`_Record`, `REFS`) com os `n` `Ref` em `refs`: o
/// caminho do `AllocRecord` grande demais para a TLAB. Não lança.
///
/// # Safety
/// `refs` aponta `n` palavras legíveis.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_record_novo(refs: *const i64, n: i64) -> i64 {
    let n = usize::try_from(n).expect("comprimento inválido");
    let campos = if n == 0 {
        &[][..]
    } else {
        // SAFETY: o contrato da função.
        unsafe { std::slice::from_raw_parts(refs, n) }
    };
    // `novo_record` enraíza os campos durante a alocação.
    HEAP.with(|heap| heap.borrow_mut().novo_record(campos))
}

// ─── Natives de identidade, closures e records (vindos de `nativos_listas.rs`,
// docs/NATIVO-ESPACO-UNIFICADO.md §4.9) ───────────────────────────────────────

/// `ClassID.getID(o)`: o cid do valor (`ClassID_getID`; o emissor o faz em
/// linha por `@df.classe`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_ClassID_getID(o: i64) -> i64 {
    dartforge_value_class(o)
}

/// `identical(a, b)` (`Identical_comparison`; o lowering o faz em linha por
/// `@df.identico`, `lower/caixas.rs`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Identical_comparison(a: i64, b: i64) -> u8 {
    dartforge_identical(a, b)
}

/// O receptor de um tear-off de método: o único campo `Ref` do `_Contexto` de
/// um campo só (`AllocEnv [receptor]`, `lower/closures.rs`). Uma closure de
/// ambiente direto ou com outro contexto não tem receptor (só é igual a si
/// mesma).
fn receptor_do_tearoff(heap: &Heap, c: &crate::caixas::ClosureRef) -> Option<i64> {
    let (ctx, e_ref) = c.contexto;
    if !e_ref || !crate::heap::e_objeto(ctx) || heap.classe(ctx) != crate::layout::cid::CONTEXTO {
        return None;
    }
    let o = heap.objeto(ctx)?;
    match (o.len(), o.get(0)) {
        (1, Some((r, true))) => Some(r),
        _ => None,
    }
}

/// `_Closure.==` (`Closure_equals`): a mesma função e o mesmo receptor (o
/// tear-off de um método sobre o mesmo objeto); closures comuns só são
/// iguais a si mesmas.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Closure_equals(this: i64, outro: i64) -> u8 {
    if this == outro {
        return 1;
    }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let (Some(a), Some(b)) = (heap.closure(this), heap.closure(outro)) else { return 0 };
        if a.codigo != b.codigo {
            return 0;
        }
        match (receptor_do_tearoff(&heap, &a), receptor_do_tearoff(&heap, &b)) {
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
        let mut h = (c.codigo as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        if let Some(r) = receptor_do_tearoff(&heap, &c) {
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
    HEAP.with(|heap| heap.borrow_mut().nova_lista(crate::layout::cid::IMMUTABLE_LIST, 0, crate::listas::Elemento::Geral))
}

/// `_Record._fieldAt(i)`: o campo (já em posição `Ref`), ou null fora da forma.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_record_fieldAt(this: i64, i: i64) -> i64 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let campos = heap.record(this);
        usize::try_from(i).ok().and_then(|i| campos.and_then(|c| c.get(i).copied())).unwrap_or(0)
    })
}

/// `Object._getHash`/`identityHashCode` (`Object_getHash`, §2.10): o `int` é o
/// próprio valor; `true`/`false`, as constantes da VM (1231/1237); a `String`,
/// o hash do conteúdo (o `_identityHashCode` da VM é o `String_getHashCode`);
/// os demais objetos, o hash de identidade do cabeçalho
/// (`Heap::hash_de_identidade`, estável porque o coletor não move).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Object_getHash(o: i64) -> i64 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.valor(o) {
            crate::heap::Valor::Int(i) => return i,
            crate::heap::Valor::Bool(b) => return if b { 1231 } else { 1237 },
            _ => {}
        }
        if o == 0 {
            return 2011;
        }
        if heap.e_texto(o) {
            return heap.hash_de_texto(o).map_or(0, i64::from);
        }
        heap.hash_de_identidade(o).unwrap_or((o >> 3) & 0x3fff_ffff)
    })
}
