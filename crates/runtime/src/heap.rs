//! Heap preciso com tracing iterativo, raízes explícitas e contadores observáveis.
//! Células, ambientes, closures e listas são infraestrutura: ainda não implicam
//! lowering Dart para LLVM. O contrato detalhado está em CONTRACT.md nesta crate.

/// Contadores cumulativos de trabalho; não representam bytes físicos do alocador.
#[derive(Debug, Clone, Copy, Default)]
pub struct HeapStats {
    pub allocations: u64,
    pub collections: u64,
    pub reclaimed: u64,
    pub roots_scanned: u64,
    pub slots_scanned: u64,
    pub live_objects: usize,
    pub reserved_slots: usize,
    /// Valores enum canônicos mantidos vivos até encerrar o runtime.
    pub permanent_roots: usize,
    pub live_roots: usize,
    pub peak_roots: usize,
    pub root_slots: usize,
    pub peak_root_slots: usize,
    /// Cabeçalhos vivos e capacidades dos payloads, sem metadados auxiliares/RSS.
    pub estimated_bytes: usize,
    pub peak_estimated_bytes: usize,
    /// `int` em posição `Ref` que viraram `Smi` (R10) em vez de caixa: sem o
    /// `Smi`, cada um seria uma alocação. `allocations + boxes_avoided` é a
    /// contagem de antes do R10, exata.
    pub boxes_avoided: u64,
}

// `Texto`, `StringMut`, `UnitsIter` e `push_point` moram em `textos.rs` (P1,
// docs/NATIVO-ESPACO-UNIFICADO.md §4.3).
pub use crate::textos::{UnitsIter, Texto, StringMut, push_point};

/// Um campo de objeto: os bits e se são referência (o que o coletor segue).
pub type Field = (i64, bool);

// O contrato de layout (docs/NATIVO-ESPACO-UNIFICADO.md §3.1) mora em
// `crate::layout`; o `heap` o reexporta para quem já o usava por aqui.
pub use crate::layout::{
    PAGE_HEAD, CAMPOS_EM_LINHA, Header, DESLOCAMENTO_DO_HANDLE, LARGEST_CLASS, PAGINA, Ref, TLAB_N,
    capacidade, is_object, palavras_do_mapa, smi,
};

// O cabeçalho ([`Header`], `crate::layout`) de um objeto do usuário no
// espaço de objetos ([`ObjectSpace`]) é seguido dos campos (8 bytes cada,
// os bits) e, com mais de 32 campos, das palavras que estendem o mapa de
// referências. Layout C fixo: contrato com o emissor (`llvm/mod.rs`).
//
// O handle do objeto é o endereço do cabeçalho com o bit 1 ligado (`bloco +
// 2`, [`DESLOCAMENTO_DO_HANDLE`]); os blocks são alinhados a 8 e o `Smi` é
// ímpar (R10), então `h & 7 == 2` ([`is_object`]) distingue o objeto sem
// consulta.
//
// Como na VM, o campo não carrega a marca de referência: o **mapa** (`map`,
// um bit por campo, e as palavras de extensão) diz quais campos o coletor
// segue — a VM tira isso da classe (o *unboxed fields bitmap*); aqui cada
// gravação acende ou apaga o bit do campo, então um campo que a representação
// faz às vezes `Ref` e às vezes escalar continua preciso.
//
// Um objeto cujos campos mudaram de número depois de criado (o erro que ganha
// o rastro, a recarga do JIT que muda o layout da classe, J03) não pode mudar
// de endereço: os campos vão para um **corpo de fora** (bit [`FORA`] em
// `flags`), com o mesmo layout (cabeçalho + campos + extensão), cujo endereço
// fica no lugar do primeiro campo. O código gerado lê o corpo de fora com uma
// seleção sem desvio (`llvm/mod.rs`).

/// Estados de um bloco ([`Header::estado`]; `crate::layout::estado`). O
/// coletor é geracional sem mover objetos (marcas "pegajosas"): um objeto nasce
/// [`JOVEM`] (a alocação em linha grava 1), e o que sobrevive a uma coleta vira
/// [`VELHO`]. A coleta menor só marca e varre os jovens; os velhos contam como
/// vivos, e as referências de velho para jovem são achadas pelos [`REMEMBERED`]s
/// — a barreira de escrita (`Heap::get_mut` e as gravações de campo do
/// runtime, e a do código gerado depois de gravar um `Ref` num campo,
/// `llvm/mod.rs`) marca o velho que recebe uma referência, como o *store
/// buffer* da VM. [`PERMANENTE`] é o objeto estático, fora do heap.
pub const LIVRE: u8 = crate::layout::estado::LIVRE;
pub const JOVEM: u8 = crate::layout::estado::JOVEM;
pub const MARKED: u8 = 2;
pub const VELHO: u8 = crate::layout::estado::VELHO;
pub const REMEMBERED: u8 = crate::layout::estado::REMEMBERED;
pub const PERMANENTE: u8 = crate::layout::estado::PERMANENTE;
/// Bit de [`Header::flags`]: os campos moram num corpo de fora.
pub const FORA: u8 = crate::layout::flags::FORA;

// [`PAGINA`] é o tamanho da página do espaço de objetos (e o alinhamento dela:
// a página de um handle é `h & !(PAGINA - 1)`). [`PAGE_HEAD`] são os
// bytes do começo de cada página com o mapa de marcas: um bit por palavra de 8
// bytes da página (o bit do bloco é o da palavra onde ele começa). A marcação
// acende o bit do objeto alcançado; as varreduras leem só os bits, sem tocar os
// blocos, vivos ou mortos (a VM também marca fora do objeto no *old space*,
// `marking.cc`, pelo bit no cabeçalho; aqui o mapa fica junto, por página,
// como o do Immix).

// O espaço de objetos (páginas, classes de tamanho, regiões grandes, mapa de
// marcas, cartões, anexos) mora em `crate::espaco`. [`LARGEST_CLASS`] e
// [`TLAB_N`] contam palavras do corpo (docs/NATIVO-ESPACO-UNIFICADO.md §2.6): a
// TLAB de `w` palavras é `Context::tlab[w]`, `w` em `1..=TLAB_N`.
pub use crate::espaco::{ObjectSpace, block_size};
use crate::espaco::{
    TLAB_BLOCKS, allocation_bytes, fields_of, body, is_reference, shape, marked, mark_block, mark_reference,
    new_out_of_line_body, body_refs, release_out_of_line_body,
};
use crate::layout::{bytes_do_bloco, flags as flags_do_bloco};

/// A coleta menor vem a cada `YOUNG_LIMIT` bytes alocados (o tamanho do
/// *new space*; o semiespaço da VM começa menor e cresce até 8 MiB em 64
/// bits — aqui fixo, entre o custo por coleta e o pico de memória; 2 MiB
/// cabem melhor no cache que 4: medido em `objetos_escapam`, menos tempo e
/// menos memória)…
const YOUNG_LIMIT: usize = 2 * 1024 * 1024;
/// Quanto o heap cresce até a próxima coleta completa, em % do que
/// sobreviveu à última: 150 (o dobro, 200, custava ~15 MB a mais de pico em
/// `objetos_escapam` sem ganho de tempo, já que a varredura só lê o mapa
/// de marcas).
const GROWTH: usize = 150;
/// … ou a cada `YOUNG_COUNT` alocações (valores pequenos do runtime).
const YOUNG_COUNT: usize = 256 * 1024;
/// No `--gc-stress`, uma coleta completa a cada tantas (as demais, menores:
/// é nelas que uma barreira faltando apareceria) …
const MINORS_PER_FULL_UNDER_STRESS: u64 = 8;
/// … ou, com muito dado vivo, a cada `work / WORK_PER_MINOR_UNDER_STRESS`
/// (o trabalho de marcação da última completa: objetos e palavras de
/// referência visitados): a completa custa o heap vivo inteiro, e uma a cada 8
/// alocações tornava o estresse quadrático (o programa 99, ~1 s sem estresse,
/// passava de 6 min; uma lista viva de 40 mil elementos custava 100 µs por
/// alocação). Assim o custo das completas por alocação fica em ~64 palavras
/// visitadas; a menor continua antes de toda alocação.
const WORK_PER_MINOR_UNDER_STRESS: usize = 64;

/// O objeto de ninguém: cabeçalho zerado e campos zerados. A leitura em
/// linha de um campo de algo que não é objeto do espaço lê daqui (o que
/// `dartforge_object_get` devolvia); nunca é gravado — está em memória só
/// de leitura, e gravar nela é erro do compilador que termina o processo.
#[repr(C)]
pub struct EmptyObject {
    pub header: Header,
    pub fields: [i64; CAMPOS_EM_LINHA],
}
pub static EMPTY_OBJECT: EmptyObject = EmptyObject {
    header: Header { estado: LIVRE, flags: 0, n: 0, class_id: 0, map: 0, metadata: 0 },
    fields: [0; CAMPOS_EM_LINHA],
};

/// A vista de um objeto do espaço para o runtime ([`Heap::object`]): a
/// classe e os campos, `(bits, é referência)`.
#[derive(Clone, Copy)]
pub struct Obj<'a> {
    body: *const Header,
    /// A classe do objeto.
    pub class_id: i64,
    _heap: std::marker::PhantomData<&'a Heap>,
}

#[allow(unsafe_code)]
impl Obj<'_> {
    /// Quantos campos.
    pub fn len(&self) -> usize {
        // SAFETY: corpo vivo enquanto o empréstimo do heap dura.
        usize::from(unsafe { (*self.body).n })
    }
    /// Sem campos?
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// O campo `i`: os bits e se é referência.
    ///
    /// # Panics
    /// Com `i` fora do objeto.
    pub fn field(&self, i: usize) -> Field {
        assert!(i < self.len(), "campo {i} fora do objeto de {} campos", self.len());
        // SAFETY: `i < n`; corpo vivo.
        unsafe { (*fields_of(self.body.cast_mut()).add(i), is_reference(self.body, i)) }
    }
    /// O campo `i`, se existe.
    pub fn get(&self, i: usize) -> Option<Field> {
        (i < self.len()).then(|| self.field(i))
    }
    /// O primeiro campo, se existe.
    pub fn first(&self) -> Option<Field> {
        self.get(0)
    }
    /// Os campos, em ordem.
    pub fn iter(&self) -> impl Iterator<Item = Field> + '_ {
        (0..self.len()).map(|i| self.field(i))
    }
    /// Os campos copiados.
    pub fn to_vec(&self) -> Vec<Field> {
        self.iter().collect()
    }
}

impl std::fmt::Debug for Obj<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Obj").field("class_id", &self.class_id).field("campos", &self.to_vec()).finish()
    }
}

/// O handle `h` foi marcado pela coleta em curso (vivo)? Null, `Smi` e
/// escalar qualquer: não.
///
/// Na coleta menor, o velho conta como vivo.
fn marked_in_collection(objects: &ObjectSpace, minor: bool, h: i64) -> bool {
    if is_object(h) {
        // SAFETY: bloco de uma página do espaço.
        #[allow(unsafe_code)]
        return match objects.block_of(h) {
            Some(b) => unsafe {
                if minor {
                    let e = (*b).estado;
                    e == VELHO || e == REMEMBERED
                } else {
                    marked(b)
                }
            },
            // O estático (seção da imagem) nunca morre.
            None => is_static(h),
        };
    }
    false
}

// `is_object` e o módulo `smi` (a etiqueta do `Ref`, R10) moram em
// `crate::layout` e são reexportados no começo deste módulo.

/// Um finalizador nativo e o dado dele (`Dart_NewFinalizableHandle`).
pub type Finalizer = (fn(usize), usize);

/// O quadro de raízes de uma função gerada (G1), no stack dela: o
/// anterior, o número de slots e os slots, que o código gerado grava com um
/// `store` comum (a pilha-sombra do LLVM, `ShadowStackGC`). O runtime só
/// encadeia e desencadeia o quadro ([`stack_frame`]) e o percorre na
/// coleta; os quadros do próprio runtime continuam em `Heap::frames`.
#[repr(C)]
pub struct RootFrame {
    previous: *const RootFrame,
    n: i64,
    slots: [i64; 0],
}

/// O contexto da thread que o código gerado lê e grava direto, sem
/// chamada (`llvm/mod.rs`): a exceção pendente (espelho de
/// `excecoes::EXCEPTION`, conferido depois de cada chamada) e o topo da
/// pilha-sombra desta thread (do isolado dela). O endereço vem de
/// `dartforge_contexto`, uma vez por ativação; não muda enquanto a
/// thread vive. Os deslocamentos são contrato com o emissor.
#[repr(C)]
pub struct Context {
    /// 1 com exceção pendente (deslocamento 0).
    pub pending: std::cell::Cell<u8>,
    /// O quadro do topo da pilha-sombra (deslocamento 8).
    pub top: std::cell::Cell<*const RootFrame>,
    /// A área de globais de cada módulo neste isolado, pelo id do módulo
    /// (`@df.area_id`; 0 = sem id): o caminho rápido do prólogo das
    /// funções (`@df.obter_area`). Endereço dos elementos (deslocamento 16)
    /// e comprimento (24); uma entrada nula vai ao runtime.
    pub areas: std::cell::Cell<*const *mut i64>,
    pub n_areas: std::cell::Cell<usize>,
    /// O pedido de interrupção do isolado (deslocamento 32): a porta de
    /// controle o liga, de outra thread, quando chega uma mensagem de controle
    /// (`ports.rs`). O código gerado o lê, atômico, no começo de cada volta
    /// de laço (o ponto seguro da J01).
    pub interrupt: std::sync::atomic::AtomicU8,
    /// O cabeçalho de [`EMPTY_OBJECT`] (deslocamento 40): a leitura em
    /// linha de um campo de algo que não é objeto do espaço usa este objeto
    /// no lugar do cabeçalho em `h - 2` (`llvm/mod.rs`), e lê zeros, como
    /// `dartforge_object_get` dava.
    pub empties: *const Header,
    /// As classes com tabela de métodos registrada (deslocamento 48; um
    /// byte 0/1 por id, `seletores.rs`) e quantas (56): a alocação em linha
    /// de uma classe que registra a tabela na primeira alocação
    /// (`dartforge_object_new_t`) só pula o runtime depois do registro.
    pub registered: std::cell::Cell<*const u8>,
    pub n_registered: std::cell::Cell<usize>,
    /// A TLAB do isolado (deslocamento 64): para cada número de campos
    /// `n ≤ TLAB_N`, uma faixa `[cursor, end)` de blocos livres contíguos do
    /// espaço de objetos já contados como alocação
    /// (`Heap::refill_tlab`): o cursor em `64 + 16n`, o fim em
    /// `72 + 16n`. O código gerado aloca avançando o cursor um bloco (o
    /// *bump pointer* da VM, `llvm/mod.rs`); faixa esgotada vai ao runtime,
    /// que reabastece.
    pub tlab: [[std::cell::Cell<*mut u8>; 2]; TLAB_N + 1],
    /// Os mapas de bits de subtipo (`seletores.rs`, fragmento `nucleo.rs`): para
    /// cada classe alvo `C` (índice; deslocamento 336), o endereço do mapa
    /// em que o bit `cid` diz se `cid <: C` (nulo = ainda não calculado), e
    /// quantos alvos (344); a largura de todos os mapas em bits (352): um
    /// `cid` fora dela vai ao runtime (`dartforge_is_subclass`, que também
    /// monta o mapa na primeira consulta). Um registro de subclasse novo
    /// (a carga, uma recarga) zera os mapas.
    pub subtypes: std::cell::Cell<*const *const u64>,
    pub n_subtypes: std::cell::Cell<usize>,
    pub subtype_width: std::cell::Cell<usize>,
    /// Os handles (endereço + 2) das caixas estáticas de `true` (deslocamento
    /// 360) e `false` (368) deste runtime: `@df.box_bool` e
    /// `@df.desencaixa_bool` os leem daqui, sem referenciar os dados
    /// `dartforge_verdadeiro`/`_falso` de outra imagem (o executável e a DLL do
    /// SDK têm cada um a sua cópia; só a do runtime ativo é a canônica).
    pub true_value: *const u8,
    pub false_value: *const u8,
    /// O limite da pilha nativa da thread (deslocamento 376): o prólogo de
    /// cada função que chama outra compara o endereço do seu quadro com ele
    /// e, abaixo, lança `StackOverflowError` em vez de estourar a pilha do
    /// sistema (o `stack_overflow_check` da VM). 0 = ainda não calculado
    /// (fragmento `gc_raizes.rs`, `dartforge_contexto`); 1 = sem limite conhecido.
    pub stack_limit: std::cell::Cell<usize>,
}

/// O `is C` lido em linha pelo código gerado ([`Context::subtypes`]). Para
/// medir, `DARTFORGE_SEM_CLASSE_EM_LINHA=1` não publica os mapas: o código
/// gerado vai sempre ao runtime (a resposta é a mesma).
pub fn inline_class() -> bool {
    static SIM: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *SIM.get_or_init(|| std::env::var_os("DARTFORGE_SEM_CLASSE_EM_LINHA").is_none())
}

const _: () = {
    use crate::layout::contexto as c;
    assert!(std::mem::offset_of!(Context, pending) == c::PENDING);
    assert!(std::mem::offset_of!(Context, top) == c::TOPO);
    assert!(std::mem::offset_of!(Context, areas) == c::AREAS);
    assert!(std::mem::offset_of!(Context, n_areas) == c::N_AREAS);
    assert!(std::mem::offset_of!(Context, interrupt) == c::INTERRUPT);
    assert!(std::mem::offset_of!(Context, empties) == c::EMPTIES);
    assert!(std::mem::offset_of!(Context, registered) == c::REGISTERED);
    assert!(std::mem::offset_of!(Context, n_registered) == c::N_REGISTERED);
    assert!(std::mem::offset_of!(Context, tlab) == c::TLAB);
    assert!(std::mem::offset_of!(Context, subtypes) == c::SUBTIPOS);
    assert!(std::mem::offset_of!(Context, n_subtypes) == c::N_SUBTIPOS);
    assert!(std::mem::offset_of!(Context, subtype_width) == c::LARGURA_SUBTIPOS);
    assert!(std::mem::offset_of!(Context, true_value) == c::TRUE);
    assert!(std::mem::offset_of!(Context, false_value) == c::FALSE);
    assert!(std::mem::offset_of!(Context, stack_limit) == c::LIMITE_DA_PILHA);
};

thread_local! {
    pub static CONTEXTO: Context = const {
        Context {
            pending: std::cell::Cell::new(0),
            top: std::cell::Cell::new(std::ptr::null()),
            areas: std::cell::Cell::new(std::ptr::null()),
            n_areas: std::cell::Cell::new(0),
            interrupt: std::sync::atomic::AtomicU8::new(0),
            empties: &raw const EMPTY_OBJECT.header,
            registered: std::cell::Cell::new(std::ptr::null()),
            n_registered: std::cell::Cell::new(0),
            tlab: [const { [std::cell::Cell::new(std::ptr::null_mut()), std::cell::Cell::new(std::ptr::null_mut())] }; TLAB_N + 1],
            subtypes: std::cell::Cell::new(std::ptr::null()),
            n_subtypes: std::cell::Cell::new(0),
            subtype_width: std::cell::Cell::new(0),
            true_value: (&raw const dartforge_verdadeiro).cast::<u8>().wrapping_add(DESLOCAMENTO_DO_HANDLE as usize),
            false_value: (&raw const dartforge_falso).cast::<u8>().wrapping_add(DESLOCAMENTO_DO_HANDLE as usize),
            stack_limit: std::cell::Cell::new(0),
        }
    };
}


/// Encadeia `q` no topo da pilha-sombra. `q.n` e os slots (zerados) já
/// foram escritos pelo código gerado.
///
/// # Safety
/// `q` aponta um quadro válido que vive até o [`unstack_frame`]
/// correspondente.
#[allow(unsafe_code)]
pub unsafe fn stack_frame(q: *mut RootFrame) {
    CONTEXTO.with(|c| {
        // SAFETY: `q` é o quadro no stack da função que chama, vivo até o
        // `unstack_frame` antes de cada retorno dela.
        unsafe { (*q).previous = c.top.get() };
        c.top.set(q);
    });
}

/// Desencadeia `q`, que tem de ser o topo (os retornos fecham os quadros
/// em ordem, G3).
///
/// # Safety
/// `q` é o quadro válido passado ao último [`stack_frame`].
#[allow(unsafe_code)]
pub unsafe fn unstack_frame(q: *const RootFrame) {
    CONTEXTO.with(|c| {
        assert!(std::ptr::eq(c.top.get(), q), "bug do compilador: quadro de raízes fechado fora de ordem");
        // SAFETY: `q` é o topo, ainda no stack de quem chama.
        c.top.set(unsafe { (*q).previous });
    });
}

/// O campo `n` de um quadro de raízes: o número de slots nos 40 bits de
/// baixo; nos de cima, quantos dos últimos são de conferência (o build de
/// conferência do percurso por mapas, `llvm/mod.rs`).
const FRAME_SLOTS: u64 = (1 << 40) - 1;

/// O bit 62 do campo `n`: o quadro está num quadro nativo próprio (o
/// programa sem otimização, sem inlining; `com_quadros_ordenados` do
/// emissor). Só entre dois quadros assim a ordem dos endereços é garantida.
const FRAME_ORDERED: u64 = 1 << 62;

/// Visita as raízes de todos os quadros da pilha-sombra desta thread.
///
/// Os quadros são `alloca` do código gerado na pilha nativa da thread, que
/// cresce para baixo: cada quadro anterior está num endereço maior. Um
/// anterior abaixo do atual é um quadro morto, de uma função que já saiu —
/// o topo que um desenrolamento deixou de restaurar (o pouso de
/// `--excecoes=tables` sem o `store` do topo, a sabotagem `pouso_sem_topo`
/// do §14.7) e que um quadro novo encadeou. Ler os slots dele seria ler
/// lixo; o processo encerra com o código 3. A conferência só vale entre dois
/// quadros com [`FRAME_ORDERED`]: com inlining (o código otimizado, o SDK),
/// o quadro da função embutida e o da que a chamou são `alloca` do mesmo
/// quadro nativo, em ordem de endereço qualquer.
#[allow(unsafe_code)]
fn visit_frames(mut f: impl FnMut(i64)) {
    let mut q = CONTEXTO.with(|c| c.top.get());
    while !q.is_null() {
        // SAFETY: cada quadro encadeado está no stack de uma função ainda
        // ativa desta thread, com `n` slots depois do cabeçalho (a ordem dos
        // endereços, conferida abaixo antes de seguir, o garante).
        unsafe {
            let n = ((*q).n as u64 & FRAME_SLOTS) as usize;
            let slots = std::ptr::addr_of!((*q).slots) as *const i64;
            for i in 0..n {
                f(*slots.add(i));
            }
            let previous = (*q).previous;
            if !previous.is_null()
                && (previous as usize) <= (q as usize)
                && (*q).n as u64 & FRAME_ORDERED != 0
                && (*previous).n as u64 & FRAME_ORDERED != 0
            {
                eprintln!("dartforge: quadro de raízes morto na pilha-sombra");
                std::process::exit(3);
            }
            q = previous;
        }
    }
}

// ---------------------------------------------------------------------------
// Raízes por mapas de pilha (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §14.8).
//
// Com `--root_list=maps` os valores vivos através de uma chamada não vão para a
// pilha-sombra: o LLVM (`rewrite-statepoints-for-gc`) os deixa num registrador
// preservado ou num slot do quadro nativo e descreve, para cada chamada, onde
// estão — a seção `.llvm_stackmaps` do objeto (`.llvm_st` na imagem). Na
// coleta o runtime percorre a pilha nativa da thread e, para cada quadro cujo
// endereço de retorno tem registro, lê os slots e os registradores (o valor
// que o desenrolador restaurou para aquele quadro). Os quadros sem registro
// (o runtime, C, o sistema, as funções que só têm pilha-sombra) são
// atravessados; a pilha-sombra continua sendo visitada como sempre.

/// O registrador DWARF do SP e o do FP do alvo, como o `.llvm_stackmaps`
/// os escreve (x86-64: RSP 7, RBP 6; aarch64: SP 31, x29 29).
#[cfg(target_arch = "aarch64")]
const DWARF_SP: u16 = 31;
#[cfg(target_arch = "aarch64")]
const DWARF_FP: u16 = 29;
#[cfg(not(target_arch = "aarch64"))]
const DWARF_SP: u16 = 7;
#[cfg(not(target_arch = "aarch64"))]
const DWARF_FP: u16 = 6;

/// Os registradores DWARF preservados através de uma chamada, pela
/// convenção do alvo (um `Register` do mapa fora deles é defeito: o valor
/// não sobreviveria à chamada). Windows x64: rbx, rsi, rdi, rbp, r12–r15;
/// SysV x86-64: rbx, rbp, r12–r15; aarch64: x19–x29.
#[cfg(target_arch = "aarch64")]
const PRESERVED: u32 = ((1 << 11) - 1) << 19;
#[cfg(all(not(target_arch = "aarch64"), windows))]
const PRESERVED: u32 = (1 << 3) | (1 << 4) | (1 << 5) | (1 << 6) | (0xF << 12);
#[cfg(all(not(target_arch = "aarch64"), not(windows)))]
const PRESERVED: u32 = (1 << 3) | (1 << 6) | (0xF << 12);

/// O tipo de uma raiz do mapa: slot sobre o SP, slot sobre o FP, ou
/// registrador (o valor é o número DWARF dele).
const ROOT_AT_SP: u8 = 0;
const ROOT_AT_FP: u8 = 1;
const ROOT_IN_REGISTER: u8 = 2;

/// Uma função com mapa de pilha numa imagem: o endereço do começo dela e
/// onde está a descrição, que só é decodificada na primeira consulta
/// (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §3.5 e §14.5, E3.2).
#[derive(Clone, Copy)]
struct FunctionWithMap {
    start: u64,
    /// DFGM: o começo do fluxo da função; `.llvm_stackmaps`: o primeiro
    /// registro dela.
    data: usize,
    /// `.llvm_stackmaps`: quantos registros e o vetor de constantes do blob
    /// (os `ConstIndex`); DFGM: `u32::MAX` e a versão do blob (1: só slots;
    /// 2: com a máscara dos registradores).
    n_llvm: u32,
    constants: usize,
}

/// Os registros de uma função, decodificados: (deslocamento do endereço de
/// retorno em relação ao começo da função, início em `slots`, número), em
/// ordem do deslocamento; e as raízes, (tipo, deslocamento em bytes do slot
/// ou número DWARF do registrador): [`ROOT_AT_SP`], [`ROOT_AT_FP`],
/// [`ROOT_IN_REGISTER`].
#[derive(Default)]
struct FunctionRecords {
    records: Vec<(u32, u32, u32)>,
    slots: Vec<(u8, i32)>,
}

/// Os mapas de uma imagem (o executável, a DLL do SDK).
struct StackMap {
    /// O que identifica a imagem no registro: a base no Windows, o começo
    /// da seção no ELF e no Mach-O.
    key: usize,
    /// Ordenadas pelo começo.
    functions: Vec<FunctionWithMap>,
    decoded: Vec<std::sync::OnceLock<FunctionRecords>>,
}

// SAFETY: os endereços guardados são de seções só de leitura da imagem,
// mapeadas enquanto o processo vive; nada é escrito por eles.
#[allow(unsafe_code)]
unsafe impl Send for StackMap {}
#[allow(unsafe_code)]
unsafe impl Sync for StackMap {}

impl StackMap {
    fn new(key: usize) -> Self {
        StackMap { key, functions: Vec::new(), decoded: Vec::new() }
    }

    /// Ordena o índice e prepara as decodificações preguiçosas. Duas
    /// entradas com o mesmo começo (o ICF do ligador juntou duas funções
    /// idênticas, de mapas idênticos) ficam numa só.
    fn do_close(&mut self) {
        self.functions.sort_by_key(|f| f.start);
        self.functions.dedup_by_key(|f| f.start);
        self.decoded = (0..self.functions.len()).map(|_| std::sync::OnceLock::new()).collect();
    }

    /// A função com o maior começo que não passa de `ret`.
    fn function_of(&self, ret: u64) -> Option<usize> {
        self.functions.partition_point(|f| f.start <= ret).checked_sub(1)
    }

    #[allow(unsafe_code)]
    fn records(&self, i: usize) -> &FunctionRecords {
        // SAFETY: `functions[i]` aponta para a descrição de uma função numa
        // seção mapeada (conferida no registro da imagem).
        self.decoded[i].get_or_init(|| unsafe { decode_function(self.functions[i]) })
    }

    /// Os slots da chamada da função `i` que retorna em `ret`.
    ///
    /// No Windows x64, em parte das chamadas, o gerador de código põe um
    /// `nop` depois do `call` (para o desenrolador do sistema) e o registro
    /// aponta depois dele (§14.5): o deslocamento seguinte também vale, só
    /// quando o byte no endereço de retorno é esse `nop`.
    #[allow(unsafe_code)]
    fn slots_of(&self, i: usize, ret: u64) -> Option<&[(u8, i32)]> {
        let r = self.records(i);
        let offset = u32::try_from(ret.checked_sub(self.functions[i].start)?).ok()?;
        let find_one = |d: u32| {
            r.records.binary_search_by_key(&d, |g| g.0).ok().map(|k| {
                let (_, begin, n) = r.records[k];
                &r.slots[begin as usize..(begin + n) as usize]
            })
        };
        find_one(offset).or_else(|| {
            // SAFETY: `ret` é um endereço de retorno da pilha desta
            // thread, um byte de código mapeado.
            let nop = cfg!(all(windows, target_arch = "x86_64")) && !sabotage("sem_nop") && unsafe { *(ret as *const u8) } == 0x90;
            if nop { find_one(offset + 1) } else { None }
        })
    }
}

static STACK_MAPS: std::sync::RwLock<Vec<StackMap>> = std::sync::RwLock::new(Vec::new());
/// Alguma imagem registrou mapas: a coleta percorre a pilha nativa.
static HAS_STACK_MAPS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Os números do percurso por mapas, do processo inteiro (§7.5, a prova de
/// que coletou): quadros nativos percorridos, raízes lidas de mapas e
/// raízes conferidas contra a pilha-sombra (`DARTFORGE_GC_PERCURSO`).
static FRAMES_WALKED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static MAP_ROOTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static ROOTS_CHECKED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// (quadros percorridos, raízes de mapa visitadas, raízes conferidas).
pub fn walk_counts() -> (u64, u64, u64) {
    use std::sync::atomic::Ordering::Relaxed;
    (FRAMES_WALKED.load(Relaxed), MAP_ROOTS.load(Relaxed), ROOTS_CHECKED.load(Relaxed))
}

/// Alguma imagem do processo registrou mapas de pilha.
pub fn has_stack_maps() -> bool {
    HAS_STACK_MAPS.load(std::sync::atomic::Ordering::Acquire)
}

/// Uma sabotagem de teste ligada (`DARTFORGE_SABOTAGEM=a,b,…`,
/// docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.3 e §14.7): cada teste
/// dirigido tem de falhar com a dele. As do runtime: `sem_nop` (o leitor
/// sem a regra do `nop`), `folga` (D8: a folga da pilha de 4 KiB) e
/// `sem_porta` (D9: as portas Rust → Dart não são registradas) e
/// `tipo_sem_raiz` (D12: o objeto `Type` canônico fica só no cache).
pub fn sabotage(name: &str) -> bool {
    static ENABLED: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    ENABLED
        .get_or_init(|| std::env::var("DARTFORGE_SABOTAGEM").unwrap_or_default().split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
        .iter()
        .any(|s| s == name)
}

/// `DARTFORGE_GC_PERCURSO=check` (§3.6, E2.5): cada coleta confere que
/// o percurso por mapas visitou as raízes que o build de conferência
/// (`DARTFORGE_RAIZES_CONFERIR=1` na compilação) gravou na pilha-sombra, e
/// uma anomalia do percurso aborta em vez de só encerrá-lo.
fn checked_walk() -> bool {
    static SIM: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *SIM.get_or_init(|| std::env::var("DARTFORGE_GC_PERCURSO").is_ok_and(|v| v == "conferir"))
}

/// Encerra o processo: um mapa que o runtime não sabe ler é uma raiz
/// perdida, e continuar seria liberar objeto vivo.
fn map_rejected(reason: &str) -> ! {
    eprintln!("dartforge: mapa de pilha recusado: {reason}");
    std::process::abort()
}

/// Registra os mapas de pilha da imagem PE carregada em `base` (uma vez por
/// imagem). O módulo que tem funções com raízes no mapa chama isto na
/// partida, com o `__ImageBase` da imagem em que foi ligado.
#[allow(unsafe_code)]
pub fn register_stack_map(base: usize) {
    if base == 0 {
        return;
    }
    let mut maps = STACK_MAPS.write().unwrap_or_else(|e| e.into_inner());
    if maps.iter().any(|m| m.key == base) {
        return;
    }
    // SAFETY: `base` é o começo de uma imagem PE carregada neste processo.
    let map = unsafe { read_image_map(base) };
    if map.functions.is_empty() {
        map_rejected("a imagem foi compilada com raízes por mapas e não tem a seção `.dfgcm` nem a `.llvm_st` (o ligador renomeia a seção quando há informação de depuração)");
    }
    HAS_STACK_MAPS.store(true, std::sync::atomic::Ordering::Release);
    maps.push(map);
}

/// Registra o mapa compacto de uma imagem ELF ou Mach-O: a seção `dfgcm`
/// (`__DATA_CONST,__dfgcm` no Mach-O) entre `start` e `end`, que o
/// ligador delimita (`__start_dfgcm`/`__stop_dfgcm`; `section$start$…`).
#[allow(unsafe_code)]
pub fn register_map_section(start: usize, end: usize) {
    if start == 0 || end <= start {
        return;
    }
    let mut maps = STACK_MAPS.write().unwrap_or_else(|e| e.into_inner());
    if maps.iter().any(|m| m.key == start) {
        return;
    }
    let mut map = StackMap::new(start);
    // SAFETY: `[start, end)` é a seção do mapa, mapeada e legível.
    unsafe { read_dfgm(start as *const u8, end - start, 0, &mut map) };
    map.do_close();
    if map.functions.is_empty() {
        map_rejected("a seção do mapa de pilha compacto está vazia numa imagem compilada com raízes por mapas");
    }
    HAS_STACK_MAPS.store(true, std::sync::atomic::Ordering::Release);
    maps.push(map);
}

/// Registra o mapa de pilha no formato do LLVM (`.llvm_stackmaps` versão 3)
/// de uma imagem Mach-O: a seção `__LLVM_STACKMAPS,__llvm_stackmaps` entre
/// `start` e `end` (`section$start$…`/`section$end$…`), com os endereços
/// das funções já ajustados pelo carregador. Seção vazia não é erro aqui: o
/// módulo que registra pode não ter função com mapa, e o `.no_dead_strip`
/// que guarda a seção vai em todo módulo que tem (`llvm/mod.rs`).
#[allow(unsafe_code)]
pub fn register_llvm_section(start: usize, end: usize) {
    if start == 0 || end <= start {
        return;
    }
    let mut maps = STACK_MAPS.write().unwrap_or_else(|e| e.into_inner());
    if maps.iter().any(|m| m.key == start) {
        return;
    }
    let mut map = StackMap::new(start);
    // SAFETY: `[start, end)` é a seção do mapa, mapeada e legível.
    unsafe { read_stackmaps(start as *const u8, end - start, &mut map) };
    map.do_close();
    if map.functions.is_empty() {
        return;
    }
    HAS_STACK_MAPS.store(true, std::sync::atomic::Ordering::Release);
    maps.push(map);
}

/// Tira do índice o mapa registrado com a chave `key` (o começo da seção):
/// o JIT o chama quando solta a memória de um objeto (uma geração aposentada
/// da recarga, docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §3.8). A
/// publicação de uma geração acontece sem quadro Dart na pilha, então nenhum
/// quadro em curso usa o mapa que sai.
pub fn unregister_map(key: usize) {
    let mut maps = STACK_MAPS.write().unwrap_or_else(|e| e.into_inner());
    maps.retain(|m| m.key != key);
}

/// Lê os mapas de pilha da imagem PE em `base`: a seção `.dfgcm` (o mapa
/// compacto, `emit_native/src/gcmap.rs`) e a `.llvm_st` (o `.llvm_stackmaps`
/// dos objetos que não passaram pelo conversor — os da LTO —, com o nome
/// cortado em 8 bytes).
///
/// # Safety
/// `base` é o começo de uma imagem PE mapeada.
#[allow(unsafe_code)]
unsafe fn read_image_map(base: usize) -> StackMap {
    let mut map = StackMap::new(base);
    // SAFETY: os cabeçalhos de uma imagem PE mapeada (DOS, NT, seções).
    unsafe {
        let b = base as *const u8;
        let nt = b.add(b.add(0x3c).cast::<u32>().read_unaligned() as usize);
        let n_sections = nt.add(6).cast::<u16>().read_unaligned() as usize;
        let optional = nt.add(20).cast::<u16>().read_unaligned() as usize;
        let sections = nt.add(24 + optional);
        for i in 0..n_sections {
            let s = sections.add(40 * i);
            let name = std::slice::from_raw_parts(s, 8);
            let size = s.add(8).cast::<u32>().read_unaligned() as usize;
            let rva = s.add(12).cast::<u32>().read_unaligned() as usize;
            if name == b".llvm_st" {
                read_stackmaps(b.add(rva), size, &mut map);
            } else if name == b".dfgcm\0\0" {
                read_dfgm(b.add(rva), size, base, &mut map);
            }
        }
    }
    map.do_close();
    map
}

/// O tamanho de um registro do `.llvm_stackmaps` v3 que começa em `r`
/// (cabeçalho, locais, enchimento, *live-outs*, enchimento).
///
/// # Safety
/// `r` é o começo de um registro dentro de uma seção mapeada.
#[allow(unsafe_code)]
unsafe fn llvm_record_end(r: *const u8) -> *const u8 {
    let align_up = |q: *const u8| ((q as usize + 7) & !7) as *const u8;
    // SAFETY: o formato do registro (§14.3).
    unsafe {
        let n_locals = r.add(14).cast::<u16>().read_unaligned() as usize;
        let q = align_up(r.add(16 + 12 * n_locals));
        let n_exits = q.add(2).cast::<u16>().read_unaligned() as usize;
        align_up(q.add(4 + 4 * n_exits))
    }
}

/// Indexa os blobs `.llvm_stackmaps` versão 3 de `[p, p + size)` (um
/// por objeto ligado, com zeros de enchimento entre eles): por função, o
/// começo e o primeiro registro. Os registros só são lidos na consulta.
///
/// # Safety
/// `[p, p + size)` é a seção, mapeada e legível.
#[allow(unsafe_code)]
unsafe fn read_stackmaps(mut p: *const u8, size: usize, map: &mut StackMap) {
    // SAFETY: todas as leituras ficam dentro da seção, pelo formato.
    unsafe {
        let end = p.add(size);
        while (p as usize) + 16 <= end as usize {
            if *p == 0 {
                p = p.add(1);
                continue;
            }
            if *p != 3 {
                map_rejected("versão do `.llvm_stackmaps` diferente de 3");
            }
            let n_functions = p.add(4).cast::<u32>().read_unaligned() as usize;
            let n_constants = p.add(8).cast::<u32>().read_unaligned() as usize;
            let functions = p.add(16);
            let constants = functions.add(24 * n_functions);
            let mut r = constants.add(8 * n_constants);
            for i in 0..n_functions {
                let address = functions.add(24 * i).cast::<u64>().read_unaligned();
                let n_records = functions.add(24 * i + 16).cast::<u64>().read_unaligned();
                if n_records > u64::from(u32::MAX - 1) {
                    map_rejected("função do `.llvm_stackmaps` com registros demais");
                }
                map.functions.push(FunctionWithMap { start: address, data: r as usize, n_llvm: n_records as u32, constants: constants as usize });
                for _ in 0..n_records {
                    r = llvm_record_end(r);
                }
                if r as usize > end as usize {
                    map_rejected("`.llvm_stackmaps` truncado");
                }
            }
            p = r;
        }
    }
}

/// Indexa os blobs DFGM v1 e v2 de `[p, p + size)` (o formato está em
/// `emit_native/src/gcmap.rs`; entre as contribuições dos objetos há zeros
/// de enchimento, em múltiplos de 4). O endereço de cada função vem do
/// índice conforme as bandeiras do blob: 0, deslocamento sobre `base` (o
/// `ADDR32NB` do COFF); 1, deslocamento de 32 bits sobre o próprio campo
/// (ELF); 2, endereço absoluto de 64 bits (Mach-O).
///
/// # Safety
/// `[p, p + size)` é a seção, mapeada e legível.
#[allow(unsafe_code)]
unsafe fn read_dfgm(mut p: *const u8, size: usize, base: usize, map: &mut StackMap) {
    // SAFETY: todas as leituras ficam dentro da seção, pelo formato.
    unsafe {
        let end = p.add(size);
        while (p as usize) + 16 <= end as usize {
            if std::slice::from_raw_parts(p, 4) != b"DFGM" {
                p = p.add(4);
                continue;
            }
            let version = *p.add(4);
            if version != 1 && version != 2 {
                map_rejected("versão do mapa compacto diferente de 1 e de 2");
            }
            let flag_bits = p.add(6).cast::<u16>().read_unaligned();
            let total = p.add(8).cast::<u32>().read_unaligned() as usize;
            let n_functions = p.add(12).cast::<u32>().read_unaligned() as usize;
            let input = if flag_bits & 2 != 0 { 16 } else { 8 };
            if total < 16 + input * n_functions || (p as usize) + total > end as usize || (flag_bits & 3 == 0 && base == 0) {
                map_rejected("mapa compacto com o cabeçalho errado");
            }
            let ix = p.add(16);
            let stream = ix.add(input * n_functions);
            for i in 0..n_functions {
                let e = ix.add(input * i);
                let (function_start, offset_in_stream) = if flag_bits & 2 != 0 {
                    (e.cast::<u64>().read_unaligned(), e.add(8).cast::<u32>().read_unaligned())
                } else if flag_bits & 1 != 0 {
                    let rel = i64::from(e.cast::<i32>().read_unaligned());
                    ((e as i64).wrapping_add(rel) as u64, e.add(4).cast::<u32>().read_unaligned())
                } else {
                    (base as u64 + u64::from(e.cast::<u32>().read_unaligned()), e.add(4).cast::<u32>().read_unaligned())
                };
                map.functions.push(FunctionWithMap {
                    start: function_start,
                    data: stream.add(offset_in_stream as usize) as usize,
                    n_llvm: u32::MAX,
                    constants: usize::from(version),
                });
            }
            p = p.add(total);
        }
    }
}

/// Decodifica os registros de uma função (a primeira consulta a ela).
///
/// # Safety
/// `f` vem de [`read_stackmaps`] ou [`read_dfgm`] sobre uma seção mapeada.
#[allow(unsafe_code)]
unsafe fn decode_function(f: FunctionWithMap) -> FunctionRecords {
    let mut d = FunctionRecords::default();
    // SAFETY: as leituras seguem o formato a partir de `f.data`.
    unsafe {
        if f.n_llvm == u32::MAX {
            let mut q = f.data as *const u8;
            let varint = |q: &mut *const u8| -> u64 {
                let mut r = 0u64;
                let mut s = 0u32;
                loop {
                    let b = **q;
                    *q = q.add(1);
                    if s < 64 {
                        r |= u64::from(b & 0x7f) << s;
                    }
                    s += 7;
                    if b & 0x80 == 0 {
                        return r;
                    }
                }
            };
            let n_records = varint(&mut q);
            let _frame = varint(&mut q);
            let mut ret = 0u64;
            // (início em `slots`, número) do conjunto do registro anterior.
            let mut previous = (0u32, 0u32);
            for _ in 0..n_records {
                ret += varint(&mut q);
                let c = varint(&mut q);
                if c & 1 == 0 {
                    let first = d.slots.len() as u32;
                    let mut slot = 0i64;
                    for _ in 0..(c >> 1) {
                        let s = varint(&mut q);
                        let u = s >> 1;
                        slot += ((u >> 1) as i64) ^ -((u & 1) as i64);
                        d.slots.push((if s & 1 == 1 { ROOT_AT_FP } else { ROOT_AT_SP }, (slot * 8) as i32));
                    }
                    // v2: a máscara dos registradores com raiz.
                    if f.constants >= 2 {
                        let mask = varint(&mut q);
                        if mask & !u64::from(PRESERVED) != 0 {
                            map_rejected("registrador não preservado com raiz no mapa compacto");
                        }
                        for r in 0..32 {
                            if mask & (1 << r) != 0 {
                                d.slots.push((ROOT_IN_REGISTER, r));
                            }
                        }
                    }
                    previous = (first, d.slots.len() as u32 - first);
                }
                if ret > u64::from(u32::MAX) {
                    map_rejected("deslocamento de retorno fora de 32 bits no mapa compacto");
                }
                d.records.push((ret as u32, previous.0, previous.1));
            }
        } else {
            let mut r = f.data as *const u8;
            for _ in 0..f.n_llvm {
                let offset = r.add(8).cast::<u32>().read_unaligned();
                let n_locals = r.add(14).cast::<u16>().read_unaligned() as usize;
                let locals = r.add(16);
                if n_locals < 3 {
                    map_rejected("registro de statepoint sem os três locais iniciais");
                }
                let n_deopt = locals.add(2 * 12 + 8).cast::<i32>().read_unaligned().max(0) as usize;
                let start = d.slots.len();
                if 3 + n_deopt > n_locals {
                    map_rejected("registro de statepoint com mais locais de `deopt` que locais");
                }
                // As raízes: cada local do `"deopt"` (a forma do §14.12) e a
                // base de cada par (base, derivado) do `"gc-live"`.
                for j in (3..3 + n_deopt).chain((3 + n_deopt..n_locals.saturating_sub(1)).step_by(2)) {
                    let l = locals.add(12 * j);
                    let tipo = *l;
                    let width = l.add(2).cast::<u16>().read_unaligned();
                    let register = l.add(4).cast::<u16>().read_unaligned();
                    let value = l.add(8).cast::<i32>().read_unaligned();
                    // Constante: null não é raiz, nem o `undef` que o
                    // otimizador deixou num operando (0xFEFEFEFE); outra
                    // constante par não nula seria um valor bruto tratado
                    // como referência (§3.5).
                    if tipo == 4 || tipo == 5 {
                        let c = if tipo == 4 {
                            i64::from(value)
                        } else {
                            (f.constants as *const u8).add(8 * value.max(0) as usize).cast::<i64>().read_unaligned()
                        };
                        if c != 0 && c & 1 == 0 && c != 0xFEFE_FEFE {
                            map_rejected("constante par não nula como raiz no mapa de pilha");
                        }
                        continue;
                    }
                    let slot = if tipo == 1 && width == 8 && register < 32 && PRESERVED & (1 << register) != 0 {
                        (ROOT_IN_REGISTER, i32::from(register))
                    } else if tipo == 3 && width == 8 && (register == DWARF_SP || register == DWARF_FP) {
                        (if register == DWARF_FP { ROOT_AT_FP } else { ROOT_AT_SP }, value)
                    } else {
                        map_rejected("local que não é `Indirect [SP|FP + d]` nem `Register` preservado de 8 bytes")
                    };
                    if !d.slots[start..].contains(&slot) {
                        d.slots.push(slot);
                    }
                }
                d.records.push((offset, start as u32, (d.slots.len() - start) as u32));
                r = llvm_record_end(r);
            }
            d.records.sort_unstable_by_key(|g| g.0);
        }
    }
    if d.records.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        map_rejected("dois registros do mapa de pilha no mesmo endereço de retorno");
    }
    d
}

/// Visita as raízes do quadro nativo parado no endereço de retorno
/// `ret`, com o SP do quadro no ponto da chamada em `sp` e o FP em `fp`;
/// `register` dá o valor de um registrador preservado (pelo número
/// DWARF) naquele quadro, como o desenrolador o restaurou.
/// `exact_start` é o começo da função como o desenrolador o conhece: se
/// ela tem mapa e o endereço não tem registro, é defeito do emissor ou do
/// conversor, e o processo aborta. Devolve se o quadro tinha mapa.
#[allow(unsafe_code)]
fn visit_frame(
    maps: &[StackMap],
    exact_start: Option<u64>,
    ret: u64,
    sp: u64,
    fp: u64,
    register: &dyn Fn(u16) -> u64,
    f: &mut dyn FnMut(i64),
) -> bool {
    for m in maps {
        let Some(i) = m.function_of(ret) else { continue };
        match m.slots_of(i, ret) {
            Some(slots) => {
                for &(tipo, d) in slots {
                    if tipo == ROOT_IN_REGISTER {
                        f(register(d as u16) as i64);
                        continue;
                    }
                    let b = if tipo == ROOT_AT_FP { fp } else { sp };
                    // SAFETY: o slot é do quadro de uma função ainda ativa,
                    // parada na chamada que o registro descreve.
                    f(unsafe { *((b as i64).wrapping_add(i64::from(d)) as *const i64) });
                }
                MAP_ROOTS.fetch_add(slots.len() as u64, std::sync::atomic::Ordering::Relaxed);
                return true;
            }
            None => {
                if exact_start == Some(m.functions[i].start) {
                    map_rejected("função com mapa sem registro exato para o endereço de retorno");
                }
            }
        }
    }
    false
}

/// Uma anomalia do percurso (SP que não cresce, fora da pilha): encerra o
/// percurso; com `DARTFORGE_GC_PERCURSO=check`, aborta.
fn walk_anomaly(reason: &str) {
    if checked_walk() {
        map_rejected(&format!("anomalia no percurso da pilha: {reason}"));
    }
}

/// Visita as raízes dos quadros nativos desta thread que têm mapa de pilha:
/// desenrola a pilha a partir daqui (`RtlVirtualUnwind`, sem chamar
/// tratadores) e, em cada quadro, lê os slots do registro do endereço de
/// retorno. Sem mapa registrado, não faz nada.
#[cfg(all(windows, target_arch = "x86_64"))]
#[allow(unsafe_code)]
#[inline(never)]
fn visit_frames_by_maps(mut f: impl FnMut(i64)) {
    if !HAS_STACK_MAPS.load(std::sync::atomic::Ordering::Acquire) {
        return;
    }
    /// O `CONTEXT` do x86-64: 1232 bytes, alinhado a 16. São lidos o `Rip`
    /// (0xF8) e os inteiros de `Rax` (0x78) a `R15` (0xF0).
    #[repr(C, align(16))]
    struct CpuContext([u8; 1232]);
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn RtlCaptureContext(contexto: *mut CpuContext);
        fn RtlLookupFunctionEntry(pc: u64, base: *mut u64, history: *mut u8) -> *const u8;
        fn RtlVirtualUnwind(
            tipo: u32,
            base: u64,
            pc: u64,
            function: *const u8,
            contexto: *mut CpuContext,
            data: *mut *mut u8,
            frame: *mut u64,
            pointers: *mut u8,
        ) -> *mut u8;
        fn GetCurrentThreadStackLimits(low: *mut usize, high: *mut usize);
    }
    let maps = STACK_MAPS.read().unwrap_or_else(|e| e.into_inner());
    let mut contexto = CpuContext([0; 1232]);
    let field = |c: &CpuContext, d: usize| u64::from_le_bytes(c.0[d..d + 8].try_into().expect("oito bytes"));
    /// O deslocamento no `CONTEXT` de cada registrador DWARF 0–15 (rax,
    /// rdx, rcx, rbx, rsi, rdi, rbp, rsp, r8–r15).
    const IN_CONTEXT: [usize; 16] = [0x78, 0x88, 0x80, 0x90, 0xA8, 0xB0, 0xA0, 0x98, 0xB8, 0xC0, 0xC8, 0xD0, 0xD8, 0xE0, 0xE8, 0xF0];
    let mut frames = 0u64;
    // SAFETY: o contexto é o desta thread, capturado aqui; cada passo
    // desenrola um quadro com a tabela de desenrolamento da função dele, e
    // para quando o SP deixa de crescer ou sai da pilha da thread. Os slots
    // lidos são do quadro de uma função ainda ativa, parada na chamada que o
    // registro descreve.
    unsafe {
        RtlCaptureContext(&mut contexto);
        let (mut low, mut high) = (0usize, 0usize);
        GetCurrentThreadStackLimits(&mut low, &mut high);
        let mut pc = field(&contexto, 0xF8);
        let mut base = 0u64;
        let mut function = RtlLookupFunctionEntry(pc, &mut base, std::ptr::null_mut());
        while !function.is_null() {
            let sp_before = field(&contexto, 0x98);
            let mut data: *mut u8 = std::ptr::null_mut();
            let mut frame = 0u64;
            RtlVirtualUnwind(0, base, pc, function, &mut contexto, &mut data, &mut frame, std::ptr::null_mut());
            // Agora o contexto é o de quem chamou: `Rip` é o endereço de
            // retorno nele e `Rsp`, o SP dele no ponto da chamada.
            let (ret, sp, fp) = (field(&contexto, 0xF8), field(&contexto, 0x98), field(&contexto, 0xA0));
            if ret == 0 {
                break;
            }
            if sp <= sp_before || (sp as usize) < low || (sp as usize) >= high {
                walk_anomaly("o SP não cresce ou saiu da pilha da thread");
                break;
            }
            frames += 1;
            // A função de quem chamou: o começo exato (o `BeginAddress` da
            // entrada do `.pdata`) e a entrada, para o passo seguinte.
            let mut next_base = 0u64;
            let next_one = RtlLookupFunctionEntry(ret, &mut next_base, std::ptr::null_mut());
            let start = (!next_one.is_null()).then(|| next_base + u64::from(next_one.cast::<u32>().read_unaligned()));
            // Os preservados no contexto são os de quem chamou, no ponto da
            // chamada: o desenrolamento dos quadros de dentro os restaurou.
            let register = |r: u16| IN_CONTEXT.get(usize::from(r)).map_or(0, |&d| field(&contexto, d));
            visit_frame(&maps, start, ret, sp, fp, &register, &mut f);
            pc = ret;
            base = next_base;
            function = next_one;
        }
    }
    FRAMES_WALKED.fetch_add(frames, std::sync::atomic::Ordering::Relaxed);
}

/// O ponteiro de pilha de um quadro no ponto da chamada que ele fez, no
/// passo do `_Unwind_Backtrace`: `cfa` é o `_Unwind_GetCFA` do próprio quadro
/// e `callee_cfa`, o do quadro anterior do percurso (o que ele chamou).
/// A libgcc (Linux) devolve no `_Unwind_GetCFA` o CFA do quadro — o SP do
/// chamador no ponto da chamada —, então o SP deste quadro é o CFA do
/// chamado. A libunwind do LLVM (macOS) devolve o SP do próprio quadro, já
/// restaurado: é ele. Usar o do chamado no macOS lia os slots `[SP + d]` do
/// mapa no quadro de dentro (raízes por mapas na produção, em que o LTO do
/// `ld64.lld` deixa as raízes em slots) e desalinhava o rastro simbólico.
#[cfg(all(unix, any(target_arch = "x86_64", target_arch = "aarch64")))]
fn frame_top(cfa: usize, callee_cfa: usize) -> usize {
    if cfg!(target_vendor = "apple") { cfa } else { callee_cfa }
}

/// O mesmo nos alvos do desenrolador Itanium (Linux e macOS, x86-64 e
/// aarch64; §3.6): `_Unwind_Backtrace` entrega os quadros de dentro para
/// fora; o SP de um quadro no ponto da chamada é o CFA do quadro que ele
/// chamou (o anterior no percurso), o FP é o registrador restaurado nele, e
/// o começo da função é o da região do FDE. O runtime e o código gerado têm
/// de ter tabelas de desenrolamento (`uwtable`; `-C
/// force-unwind-tables=yes` no runtime), senão o percurso vê zero quadros.
#[cfg(all(unix, any(target_arch = "x86_64", target_arch = "aarch64")))]
#[allow(unsafe_code)]
#[inline(never)]
fn visit_frames_by_maps(mut f: impl FnMut(i64)) {
    if !HAS_STACK_MAPS.load(std::sync::atomic::Ordering::Acquire) {
        return;
    }
    unsafe extern "C" {
        fn _Unwind_Backtrace(step: unsafe extern "C" fn(*mut u8, *mut u8) -> i32, argument: *mut u8) -> i32;
        fn _Unwind_GetIP(contexto: *mut u8) -> usize;
        fn _Unwind_GetCFA(contexto: *mut u8) -> usize;
        fn _Unwind_GetGR(contexto: *mut u8, register: i32) -> usize;
        fn _Unwind_GetRegionStart(contexto: *mut u8) -> usize;
    }
    /// `_URC_NO_REASON` (continua) e `_URC_NORMAL_STOP` (para).
    const CONTINUE: i32 = 0;
    const PARA: i32 = 4;
    struct State<'a> {
        maps: &'a [StackMap],
        f: &'a mut dyn FnMut(i64),
        /// O CFA do quadro anterior do percurso (o chamado).
        previous_cfa: Option<usize>,
        frames: u64,
    }
    unsafe extern "C" fn step(contexto: *mut u8, argument: *mut u8) -> i32 {
        // SAFETY: `argument` é o `State` abaixo, vivo durante o percurso;
        // `contexto` é o do desenrolador, válido durante a chamada.
        unsafe {
            let e = &mut *argument.cast::<State<'_>>();
            let ip = _Unwind_GetIP(contexto) as u64;
            if ip == 0 {
                return PARA;
            }
            let cfa = _Unwind_GetCFA(contexto);
            if let Some(previous) = e.previous_cfa {
                if cfa < previous {
                    walk_anomaly("o CFA não cresce");
                    return PARA;
                }
                let sp = frame_top(cfa, previous);
                e.frames += 1;
                let start = _Unwind_GetRegionStart(contexto) as u64;
                let fp = _Unwind_GetGR(contexto, i32::from(DWARF_FP)) as u64;
                // O fecho herda o bloco `unsafe` acima: o contexto do
                // desenrolador é válido durante o passo.
                let register = |r: u16| _Unwind_GetGR(contexto, i32::from(r)) as u64;
                visit_frame(e.maps, (start != 0).then_some(start), ip, sp as u64, fp, &register, e.f);
            }
            e.previous_cfa = Some(cfa);
            CONTINUE
        }
    }
    let maps = STACK_MAPS.read().unwrap_or_else(|e| e.into_inner());
    let mut estado = State { maps: &maps, f: &mut f, previous_cfa: None, frames: 0 };
    // SAFETY: o desenrolador percorre a pilha desta thread; `step` só lê.
    unsafe {
        _Unwind_Backtrace(step, std::ptr::addr_of_mut!(estado).cast::<u8>());
    }
    FRAMES_WALKED.fetch_add(estado.frames, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(not(any(all(windows, target_arch = "x86_64"), all(unix, any(target_arch = "x86_64", target_arch = "aarch64")))))]
fn visit_frames_by_maps(_f: impl FnMut(i64)) {}

/// Os endereços de retorno da pilha desta thread, de dentro para fora, com o
/// começo da função de cada um (0 quando o desenrolador não o dá) e o topo
/// da pilha do quadro em que o endereço está (o ponteiro de pilha dele na
/// chamada; 0 quando não se sabe), até `maximum` quadros: o rastro simbólico
/// (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13.14) os guarda no `throw` e
/// só os simboliza quando o texto é pedido; o topo casa o quadro com a pilha
/// do rastro (a cadeia de quem espera).
/// O mesmo percurso de `visit_frames_by_maps`, sem ler raízes: não
/// depende dos mapas, e vale nos dois modos de raízes.
#[cfg(all(windows, target_arch = "x86_64"))]
#[allow(unsafe_code)]
#[inline(never)]
pub fn return_addresses(maximum: usize) -> Vec<(u64, u64, u64)> {
    /// O `CONTEXT` do x86-64 (veja `visit_frames_by_maps`).
    #[repr(C, align(16))]
    struct CpuContext([u8; 1232]);
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn RtlCaptureContext(contexto: *mut CpuContext);
        fn RtlLookupFunctionEntry(pc: u64, base: *mut u64, history: *mut u8) -> *const u8;
        fn RtlVirtualUnwind(
            tipo: u32,
            base: u64,
            pc: u64,
            function: *const u8,
            contexto: *mut CpuContext,
            data: *mut *mut u8,
            frame: *mut u64,
            pointers: *mut u8,
        ) -> *mut u8;
        fn GetCurrentThreadStackLimits(low: *mut usize, high: *mut usize);
    }
    let mut out = Vec::new();
    let mut contexto = CpuContext([0; 1232]);
    let field = |c: &CpuContext, d: usize| u64::from_le_bytes(c.0[d..d + 8].try_into().expect("oito bytes"));
    // SAFETY: como em `visit_frames_by_maps`: o contexto é o desta
    // thread, cada passo desenrola um quadro com a tabela da função dele, e o
    // percurso para quando o SP deixa de crescer ou sai da pilha.
    unsafe {
        RtlCaptureContext(&mut contexto);
        let (mut low, mut high) = (0usize, 0usize);
        GetCurrentThreadStackLimits(&mut low, &mut high);
        let mut pc = field(&contexto, 0xF8);
        let mut base = 0u64;
        let mut function = RtlLookupFunctionEntry(pc, &mut base, std::ptr::null_mut());
        while !function.is_null() && out.len() < maximum {
            let sp_before = field(&contexto, 0x98);
            let mut data: *mut u8 = std::ptr::null_mut();
            let mut frame = 0u64;
            RtlVirtualUnwind(0, base, pc, function, &mut contexto, &mut data, &mut frame, std::ptr::null_mut());
            let (ret, sp) = (field(&contexto, 0xF8), field(&contexto, 0x98));
            if ret == 0 || sp <= sp_before || (sp as usize) < low || (sp as usize) >= high {
                break;
            }
            let mut next_base = 0u64;
            let next_one = RtlLookupFunctionEntry(ret, &mut next_base, std::ptr::null_mut());
            let start = if next_one.is_null() { 0 } else { next_base + u64::from(next_one.cast::<u32>().read_unaligned()) };
            out.push((ret, start, sp));
            pc = ret;
            base = next_base;
            function = next_one;
        }
    }
    out
}

/// O mesmo nos alvos do desenrolador Itanium, por `_Unwind_Backtrace`: o
/// IP de cada quadro depois do primeiro é o endereço de retorno, e o começo
/// da função é o da região do FDE.
#[cfg(all(unix, any(target_arch = "x86_64", target_arch = "aarch64")))]
#[allow(unsafe_code)]
#[inline(never)]
pub fn return_addresses(maximum: usize) -> Vec<(u64, u64, u64)> {
    unsafe extern "C" {
        fn _Unwind_Backtrace(step: unsafe extern "C" fn(*mut u8, *mut u8) -> i32, argument: *mut u8) -> i32;
        fn _Unwind_GetIP(contexto: *mut u8) -> usize;
        fn _Unwind_GetRegionStart(contexto: *mut u8) -> usize;
        fn _Unwind_GetCFA(contexto: *mut u8) -> usize;
    }
    /// `_URC_NO_REASON` (continua) e `_URC_NORMAL_STOP` (para).
    const CONTINUE: i32 = 0;
    const PARA: i32 = 4;
    struct State {
        out: Vec<(u64, u64, u64)>,
        maximum: usize,
        /// O CFA do quadro anterior (mais fundo): o topo da pilha deste.
        previous_cfa: u64,
    }
    unsafe extern "C" fn step(contexto: *mut u8, argument: *mut u8) -> i32 {
        // SAFETY: `argument` é o `State` abaixo, vivo durante o percurso;
        // `contexto` é o do desenrolador, válido durante a chamada.
        unsafe {
            let e = &mut *argument.cast::<State>();
            let ip = _Unwind_GetIP(contexto) as u64;
            if ip == 0 || e.out.len() >= e.maximum {
                return PARA;
            }
            let cfa = _Unwind_GetCFA(contexto) as u64;
            let top = if e.out.is_empty() { e.previous_cfa } else { frame_top(cfa as usize, e.previous_cfa as usize) as u64 };
            e.out.push((ip, _Unwind_GetRegionStart(contexto) as u64, top));
            e.previous_cfa = cfa;
            CONTINUE
        }
    }
    let mut estado = State { out: Vec::new(), maximum, previous_cfa: 0 };
    // SAFETY: o desenrolador percorre a pilha desta thread; `step` só lê.
    unsafe {
        _Unwind_Backtrace(step, std::ptr::addr_of_mut!(estado).cast::<u8>());
    }
    estado.out
}

#[cfg(not(any(all(windows, target_arch = "x86_64"), all(unix, any(target_arch = "x86_64", target_arch = "aarch64")))))]
pub fn return_addresses(_max: usize) -> Vec<(u64, u64, u64)> {
    Vec::new()
}

/// O build de conferência (§15.2, E2.5): os valores dos slots de
/// conferência dos quadros da pilha-sombra desta thread (os últimos `k`
/// slots de um quadro cujo campo `n` traz `k` nos bits 40 em diante).
#[allow(unsafe_code)]
fn visit_check_slots(mut f: impl FnMut(i64)) {
    let mut q = CONTEXTO.with(|c| c.top.get());
    while !q.is_null() {
        // SAFETY: cada quadro encadeado está no stack de uma função ainda
        // ativa desta thread (como em `visit_frames`).
        unsafe {
            let n = (*q).n as u64;
            let total = (n & FRAME_SLOTS) as usize;
            let k = ((n & !FRAME_ORDERED) >> 40) as usize;
            let slots = std::ptr::addr_of!((*q).slots) as *const i64;
            for i in total.saturating_sub(k)..total {
                f(*slots.add(i));
            }
            q = (*q).previous;
        }
    }
}

/// `DARTFORGE_GC_PERCURSO=check`: toda raiz que o build de conferência
/// gravou na pilha-sombra tem de estar entre as que o percurso por mapas
/// visitou (`from_maps`). Uma que falte é raiz que o mapa perdeu.
#[allow(unsafe_code)]
fn check_walk(from_maps: &[i64]) {
    let seen_set: crate::hash::HashSet<i64> = from_maps.iter().copied().collect();
    let mut checked = 0u64;
    visit_check_slots(|h| {
        // Só os objetos do heap: um `Smi` (ou o nulo) no slot não é raiz, e
        // o objeto estático da imagem (`PERMANENTE`) nunca é coletado — o
        // compilador o tira do `"deopt"` (`remove_statics_from_deopt`, §14.12).
        if !crate::layout::is_object(h) {
            return;
        }
        // SAFETY: `h` é o handle de um objeto que o código gerado gravou no
        // slot de conferência, vivo neste ponto de coleta.
        if unsafe { (*((h - DESLOCAMENTO_DO_HANDLE) as *const Header)).estado } == PERMANENTE {
            return;
        }
        checked += 1;
        if !seen_set.contains(&h) {
            map_rejected(&format!("conferência do percurso: a raiz {h:#x} está viva na pilha-sombra de conferência e o percurso por mapas não a visitou"));
        }
    });
    ROOTS_CHECKED.fetch_add(checked, std::sync::atomic::Ordering::Relaxed);
}

/// O heap de um isolado: o espaço de objetos (todo valor do runtime é um bloco
/// dele, docs/NATIVO-ESPACO-UNIFICADO.md), as raízes, as tabelas laterais por
/// handle e a coleta.
/// A coleta agendada por semente (`DARTFORGE_GC_AGENDA=<seed>,<rate>`,
/// docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.5): uma coleta em uma de cada
/// `rate` alocações, sorteadas por um gerador fixo a partir da semente. A
/// mesma semente e o mesmo programa coletam nos mesmos pontos: acha o defeito
/// que o `--gc-stress` total esconde por mudar o tempo, e o reproduz. Como
/// no estresse, sem TLAB (toda alocação passa pelo runtime).
#[derive(Debug)]
struct Schedule {
    estado: std::cell::Cell<u64>,
    rate: u64,
    /// O sorteio já saiu e a coleta ainda não veio: a próxima consulta
    /// responde o mesmo (o caminho lento consulta de novo).
    pending: std::cell::Cell<bool>,
}

impl Schedule {
    fn from_env() -> Option<Schedule> {
        let v = std::env::var("DARTFORGE_GC_AGENDA").ok()?;
        let (seed, rate) = v.split_once(',')?;
        let seed: u64 = seed.trim().parse().ok()?;
        let rate: u64 = rate.trim().parse().ok().filter(|&t| t > 0)?;
        let estado = seed ^ 0x9E37_79B9_7F4A_7C15;
        Some(Schedule { estado: std::cell::Cell::new(if estado == 0 { 1 } else { estado }), rate, pending: std::cell::Cell::new(false) })
    }

    /// Esta alocação coleta? (O xorshift64*.)
    fn draws(&self) -> bool {
        if self.pending.get() {
            return true;
        }
        let mut x = self.estado.get();
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.estado.set(x);
        let yes = x.wrapping_mul(0x2545_F491_4F6C_DD1D) % self.rate == 0;
        self.pending.set(yes);
        yes
    }
}

#[derive(Debug)]
pub struct Heap {
    /// Identificador, slots e categoria: `true` conta cada ocorrência no ARC.
    frames: Vec<(i64, Vec<i64>, bool)>,
    next_frame: i64,
    allocations: usize,
    stress: bool,
    /// `DARTFORGE_GC_AGENDA`; `None` sem ela.
    schedule: Option<Schedule>,
    stats: HeapStats,
    /// Posições percorridas pela última marcação.
    marking_work: usize,
    pending: Vec<i64>,
    byte_threshold: usize,
    /// Teto DURO do heap, em bytes.
    ///
    /// Nao confundir com `byte_threshold`, que e o gatilho de COLETA. Este e o
    /// limite do processo: um programa em laco infinito que aloca strings come
    /// a memoria da maquina inteira antes de qualquer tempo-limite do harness
    /// disparar (foi o que travou a maquina rodando o corpus nativo em
    /// paralelo). Ao estourar, o processo sai com mensagem legivel e codigo
    /// 255, que e o que a VM usa para excecao nao capturada — vira uma falha
    /// no placar em vez de um travamento.
    ///
    /// Padrao 256 MiB; `DARTFORGE_HEAP_MAX_MB` ajusta, e 0 desliga o teto.
    byte_limit: usize,
    enum_values: crate::hash::HashMap<(i64, i64), i64>,
    /// Tear-offs canônicos de funções top-level, por ID de código.
    ///
    /// O oráculo Dart 3.6.2 exige `identical(f, f)` verdadeiro para dois
    /// tear-offs da mesma função top-level; cada `code_id` tem um único handle,
    /// mantido vivo como raiz permanente, como os singletons de enum.
    tearoffs: crate::hash::HashMap<i64, i64>,
    /// Literais do compilador, canônicos por unidades UTF-16. Permanecem
    /// enraizados pelo isolate; strings criadas em execução não entram aqui.
    literals: crate::hash::HashMap<Vec<u16>, i64>,
    /// Objetos permanentes e imutáveis que uma mensagem entre portas do
    /// mesmo isolado passa pela identidade, sem copiar (a VM compartilha os
    /// profundamente imutáveis): constantes canônicas, valores de enum e
    /// globais `const` (o código gerado os marca, `dartforge_marcar_permanente`),
    /// tear-offs de topo e literais.
    permanentes: crate::hash::HashSet<i64>,
    /// Constante canônica → o getter gerado que a produz (o mesmo endereço
    /// em todos os isolados): uma mensagem para outro isolado leva o getter,
    /// e o destino recebe a própria instância canônica (`identical` entre
    /// isolados, como os objetos compartilhados do grupo da VM).
    constants: crate::hash::HashMap<i64, usize>,
    /// Tear-off canônico → o código dele (o inverso de `tearoffs`).
    tearoff_code: crate::hash::HashMap<i64, i64>,
    /// `DARTFORGE_GC_OFF=1`: nunca coleta. Instrumento de diagnóstico
    /// (docs/NATIVO-PLANO.md §6): um programa que morre com "handle já
    /// coletado" e passa com a coleta desligada tem raiz faltando; um que
    /// morre igual nos dois modos tem escalar usado como handle.
    gc_disabled: bool,
    /// Valor corrente de cada global `Ref` do programa (variável de topo ou
    /// campo estático), por id: raízes permanentes (N6/G6 do contrato).
    globals: crate::hash::HashMap<i64, i64>,
    /// Raízes que moram no runtime e não num frame (G6): a exceção pendente
    /// e o rastro corrente. 0 = nenhuma.
    runtime_roots: [i64; 2],
    /// Ocorrências compartilhadas mantidas por mensagens neste isolado.
    message_owners: crate::hash::HashMap<u64, Ref>,
    next_message_owner: u64,
    /// Tokens SSA da ABI ARC, com multiplicidade separada de slots e mapas.
    /// O inventário sustenta alcance e auditoria; mapas não contam outra vez.
    code_owners: crate::hash::HashMap<Ref, usize>,
    /// Campos `late` já escritos, por handle e índice físico. A marca fica
    /// fora do valor: zero e null são atribuições válidas do programa.
    late_fields_initialized: crate::hash::HashSet<(i64, i64)>,
    /// As marcas de `late_fields_initialized` gravadas desde a última
    /// coleta: só elas podem ser de um jovem, e a coleta menor confere só
    /// estas (os velhos não morrem nela).
    new_lates: Vec<(i64, i64)>,
    /// A época de layout (`LAYOUT_EPOCH`) dos objetos deste heap: a de
    /// quando ele nasceu, e a de cada migração aplicada depois (J03).
    pub layout_epoch: u64,
    /// Onde mora todo valor do runtime ([`ObjectSpace`]).
    objects: ObjectSpace,
    /// O heap do isolado da thread (não um de teste): reabastece a TLAB do
    /// [`Context`] dela.
    published: bool,
    /// Bytes alocados desde a última coleta (o gatilho da coleta menor).
    young_bytes: usize,
    /// A coleta em curso é menor (só os jovens).
    minor_collection: bool,
    /// `--gc-stress`: coletas desde a última completa e o trabalho de marcação
    /// dela (o espaçamento das completas, [`Heap::coletar_automatico`]).
    minors_since_full: u64,
    work_in_full: usize,
    /// Bytes fora do heap que contam para os gatilhos
    /// ([`Heap::count_external`]); os dos anexos o espaço conta à parte.
    external: u128,
    /// Soma interna antes da projeção saturada em HeapStats; preserva descontos após overflow.
    exact_estimated_bytes: u128,
    /// `DARTFORGE_GC_VERIFICAR=1`: toda coleta menor confere, por uma
    /// travessia completa, que nenhum jovem alcançável ficou sem marca (uma
    /// barreira de escrita faltando).
    /// Quanto o heap pode crescer até a próxima coleta completa, em % do que
    /// sobreviveu à última ([`GROWTH`]; para medir,
    /// `DARTFORGE_GC_CRESCIMENTO`).
    growth: usize,
    /// Os bytes entre coletas menores ([`YOUNG_LIMIT`]; para medir,
    /// `DARTFORGE_GC_JOVEM_KB`).
    young_limit: usize,
    verify: bool,
    /// Confere todo handle contra o mapa de páginas antes de ler o bloco (as
    /// mensagens N4 de handle inválido): no `--gc-stress`, com a verificação,
    /// nos testes e no runtime de depuração, ou com
    /// `DARTFORGE_VALIDAR_HANDLES=1`. Fora disso, um handle de objeto é o
    /// bloco (a invariante do código gerado): só o estado `LIVRE` (objeto já
    /// coletado) é conferido, na leitura do cabeçalho que se faz de todo modo.
    validate_handles: bool,
    /// `DARTFORGE_GC_RASTRO=1`: uma linha por coleta no stderr (o tipo, os
    /// marcados, a estimativa e o gatilho).
    trace: bool,
    /// `DARTFORGE_GC_MEMORIA=1`: depois de cada coleta completa (e no fim do
    /// programa), a quebra da memória do espaço por classe de tamanho, das
    /// tabelas do runtime e do processo ([`Heap::memory_report`]).
    /// Com `2`, depois de toda coleta.
    memory: u8,
    /// Pares nativos finalizáveis (o `Dart_NewFinalizableHandle` da VM):
    /// objeto → (finalizador, par). Quando o objeto morre, a coleta tira a
    /// entrada e chama `finalizer(pair)` — que só libera recursos do
    /// sistema (fecha um arquivo, solta uma contagem de referências) e nunca
    /// toca o heap.
    pub finalizable: crate::hash::HashMap<i64, Finalizer>,
    /// As referências fracas (`WeakReference`, o `WeakReference_*` da VM):
    /// objeto portador → alvo. O alvo NÃO é seguido pela marcação; se não
    /// sobreviver por outro caminho, a coleta o troca por 0 (null).
    pub weak_refs: crate::hash::HashMap<i64, i64>,
    /// Os efêmeros (`_WeakProperty`, a base do `Expando`): portador →
    /// (chave, valor). O valor só é alcançado se a chave for (ponto fixo na
    /// marcação); chave morta zera os dois.
    pub ephemerons: crate::hash::HashMap<i64, (i64, i64)>,
    /// Os anexos de `Finalizer`/`NativeFinalizer` (o `FinalizerEntry` da
    /// VM): valor, dono e chave de `detach` são fracos; a ação Dart é
    /// aresta forte do dono. Valor morto: a ação de um `Finalizer` vai para
    /// [`Heap::ready_finalizations`] (o laço de eventos a chama); a de um
    /// `NativeFinalizer` roda logo depois da coleta.
    attachments: Vec<FinalizerAttachment>,
    /// Posições dos anexos Dart por dono; reconstruído após remoções do vetor.
    attachments_by_owner: crate::hash::HashMap<Ref, Vec<usize>>,
    /// As ações de `Finalizer` cujo valor morreu, à espera do laço de
    /// eventos (raízes até lá).
    pub ready_finalizations: std::collections::VecDeque<i64>,
    /// O ARC (`--memory=arc`, docs/ARC-IMPLEMENTACAO.md): ligado pelo
    /// programa na entrada (`dartforge_memoria_arc_v1`); `None` é o coletor
    /// por rastreamento de sempre.
    arc: Option<Box<HeapArc>>,
}

/// Um anexo de finalizador (ver `Heap::attachments`).
#[derive(Debug, Clone, Copy)]
pub struct FinalizerAttachment {
    /// O `Finalizer`/`NativeFinalizer` (identidade do `detach`).
    pub owner: i64,
    pub value: i64,
    /// A chave de `detach` (0 = nenhuma).
    pub detach: i64,
    pub action: FinalizerAction,
}

/// O que um finalizador faz quando o valor morre.
#[derive(Debug, Clone, Copy)]
pub enum FinalizerAction {
    /// Uma closure Dart sem argumentos (`callback(token)` já aplicada).
    Dart(i64),
    /// `function(token)`, uma função C, e bytes externos contabilizados até execução/detach.
    Native(usize, usize, usize),
}

#[allow(unsafe_code)]
impl Heap {
    /// Roda as ações nativas de todos os anexos ainda vivos (o isolado
    /// terminou: a VM garante os `NativeFinalizer` no encerramento) e
    /// descarta os de `Finalizer`.
    pub fn encerrar_finalizadores(&mut self) {
        let attachments = std::mem::take(&mut self.attachments);
        self.attachments_by_owner.clear();
        while self.finish_ready_finalization().is_some() {}
        for a in attachments {
            if let FinalizerAction::Dart(action) = a.action {
                self.arc_raw_write(a.owner, action, 0);
            }
            if let FinalizerAction::Native(f, token, bytes) = a.action {
                self.count_external(-(bytes as isize));
                // SAFETY: `f` é a `NativeFinalizerFunction` que o programa
                // registrou, `void f(void* token)`.
                let f: extern "C" fn(usize) = unsafe { std::mem::transmute(f) };
                f(token);
            }
        }
    }
}
impl Heap {
    /// Inicializa heap; stress força coleta antes de cada alocação.
    pub fn new(stress: bool) -> Self {
        Self {
            frames: Vec::new(),
            next_frame: 1,
            allocations: 0,
            stress,
            schedule: Schedule::from_env(),
            stats: HeapStats::default(),
            marking_work: 0,
            pending: Vec::new(),
            byte_threshold: 2 * YOUNG_LIMIT,
            growth: std::env::var("DARTFORGE_GC_CRESCIMENTO").ok().and_then(|v| v.parse::<usize>().ok()).map_or(GROWTH, |c| c.max(110)),
            young_limit: std::env::var("DARTFORGE_GC_JOVEM_KB")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .map_or(YOUNG_LIMIT, |kb| (kb * 1024).max(64 * 1024)),
            byte_limit: Self::limit_from_env(),
            enum_values: crate::hash::HashMap::default(),
            tearoffs: crate::hash::HashMap::default(),
            literals: crate::hash::HashMap::default(),
            permanentes: crate::hash::HashSet::default(),
            constants: crate::hash::HashMap::default(),
            tearoff_code: crate::hash::HashMap::default(),
            gc_disabled: std::env::var("DARTFORGE_GC_OFF").as_deref() == Ok("1"),
            globals: crate::hash::HashMap::default(),
            runtime_roots: [0, 0],
            message_owners: crate::hash::HashMap::default(),
            next_message_owner: 0,
            code_owners: crate::hash::HashMap::default(),
            late_fields_initialized: crate::hash::HashSet::default(),
            new_lates: Vec::new(),
            layout_epoch: LAYOUT_EPOCH.load(std::sync::atomic::Ordering::Acquire),
            // Os testes do runtime conferem que o handle de um morto é
            // recusado: lá, como no `--gc-stress`, os mortos são zerados já.
            external: 0,
            exact_estimated_bytes: 0,
            objects: ObjectSpace::new(stress || cfg!(test) || std::env::var("DARTFORGE_GC_VERIFICAR").as_deref() == Ok("1")),
            published: false,
            young_bytes: 0,
            arc: None,
            minor_collection: false,
            minors_since_full: 0,
            work_in_full: 0,
            verify: std::env::var("DARTFORGE_GC_VERIFICAR").as_deref() == Ok("1"),
            validate_handles: stress
                || cfg!(any(test, debug_assertions))
                // O veneno só pega o uso depois de liberar na validação.
                || std::env::var("DARTFORGE_GC_VENENO").is_ok_and(|v| !v.is_empty() && v != "0")
                || std::env::var("DARTFORGE_GC_VERIFICAR").as_deref() == Ok("1")
                || std::env::var("DARTFORGE_VALIDAR_HANDLES").as_deref() == Ok("1"),
            trace: std::env::var("DARTFORGE_GC_RASTRO").as_deref() == Ok("1"),
            memory: std::env::var("DARTFORGE_GC_MEMORIA").ok().and_then(|v| v.parse::<u8>().ok()).unwrap_or(0),
            finalizable: crate::hash::HashMap::default(),
            weak_refs: crate::hash::HashMap::default(),
            ephemerons: crate::hash::HashMap::default(),
            attachments: Vec::new(),
            attachments_by_owner: crate::hash::HashMap::default(),
            ready_finalizations: std::collections::VecDeque::new(),
        }
    }
    /// Slot proprietário do runtime: 0 = exceção pendente, 1 = rastro corrente.
    ///
    /// # Panics
    /// `which` não é 0 nem 1, ou um handle morto é detectado pela validação.
    ///
    /// ```
    /// use dartforge_runtime::heap::Heap;
    /// let mut heap = Heap::new(false);
    /// heap.set_runtime_root(0, 0);
    /// ```
    pub fn set_runtime_root(&mut self, which: usize, handle: i64) {
        if smi::e_handle(handle) {
            self.check_live(handle);
        }
        let old = self.runtime_roots[which];
        if handle != old { self.arc_swap_owning_root(handle, 0); }
        self.runtime_roots[which] = handle;
        if handle != old { self.arc_swap_owning_root(0, old); }
    }
    /// A string internada com as `unidades` (a tabela `literals`, raiz), se já
    /// existe: o literal do JIT e do runtime (`textos.rs`, P1).
    pub fn literal(&self, unidades: &[u16]) -> Option<Ref> {
        self.literals.get(unidades).copied()
    }
    /// Interna `h` (uma string com as `unidades`) como literal canônico: raiz
    /// (`literals`) e permanente (identidade entre portas do mesmo isolado).
    /// A entrada conta um owner; repetir o mesmo registro não retém novamente.
    ///
    /// ```
    /// use dartforge_runtime::heap::Heap;
    /// let mut heap = Heap::new(false);
    /// let text = heap.allocate_str("A");
    /// heap.store_literal(vec![65], text);
    /// assert_eq!(heap.literal(&[65]), Some(text));
    /// ```
    pub fn store_literal(&mut self, unidades: Vec<u16>, h: Ref) {
        if smi::e_handle(h) { self.check_live(h); }
        let old = self.literals.get(&unidades).copied().unwrap_or(0);
        self.arc_swap_owning_root(h, old);
        self.literals.insert(unidades, h);
        self.permanentes.insert(h);
    }
    /// Copia o valor para o owner global; null ou `Smi` solta o owner anterior.
    ///
    /// ```
    /// use dartforge_runtime::heap::Heap;
    /// let mut heap = Heap::new(false);
    /// heap.set_global_root(10, 0);
    /// ```
    pub fn set_global_root(&mut self, id: i64, handle: i64) {
        if smi::e_handle(handle) { self.check_live(handle); }
        let old = self.globals.get(&id).copied().unwrap_or(0);
        if handle != old { self.arc_swap_owning_root(handle, 0); }
        if !smi::e_handle(handle) {
            // null ou `Smi`: nada no heap a manter vivo.
            self.globals.remove(&id);
        } else {
            self.globals.insert(id, handle);
        }
        if handle != old { self.arc_swap_owning_root(0, old); }
    }

    /// Transfere token SSA ao registro proprietário de um slot global.
    /// O chamador publica os bits no armazenamento real antes desta chamada,
    /// sem safepoint no intervalo. O registro não guarda null/Smi como valores.
    pub(crate) fn move_code_to_global(&mut self, id: i64, value: Ref) {
        let old = self.globals.get(&id).copied().unwrap_or(0);
        if smi::e_handle(value) {
            self.check_live(value);
            let n = *self.code_owners.get(&value).expect("move global ARC sem token do código");
            if n == 1 {
                self.code_owners.remove(&value);
            } else {
                self.code_owners.insert(value, n - 1);
            }
            self.globals.insert(id, value);
        } else {
            self.globals.remove(&id);
        }
        self.arc_swap_owning_root(0, old);
    }

    /// Move a raiz de um global de um endereço de slot para outro (a área de
    /// globais de um módulo recarregado, `gc_raizes.rs`).
    /// Solta o owner anterior do destino. Origem ausente ou movimento para
    /// o mesmo endereço não altera os owners.
    ///
    /// ```
    /// use dartforge_runtime::heap::Heap;
    /// let mut heap = Heap::new(false);
    /// heap.move_global_root(10, 20);
    /// ```
    pub fn move_global_root(&mut self, de: i64, to: i64) {
        if de == to { return; }
        if let Some(h) = self.globals.remove(&de) {
            let old = self.globals.get(&to).copied().unwrap_or(0);
            self.globals.insert(to, h);
            self.arc_swap_owning_root(0, old);
        }
    }

    /// Solta a raiz de um global que deixou de existir.
    ///
    /// ```
    /// use dartforge_runtime::heap::Heap;
    /// let mut heap = Heap::new(false);
    /// heap.release_global_root(10);
    /// ```
    pub fn release_global_root(&mut self, id: i64) {
        if let Some(h) = self.globals.remove(&id) {
            self.arc_swap_owning_root(0, h);
        }
    }

    /// Marca `handle` como permanente (ver `permanentes`).
    pub fn mark_permanent(&mut self, handle: i64) {
        if smi::e_handle(handle) {
            self.permanentes.insert(handle);
        }
    }

    /// Marca `handle` como a constante canônica que `getter` produz.
    pub fn mark_constant(&mut self, handle: i64, getter: usize) {
        if smi::e_handle(handle) {
            self.permanentes.insert(handle);
            if getter != 0 {
                self.constants.insert(handle, getter);
            }
        }
    }

    /// O getter da constante canônica `handle`, se for uma.
    pub fn constant_getter(&self, handle: i64) -> Option<usize> {
        self.constants.get(&handle).copied()
    }

    /// O código do tear-off canônico `handle`, se for um.
    pub fn tearoff_code(&self, handle: i64) -> Option<i64> {
        self.tearoff_code.get(&handle).copied()
    }

    /// Se `handle` é permanente e imutável (ver `permanentes`): uma
    /// mensagem no mesmo isolado o passa sem copiar.
    pub fn is_permanent(&self, handle: i64) -> bool {
        self.permanentes.contains(&handle) || is_static(handle)
    }
    /// Le o teto do heap do ambiente uma vez, na criacao.
    fn limit_from_env() -> usize {
        const DEFAULT: usize = 256 * 1024 * 1024;
        match std::env::var("DARTFORGE_HEAP_MAX_MB") {
            Ok(v) => match v.trim().parse::<usize>() {
                Ok(0) => usize::MAX,
                Ok(mb) => mb.saturating_mul(1024 * 1024),
                Err(_) => DEFAULT,
            },
            Err(_) => DEFAULT,
        }
    }

    /// Bytes que o heap ocupa: os blocos em uso (com as regiões grandes e os
    /// bytes de fora, anexos e externos), mais `extra`.
    fn total_bytes(&self, extra: usize) -> usize {
        self.stats.estimated_bytes.saturating_add(extra)
    }

    /// Para o processo quando o heap passa do teto.
    fn verify_ceiling(&self, extra: usize) {
        if self.byte_limit == usize::MAX {
            return;
        }
        let total = self.total_bytes(extra);
        if total > self.byte_limit {
            Self::abort_out_of_memory(total, self.byte_limit);
        }
    }

    #[cold]
    #[inline(never)]
    fn abort_out_of_memory(total: usize, limit: usize) -> ! {
        // Sem `panic!`: o runtime e chamado por `extern "C"` a partir do codigo
        // gerado, e um panico atravessando essa fronteira aborta com um despejo
        // de pilha ilegivel. Uma linha e o codigo 255 (o mesmo da VM para
        // excecao nao capturada) sao o que o harness precisa.
        let mib = 1024 * 1024;
        eprintln!(
            "Out of memory: heap do DartForge chegou a {} MiB, acima do teto de {} MiB (ajuste com DARTFORGE_HEAP_MAX_MB, 0 desliga).",
            total / mib,
            limit / mib
        );
        std::process::exit(255);
    }

    /// Obtém o singleton de um valor enum, protegendo as alocações internas.
    /// A tabela guarda um owner persistente para cada singleton criado.
    ///
    /// # Panics
    /// A identidade da classe ou o índice é negativo.
    ///
    /// ```
    /// use dartforge_runtime::heap::Heap;
    /// let mut heap = Heap::new(false);
    /// let value = heap.enum_value(900, 0, "zero");
    /// assert_eq!(heap.enum_value(900, 0, "zero"), value);
    /// ```
    pub fn enum_value(&mut self, class_id: i64, index: i64, name: &str) -> i64 {
        assert!(class_id >= 0 && index >= 0, "identidade enum inválida");
        if let Some(&handle) = self.enum_values.get(&(class_id, index)) {
            return handle;
        }
        let frame = self.push_owning_frame(1);
        let text = self.allocate_str(name);
        self.set_root(frame, 0, text);
        let object = self.new_object(class_id, &[(index, false), (text, true)]);
        self.arc_swap_owning_root(object, 0);
        self.enum_values.insert((class_id, index), object);
        self.permanentes.insert(object);
        self.pop_frame(frame);
        object
    }
    /// O tear-off canônico já registrado da função de topo `code` (a tabela
    /// `tearoffs`, raiz), se existe (`caixas.rs`, P2).
    pub fn registered_tearoff(&self, code: i64) -> Option<Ref> {
        self.tearoffs.get(&code).copied()
    }
    /// Registra `h` como o tear-off canônico de `code`: raiz (`tearoffs`), o
    /// inverso (`tearoff_code`) e permanente.
    /// A tabela conta um owner, incluindo substituição sem duplicar aliases.
    ///
    /// ```
    /// use dartforge_runtime::heap::Heap;
    /// let mut heap = Heap::new(false);
    /// let closure = heap.new_closure(123, (0, false), 0, 0);
    /// heap.register_tearoff(123, closure);
    /// assert_eq!(heap.registered_tearoff(123), Some(closure));
    /// ```
    pub fn register_tearoff(&mut self, code: i64, h: Ref) {
        if smi::e_handle(h) { self.check_live(h); }
        let old = self.tearoffs.get(&code).copied().unwrap_or(0);
        self.arc_swap_owning_root(h, old);
        self.tearoffs.insert(code, h);
        self.tearoff_code.insert(h, code);
        self.permanentes.insert(h);
    }
    /// Abre frame de raízes com identificador monotônico.
    pub fn push_frame(&mut self) -> i64 {
        self.push_frame_with_slots(0)
    }
    /// Reserva slots fixos inicialmente null, reutilizados por todas as iterações.
    pub fn push_frame_with_slots(&mut self, slots: usize) -> i64 {
        let id = self.next_frame;
        self.next_frame = id.checked_add(1).expect("frames esgotados");
        self.stats.root_slots = self
            .stats
            .root_slots
            .checked_add(slots)
            .expect("slots excedem usize");
        self.stats.peak_root_slots = self.stats.peak_root_slots.max(self.stats.root_slots);
        self.frames.push((id, vec![0; slots], false));
        id
    }
    /// Abre slots proprietários: cópia retém e fechamento solta no ARC.
    /// Quadros observacionais devem continuar usando `push_frame_with_slots`.
    ///
    /// # Panics
    /// O identificador ou a soma dos slots se esgotou.
    ///
    /// ```
    /// use dartforge_runtime::heap::Heap;
    /// let mut heap = Heap::new(false);
    /// let frame = heap.push_owning_frame(1);
    /// heap.set_root(frame, 0, 0);
    /// heap.pop_frame(frame);
    /// ```
    pub fn push_owning_frame(&mut self, slots: usize) -> i64 {
        let id = self.push_frame_with_slots(slots);
        self.frames.last_mut().unwrap().2 = true;
        id
    }

    /// Exige owner antes de uma operação da ABI; quadro observacional não
    /// pode aceitar uma cópia que o compilador acredita ser contada.
    pub(crate) fn check_owning_frame(&self, frame: i64) {
        let (_, _, owning) = self.frames.iter().rfind(|(id, _, _)| *id == frame)
            .expect("quadro ARC inexistente");
        assert!(*owning, "quadro ARC deve ser proprietário");
    }

    fn owning_roots(&self) -> Vec<Ref> {
        self.frames.iter().filter(|(_, _, owner)| *owner)
            .flat_map(|(_, slots, _)| slots.iter().copied())
            .chain(self.globals.values().copied())
            .chain(self.literals.values().copied())
            .chain(self.tearoffs.values().copied())
            .chain(self.enum_values.values().copied())
            .chain(self.message_owners.values().copied())
            .chain(self.code_owners.iter().flat_map(|(&h, &n)| std::iter::repeat_n(h, n)))
            .chain(self.ready_finalizations.iter().copied())
            .chain(self.runtime_roots.iter().copied()).collect()
    }

    /// Publica uma ocorrência proprietária na fila, sem coleta entre retenção e escrita.
    #[cfg(test)]
    pub(crate) fn add_ready_finalization(&mut self, action: Ref) {
        self.check_live(action);
        self.arc_swap_owning_root(action, 0);
        self.ready_finalizations.push_back(action);
    }

    /// Consome o owner da primeira finalização; fila vazia é um no-op.
    pub(crate) fn finish_ready_finalization(&mut self) -> Option<Ref> {
        let action = self.ready_finalizations.pop_front()?;
        self.arc_swap_owning_root(0, action);
        Some(action)
    }

    /// Transfere a ocorrência da fila ao callback ativo, sem retenção adicional.
    pub(crate) fn move_finalization_to_slot(&mut self, frame: i64, slot: usize) -> Ref {
        let ix = self.frames.iter().rposition(|(id, _, _)| *id == frame)
            .expect("frame inexistente");
        assert!(self.frames[ix].2, "movimento exige quadro proprietário");
        let old = *self.frames[ix].1.get(slot).expect("slot de raiz inválido");
        let value = *self.ready_finalizations.front().expect("fila de finalizações vazia");
        self.arc_swap_owning_root(0, old);
        self.ready_finalizations.pop_front();
        self.frames[ix].1[slot] = value;
        self.stats.live_roots -= usize::from(old != 0);
        self.stats.live_roots += usize::from(value != 0);
        self.stats.peak_roots = self.stats.peak_roots.max(self.stats.live_roots);
        value
    }

    /// Retém uma ocorrência de mensagem no domínio deste heap.
    pub(crate) fn retain_message_owner(&mut self, value: Ref) -> u64 {
        self.check_live(value);
        let id = self.next_message_owner.checked_add(1).expect("owners de mensagem esgotados");
        self.arc_swap_owning_root(value, 0);
        self.message_owners.insert(id, value);
        self.next_message_owner = id;
        id
    }

    /// Consome uma ocorrência de mensagem neste heap, sem coleta.
    pub(crate) fn release_message_owner(&mut self, id: u64) {
        let value = self.message_owners.remove(&id).expect("owner de mensagem inexistente");
        self.arc_swap_owning_root(0, value);
    }

    /// Snapshot de diagnóstico: bit 0 para bloco vivo do heap, bit 1 para owner do código.
    /// Consulta o índice de páginas antes de ler o cabeçalho; não dereferencia
    /// endereços fora do heap nem inclui objetos estáticos, null ou Smi.
    pub(crate) fn observe_code_owner(&self, address: i64) -> u8 {
        let alive = self.objects.occupied_block(address);
        u8::from(alive) | (u8::from(self.code_owners.contains_key(&address)) << 1)
    }

    /// `address` tem token do código? O bit 1 de [`Self::observe_code_owner`],
    /// sem procurar o bloco nas listas de livres (a conferência de cada
    /// retorno Owned das portas).
    pub(crate) fn has_code_owner(&self, address: i64) -> bool {
        self.code_owners.contains_key(&address)
    }

    /// Cria uma ocorrência do código gerado, sem coleta ou execução Dart.
    /// A localização observacional publicada não cria outro owner.
    pub(crate) fn retain_code_owner(&mut self, value: Ref) {
        if !smi::e_handle(value) { return; }
        self.check_live(value);
        self.arc_swap_owning_root(value, 0);
        // Uma busca só no inventário (o caminho de cada retain do código).
        let n = self.code_owners.entry(value).or_insert(0);
        *n = n.checked_add(1).expect("tokens ARC do código esgotados");
    }

    /// Consome exatamente uma ocorrência da ABI, nunca um owner de slot.
    pub(crate) fn release_code_owner(&mut self, value: Ref) {
        if !smi::e_handle(value) { return; }
        match self.code_owners.entry(value) {
            std::collections::hash_map::Entry::Occupied(mut e) => {
                if *e.get() == 1 {
                    e.remove();
                } else {
                    *e.get_mut() -= 1;
                }
            }
            std::collections::hash_map::Entry::Vacant(_) => panic!("release ARC sem token do código"),
        }
        self.arc_swap_owning_root(0, value);
    }

    /// Copia um slot proprietário para um token independente do código.
    pub(crate) fn copy_root_to_code(&mut self, frame: i64, slot: usize) -> Ref {
        self.check_owning_frame(frame);
        let i = self
            .frames
            .iter()
            .rposition(|(id, _, _)| *id == frame)
            .unwrap();
        let value = *self.frames[i].1.get(slot).expect("slot de raiz inválido");
        self.retain_code_owner(value);
        value
    }

    /// Transfere um token do código para um slot, sem retain da origem.
    /// Valida tudo antes de consumir/publicar; solta o antigo após publicação.
    pub(crate) fn move_code_to_root(&mut self, frame: i64, slot: usize, value: Ref) {
        self.check_owning_frame(frame);
        let i = self
            .frames
            .iter()
            .rposition(|(id, _, _)| *id == frame)
            .unwrap();
        let old = *self.frames[i].1.get(slot).expect("slot de raiz inválido");
        if smi::e_handle(value) {
            self.check_live(value);
            let n = *self
                .code_owners
                .get(&value)
                .expect("move ARC sem token do código");
            if n == 1 {
                self.code_owners.remove(&value);
            } else {
                self.code_owners.insert(value, n - 1);
            }
        }
        self.frames[i].1[slot] = value;
        self.stats.live_roots -= usize::from(old != 0);
        self.stats.live_roots += usize::from(value != 0);
        self.stats.peak_roots = self.stats.peak_roots.max(self.stats.live_roots);
        // A ocorrência nova já era contada no inventário do código.
        self.arc_swap_owning_root(0, old);
    }

    /// Retém antes de soltar, sem coleta nem chamada Dart entre as operações.
    fn arc_swap_owning_root(&mut self, new: Ref, old: Ref) {
        if new == old { return; }
        let Some(mut arc) = self.arc.take() else { return };
        self.arc_retain_registering(&mut arc, new);
        arc.estado.release(old).unwrap_or_else(|e| arc_failure(e));
        self.arc = Some(arc);
    }

    /// Copia a raiz para o slot; zero remove a referência anterior.
    /// Num quadro proprietário, retém a nova ocorrência antes de soltar a antiga.
    /// Um `Smi` ocupa o slot como qualquer `Ref`, mas o coletor não o segue.
    pub fn set_root(&mut self, frame: i64, slot: usize, handle: i64) {
        if smi::e_handle(handle) {
            self.check_live(handle);
        }
        let ix = self.frames.iter().rposition(|(id, _, _)| *id == frame)
            .expect("frame inexistente");
        let old = *self.frames[ix].1.get(slot).expect("slot de raiz inválido");
        let owning = self.frames[ix].2;
        if owning && handle != old { self.arc_swap_owning_root(handle, 0); }
        let previous = &mut self.frames[ix].1[slot];
        self.stats.live_roots -= usize::from(*previous != 0);
        self.stats.live_roots += usize::from(handle != 0);
        *previous = handle;
        self.stats.peak_roots = self.stats.peak_roots.max(self.stats.live_roots);
        if owning && handle != old { self.arc_swap_owning_root(0, old); }
    }
    /// Transfere uma ocorrência entre slots proprietários, zerando a origem.
    /// Solta o conteúdo anterior do destino sem reter o valor transferido.
    /// Mover um slot para ele próprio preserva seu conteúdo e sua contagem.
    /// Não há coleta nem chamada Dart durante a transferência.
    ///
    /// # Panics
    /// Algum quadro não existe, não é proprietário ou o slot é inválido.
    ///
    /// ```
    /// use dartforge_runtime::heap::Heap;
    /// let mut heap = Heap::new(false);
    /// let caller = heap.push_owning_frame(1);
    /// let callee = heap.push_owning_frame(1);
    /// heap.move_root(callee, 0, caller, 0);
    /// heap.pop_frame(callee);
    /// heap.pop_frame(caller);
    /// ```
    pub fn move_root(&mut self, origin: i64, source_slot: usize, dest: i64, dest_slot: usize) {
        let de = self.frames.iter().rposition(|(id, _, _)| *id == origin)
            .expect("frame de origem inexistente");
        let to = self.frames.iter().rposition(|(id, _, _)| *id == dest)
            .expect("frame de destino inexistente");
        assert!(self.frames[de].2 && self.frames[to].2, "movimento exige quadros proprietários");
        let value = *self.frames[de].1.get(source_slot).expect("slot de origem inválido");
        let old = *self.frames[to].1.get(dest_slot).expect("slot de destino inválido");
        if de == to && source_slot == dest_slot { return; }
        self.frames[de].1[source_slot] = 0;
        self.frames[to].1[dest_slot] = value;
        self.stats.live_roots -= usize::from(old != 0);
        self.arc_swap_owning_root(0, old);
    }

    /// Transfere um owner global para um slot proprietário, consumindo o global.
    /// Solta o conteúdo anterior do slot sem reter o valor transferido.
    ///
    /// # Panics
    /// O global não existe, o quadro não é proprietário ou o slot é inválido.
    ///
    /// ```
    /// use dartforge_runtime::heap::Heap;
    /// let mut heap = Heap::new(false);
    /// let text = heap.allocate_str("event");
    /// heap.set_global_root(-1, text);
    /// let frame = heap.push_owning_frame(1);
    /// heap.move_global_to_slot(-1, frame, 0);
    /// heap.pop_frame(frame);
    /// ```
    pub fn move_global_to_slot(&mut self, global: i64, frame: i64, slot: usize) {
        let ix = self.frames.iter().rposition(|(id, _, _)| *id == frame)
            .expect("frame inexistente");
        assert!(self.frames[ix].2, "movimento exige quadro proprietário");
        let old = *self.frames[ix].1.get(slot).expect("slot de raiz inválido");
        let value = *self.globals.get(&global).expect("owner global inexistente");
        self.globals.remove(&global);
        self.frames[ix].1[slot] = value;
        self.stats.live_roots -= usize::from(old != 0);
        self.stats.live_roots += usize::from(value != 0);
        self.stats.peak_roots = self.stats.peak_roots.max(self.stats.live_roots);
        self.arc_swap_owning_root(0, old);
    }

    /// Protege handle até o retorno da função; null não ocupa uma raiz.
    pub fn root(&mut self, frame: i64, handle: i64) {
        if !smi::e_handle(handle) {
            return;
        }
        self.check_live(handle);
        let ix = self.frames.iter().rposition(|(id, _, _)| *id == frame)
            .expect("frame inexistente");
        if self.frames[ix].2 { self.arc_swap_owning_root(handle, 0); }
        self.frames[ix].1.push(handle);
        self.stats.live_roots += 1;
        self.stats.root_slots += 1;
        self.stats.peak_roots = self.stats.peak_roots.max(self.stats.live_roots);
        self.stats.peak_root_slots = self.stats.peak_root_slots.max(self.stats.root_slots);
    }
    /// Fecha exatamente o frame do topo, sem coletar entre retorno e raiz do chamador.
    pub fn pop_frame(&mut self, frame: i64) {
        assert_eq!(self.frames.last().map(|(id, _, _)| *id), Some(frame));
        let (_, roots, owning) = self.frames.pop().unwrap();
        if owning {
            for &h in &roots { self.arc_swap_owning_root(0, h); }
        }
        self.stats.root_slots -= roots.len();
        self.stats.live_roots -= roots.iter().filter(|handle| **handle != 0).count();
    }
    /// Coleta se algum gatilho pede, confere o teto e conta a alocação de
    /// `bytes`.
    fn antes_de_alocar(&mut self, bytes: usize) {
        // Dois limites diferentes (G6):
        // * o GATILHO (`threshold`, `byte_threshold`): quanto se aloca desde
        //   a última coleta, recalculado no fim de cada uma
        //   (`recompute_triggers`) a partir dos sobreviventes e da folga até
        //   o teto;
        // * o TETO (`byte_limit`): só a alocação que passaria dele força
        //   uma última coleta; se ainda passa, falta memória.
        // O portão antigo (`!self.frames.is_empty()`) existia porque o código
        // gerado não registrava raízes; com o frame de cada função (G1),
        // coletar sem frame aberto é só coletar com as raízes permanentes.
        if !self.gc_disabled && self.needs_collection(bytes) {
            self.coletar_automatico(bytes);
        }
        // Depois da coleta: se ainda passa do teto, nao ha o que recuperar.
        self.verify_ceiling(bytes);
        self.count_allocation(1, bytes);
    }
    /// Um objeto do usuário novo com os campos `fields` (os que o runtime
    /// monta à mão: erros, valores de enum, a classe `Type`…). Coleta antes
    /// como [`Heap::allocate`]: as referências em `fields` precisam de raiz
    /// de quem chama.
    pub fn new_object(&mut self, class_id: i64, fields: &[Field]) -> i64 {
        let h = self.allocate_object(class_id, fields.len());
        for (i, &(bits, is_ref_kind)) in fields.iter().enumerate() {
            self.set_field(h, i, bits, is_ref_kind);
        }
        h
    }
    /// Conta `k` alocações de `bytes` no total para os gatilhos e os
    /// contadores.
    #[inline]
    fn count_allocation(&mut self, k: usize, bytes: usize) {
        self.add_estimated_bytes(bytes);
        self.allocations += k;
        self.stats.allocations += k as u64;
        self.young_bytes = self.young_bytes.saturating_add(bytes);
    }
    /// O campo `late` `key` (handle, índice) já foi escrito?
    #[inline]
    pub fn late_initialized(&self, key: (i64, i64)) -> bool {
        !self.late_fields_initialized.is_empty() && self.late_fields_initialized.contains(&key)
    }
    /// Marca o campo `late` `key` como escrito.
    pub fn mark_late(&mut self, key: (i64, i64)) {
        if self.late_fields_initialized.insert(key) {
            self.new_lates.push(key);
        }
    }
    /// Desfaz a marca do campo `late` `key`.
    pub fn unmark_late(&mut self, key: (i64, i64)) {
        self.late_fields_initialized.remove(&key);
    }
    /// O heap do isolado da thread: reabastece a TLAB do [`Context`] (a
    /// alocação em linha do código gerado).
    pub fn for_isolate(stress: bool) -> Self {
        let mut h = Self::new(stress);
        h.published = true;
        h
    }
    /// Aloca um objeto do usuário de `n` campos zerados (o caminho do
    /// `dartforge_object_new`), no espaço de objetos.
    ///
    /// O caminho rápido confere os gatilhos de [`Heap::allocate`] numa
    /// comparação cada; se algum dispara (coleta, teto, `--gc-stress`), vai
    /// ao caminho geral.
    #[inline]
    pub fn allocate_object(&mut self, class_id: i64, n: usize) -> i64 {
        let cid = i32::try_from(class_id).expect("id de classe além de 32 bits");
        self.alocar_bloco(cid, crate::layout::palavras_de_instancia(n), flags_do_bloco::INSTANCIA, n)
    }
    /// Um bloco de corpo de `words` palavras zeradas, classe `cid`, `flags` e
    /// `n` no cabeçalho (em `INSTANCIA`, o número de campos; nos demais, as
    /// palavras): o caminho de [`Heap::allocate`], [`Heap::allocate_object`] e de
    /// `dartforge_alocar`.
    #[inline]
    pub(crate) fn alocar_bloco(&mut self, cid: i32, words: usize, flags: u8, n: usize) -> Ref {
        let bytes = allocation_bytes(words);
        if self.needs_collection(bytes) {
            return self.alocar_lento(cid, words, flags, n);
        }
        self.count_allocation(1, bytes);
        self.objects.allocate(cid, words, flags, n)
    }
    /// Uma alocação de `bytes` agora passaria de algum gatilho de coleta?
    /// A menor vem a cada [`YOUNG_LIMIT`] bytes (ou [`YOUNG_COUNT`]
    /// alocações) desde a última coleta; a completa, quando o total passa do
    /// gatilho que a última completa calculou (`byte_threshold`) ou do teto.
    #[inline]
    fn needs_collection(&self, bytes: usize) -> bool {
        self.stress
            || self.schedule.as_ref().is_some_and(Schedule::draws)
            || self.allocations >= YOUNG_COUNT
            || self.young_bytes.saturating_add(bytes) > self.young_limit
            || self.stats.estimated_bytes.saturating_add(bytes) > self.byte_threshold
            || (self.byte_limit != usize::MAX && self.total_bytes(bytes) > self.byte_limit)
    }
    /// A coleta que os gatilhos pedem antes de alocar `bytes`: a completa se
    /// o total passou do gatilho dela (ou do teto, ou a cada
    /// [`MINORS_PER_FULL_UNDER_STRESS`] no `--gc-stress`); a menor senão.
    fn coletar_automatico(&mut self, bytes: usize) {
        let full = self.stats.estimated_bytes.saturating_add(bytes) > self.byte_threshold
            || (self.byte_limit != usize::MAX && self.total_bytes(bytes) > self.byte_limit)
            || (self.stress
                && self.minors_since_full + 1
                    >= MINORS_PER_FULL_UNDER_STRESS.max((self.work_in_full / WORK_PER_MINOR_UNDER_STRESS) as u64));
        self.coletar(!full);
    }
    #[cold]
    #[inline(never)]
    fn alocar_lento(&mut self, cid: i32, words: usize, flags: u8, n: usize) -> Ref {
        self.antes_de_alocar(allocation_bytes(words));
        self.objects.allocate(cid, words, flags, n)
    }
    /// Reabastece a TLAB de `n` campos do [`Context`] (se esgotada) com
    /// uma faixa contígua de até `TLAB_BLOCKS` blocos livres, contados
    /// como alocados agora — sem coletar: só o que cabe antes do próximo
    /// gatilho (o que passa dele fica para o runtime, que coleta). Sem TLAB
    /// no `--gc-stress` (toda alocação passa pelo runtime, que coleta antes)
    /// e nos heaps de teste.
    pub fn refill_tlab(&mut self, words: usize) {
        let n = words;
        if !self.published || self.stress || self.schedule.is_some() || n == 0 || n > TLAB_N {
            return;
        }
        let size = bytes_do_bloco(n);
        if CONTEXTO.with(|c| (c.tlab[n][1].get() as usize).saturating_sub(c.tlab[n][0].get() as usize) >= size) {
            return;
        }
        let mut k = TLAB_BLOCKS
            .min(YOUNG_COUNT.saturating_sub(self.allocations))
            .min(self.young_limit.saturating_sub(self.young_bytes) / size)
            .min(self.byte_threshold.saturating_sub(self.stats.estimated_bytes) / size);
        if self.byte_limit != usize::MAX {
            k = k.min(self.byte_limit.saturating_sub(self.total_bytes(0)) / size);
        }
        if k == 0 {
            return;
        }
        let (start, k) = self.objects.take_range(n, k);
        self.objects.live_set += k;
        self.count_allocation(k, k * size);
        CONTEXTO.with(|c| {
            c.tlab[n][0].set(start);
            c.tlab[n][1].set(start.wrapping_add(k * size));
        });
    }
    /// Devolve as TLABs do [`Context`] (o que o código gerado não usou) antes
    /// de uma coleta: o resto de cada faixa volta à frente da lista livre
    /// e sai da contagem de alocações.
    fn return_tlabs(&mut self) {
        if !self.published {
            return;
        }
        for n in 1..=TLAB_N {
            let (cursor, end) = CONTEXTO.with(|c| (c.tlab[n][0].replace(std::ptr::null_mut()), c.tlab[n][1].replace(std::ptr::null_mut())));
            let size = bytes_do_bloco(n);
            let k = (end as usize).saturating_sub(cursor as usize) / size;
            if k > 0 {
                self.objects.return_range(n, cursor, end);
                let bytes = k * size;
                self.objects.live_set -= k;
                self.allocations = self.allocations.saturating_sub(k);
                self.stats.allocations = self.stats.allocations.saturating_sub(k as u64);
                self.subtract_estimated_bytes(bytes);
                self.young_bytes = self.young_bytes.saturating_sub(bytes);
            }
        }
    }
    /// O bloco vivo do objeto `h` (`h & 3 == 2`), ou `panic` N4 para handle
    /// inválido ou objeto já coletado.
    ///
    /// Um objeto estático (seção da imagem, `PERMANENTE`) também é devolvido:
    /// ele se lê como qualquer bloco; quem grava confere
    /// ([`Heap::check_writable`]).
    ///
    /// As falhas têm causas diferentes e a mensagem é a chave de agrupamento do
    /// harness (docs/NATIVO-PLANO.md §6.4, N4): texto fixo na primeira linha e
    /// o detalhe na segunda — null desreferenciado, `Smi` ou escalar negativo
    /// usado como handle, handle fora do espaço (escalar positivo) e objeto já
    /// coletado (raiz faltando).
    #[inline(always)]
    fn live_block(&self, handle: i64) -> *mut Header {
        if !self.validate_handles && is_object(handle) {
            let b = (handle - DESLOCAMENTO_DO_HANDLE) as *mut Header;
            // SAFETY: sem a validação, um handle de objeto é um bloco do espaço
            // ou um estático (`validate_handles`).
            #[allow(unsafe_code)]
            if unsafe { (*b).estado } != LIVRE {
                return b;
            }
        }
        self.live_block_checked(handle)
    }
    /// [`Heap::live_block`] pelo mapa de páginas, com as mensagens N4 (o
    /// caminho de `validate_handles` e o das falhas).
    #[cold]
    #[inline(never)]
    fn live_block_checked(&self, handle: i64) -> *mut Header {
        if !is_object(handle) {
            Self::invalid_handle(handle);
        }
        match self.objects.block_of(handle) {
            // SAFETY: bloco de uma página do espaço.
            #[allow(unsafe_code)]
            Some(b) if unsafe { (*b).estado } == crate::espaco::POISON_STATE => {
                panic!("bug do compilador: handle já coletado (raiz faltando; bloco envenenado, DARTFORGE_GC_VENENO)\nhandle {handle}")
            }
            // SAFETY: bloco de uma página do espaço.
            #[allow(unsafe_code)]
            Some(b) if unsafe { (*b).estado } != 0 => b,
            Some(_) => panic!("bug do compilador: handle já coletado (raiz faltando)\nhandle {handle}"),
            None if is_static(handle) => (handle - DESLOCAMENTO_DO_HANDLE) as *mut Header,
            None if self.objects.in_page(handle) => {
                panic!("bug do compilador: handle já coletado (raiz faltando)\nhandle {handle}")
            }
            None => panic!("bug do compilador: handle além da tabela (escalar usado como handle)\nhandle {handle}"),
        }
    }
    /// O bloco de `h` se é objeto do espaço (vivo ou `LIVRE`; nunca um
    /// estático): o mapa de páginas com `validate_handles`, senão o próprio
    /// endereço.
    #[inline]
    fn space_block(&self, h: i64) -> Option<*mut Header> {
        if self.validate_handles {
            return self.objects.block_of(h);
        }
        if !is_object(h) {
            return None;
        }
        let b = (h - DESLOCAMENTO_DO_HANDLE) as *mut Header;
        // SAFETY: `validate_handles` (um handle de objeto é um bloco ou um estático).
        #[allow(unsafe_code)]
        (unsafe { (*b).estado } != PERMANENTE).then_some(b)
    }
    /// O filho `v` gravado num velho pede a barreira (§2.7)? Só um objeto jovem:
    /// null, `Smi`, velho e estático não pedem.
    #[inline]
    fn young_child(&self, v: Ref) -> bool {
        if !smi::e_handle(v) || v < 0 {
            return false;
        }
        // SAFETY: bloco do espaço.
        #[allow(unsafe_code)]
        self.space_block(v).is_some_and(|b| unsafe { (*b).estado } == JOVEM)
    }
    /// O pânico N4 de um handle que não é de objeto.
    #[cold]
    #[inline(never)]
    fn invalid_handle(handle: i64) -> ! {
        if handle == 0 {
            panic!("bug do compilador: handle null (0) desreferenciado");
        }
        if smi::is_smi(handle) {
            panic!(
                "bug do compilador: Smi usado como handle (int em posição Ref lido como objeto)\nSmi {}",
                smi::value(handle)
            );
        }
        if handle < 0 {
            panic!("bug do compilador: handle negativo (escalar usado como handle)\nhandle {handle}");
        }
        panic!("bug do compilador: handle além da tabela (escalar usado como handle)\nhandle {handle}");
    }
    /// Pânico se `b` é um objeto estático (gravar nele derrubaria o processo:
    /// a seção da imagem é só de leitura).
    #[inline]
    #[allow(unsafe_code)]
    fn check_writable(b: *const Header) {
        // SAFETY: bloco legível (de `live_block`).
        assert!(unsafe { (*b).estado } != PERMANENTE, "bug do compilador: gravação num objeto estático");
    }
    /// O `hashCode` de identidade do objeto `h` do espaço: tirado do
    /// endereço, que não muda enquanto o objeto vive (o coletor não move) e
    /// é único entre os vivos — 30 bits espalhados, nunca 0 (a VM sorteia e
    /// guarda no cabeçalho; aqui o cabeçalho não tem lugar e não precisa).
    /// `None` se `h` não é objeto vivo do espaço.
    pub fn identity_hash(&self, h: i64) -> Option<i64> {
        let b = match self.space_block(h) {
            Some(b) => b,
            None if self.is_static(h) => (h - DESLOCAMENTO_DO_HANDLE) as *mut Header,
            None => return None,
        };
        // SAFETY: bloco de uma página do espaço.
        #[allow(unsafe_code)]
        if unsafe { (*b).estado } == LIVRE {
            return None;
        }
        let x = ((b as u64 >> 3).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 34) as i64;
        Some(x.max(1))
    }
    /// A vista do objeto `h` do espaço (classe e campos), ou `None` se `h`
    /// não é objeto vivo do espaço (null, `Smi`, valor do runtime, escalar).
    pub fn object(&self, h: i64) -> Option<Obj<'_>> {
        let b = self.space_block(h)?;
        // SAFETY: bloco de uma página do espaço, conferido vivo.
        #[allow(unsafe_code)]
        unsafe {
            // O mapa só é mapa de referências em `INSTANCIA` (§2.3).
            if (*b).estado == LIVRE || shape(b) != flags_do_bloco::INSTANCIA {
                return None;
            }
            Some(Obj { body: body(b), class_id: i64::from((*b).class_id), _heap: std::marker::PhantomData })
        }
    }
    /// A vista para leitura do objeto `h`: do espaço ou estático (as
    /// constantes da imagem, nunca gravadas). As leituras auditadas do ARC
    /// leem campos das duas; a gravação usa [`Heap::object`].
    pub fn readable_object(&self, h: i64) -> Option<Obj<'_>> {
        if let Some(o) = self.object(h) {
            return Some(o);
        }
        if !self.is_static(h) {
            return None;
        }
        let b = (h - DESLOCAMENTO_DO_HANDLE) as *mut Header;
        // SAFETY: bloco estático da imagem, vivo o processo inteiro.
        #[allow(unsafe_code)]
        unsafe {
            if shape(b) != flags_do_bloco::INSTANCIA {
                return None;
            }
            Some(Obj { body: body(b), class_id: i64::from((*b).class_id), _heap: std::marker::PhantomData })
        }
    }
    /// A classe do objeto `h` do espaço (`None` se não é objeto vivo).
    #[inline]
    pub fn object_class(&self, h: i64) -> Option<i64> {
        self.object(h).map(|o| o.class_id)
    }
    /// Grava o campo `i` do objeto `h` (com a marca de referência e a
    /// barreira de escrita).
    ///
    /// # Panics
    /// Se `h` não é objeto vivo ou `i` está fora dele.
    pub fn set_field(&mut self, h: i64, i: usize, bits: i64, is_ref_kind: bool) {
        if is_ref_kind && smi::e_handle(bits) {
            self.check_live(bits);
        }
        let b = self.live_block(h);
        Self::check_writable(b);
        // SAFETY: bloco vivo.
        #[allow(unsafe_code)]
        let instance = unsafe { shape(b) } == flags_do_bloco::INSTANCIA;
        assert!(instance, "bug do runtime: definir_campo num objeto que não é INSTANCIA");
        self.objects.remember(b);
        // SAFETY: bloco vivo; `i` conferido contra o número de campos.
        #[allow(unsafe_code)]
        let old = unsafe {
            let c = body(b);
            assert!(i < usize::from((*c).n), "campo {i} fora do objeto de {} campos", (*c).n);
            let old = if is_reference(c, i) { *fields_of(c).add(i) } else { 0 };
            *fields_of(c).add(i) = bits;
            mark_reference(c, i, is_ref_kind);
            old
        };
        if self.arc.is_some() {
            self.arc_swapped(b, h, old, if is_ref_kind { bits } else { 0 });
        }
    }
    /// Pânico (N4) se `h` não é handle vivo.
    fn check_live(&self, h: i64) {
        self.live_block(h);
    }
    /// A barreira de escrita do código gerado (`dartforge_lembrar`): o
    /// objeto `h`, velho, recebeu um `Ref` num campo.
    pub fn remember_object(&mut self, h: i64) {
        if let Some(b) = self.space_block(h) {
            self.objects.remember(b);
        }
    }
    /// O metadado de um handle vivo (0 = nenhum).
    pub fn metadata(&self, handle: i64) -> i64 {
        // SAFETY: bloco vivo (ou estático).
        #[allow(unsafe_code)]
        i64::from(unsafe { (*self.live_block(handle)).metadata })
    }
    /// Grava o metadado de um handle vivo.
    pub fn set_metadata(&mut self, handle: i64, value: i64) {
        let b = self.live_block(handle);
        Self::check_writable(b);
        // SAFETY: bloco vivo.
        #[allow(unsafe_code)]
        unsafe {
            (*b).metadata = u32::try_from(value).expect("metadado além de 32 bits")
        };
    }
    /// Migra os objetos das classes de `plan` para o layout novo (J03):
    /// cada posição nova recebe o valor da antiga indicada (`-1`: zero, que
    /// é `null` para o campo anulável e "não inicializado" para o `late`), e
    /// as marcas de `late` inicializado seguem o campo.
    pub fn migrate_instances(&mut self, plan: &crate::hash::HashMap<i64, Vec<i64>>) {
        let mut blocks = Vec::new();
        self.objects.for_each_live(|b| blocks.push(b));
        for b in blocks {
            // SAFETY: bloco vivo do espaço.
            #[allow(unsafe_code)]
            let class_id = i64::from(unsafe { (*b).class_id });
            let Some(origin) = plan.get(&class_id) else { continue };
            let h = b as i64 + DESLOCAMENTO_DO_HANDLE;
            let olds = self.object(h).expect("objeto vivo").to_vec();
            let value_list: Vec<Field> = origin
                .iter()
                .map(|&o| usize::try_from(o).ok().and_then(|o| olds.get(o).copied()).unwrap_or((0, false)))
                .collect();
            self.swap_fields(b, &value_list);
        }
        let marks: Vec<(i64, i64)> = self.late_fields_initialized.iter().copied().filter(|&(_, i)| i >= 0).collect();
        for (h, i) in marks {
            let Some(class_id) = self.object_class(h) else { continue };
            let Some(origin) = plan.get(&class_id) else { continue };
            let new_area = origin.iter().position(|&o| o == i);
            self.late_fields_initialized.remove(&(h, i));
            if let Some(j) = new_area {
                self.mark_late((h, j as i64));
            }
        }
    }

    /// Troca os campos do objeto do bloco `b` por `value_list`: no próprio bloco
    /// se o número de campos é o dele, senão num corpo de fora (o objeto não
    /// muda de endereço; o código gerado segue o corpo, [`FORA`]).
    fn swap_fields(&mut self, b: *mut Header, value_list: &[Field]) {
        self.objects.remember(b);
        // No ARC, a migração (a recarga do JIT) e o campo acrescentado são
        // uma escrita crua: a foto conta os campos de antes e os de depois.
        if self.arc.is_some() {
            self.arc_snapshot(b, b as i64 + DESLOCAMENTO_DO_HANDLE);
        }
        // SAFETY: bloco vivo do espaço; o corpo de fora é novo e do tamanho
        // de `value_list`.
        #[allow(unsafe_code)]
        unsafe {
            let current = body(b);
            let c = if usize::from((*current).n) == value_list.len() {
                current
            } else {
                let c = new_out_of_line_body(value_list.len());
                (*c).class_id = (*b).class_id;
                if (*b).flags & FORA != 0 {
                    release_out_of_line_body(current);
                } else {
                    self.objects.with_out_of_line.push(b);
                }
                *fields_of(b).cast::<*mut Header>() = c;
                (*b).flags |= FORA;
                c
            };
            for (i, &(bits, is_ref_kind)) in value_list.iter().enumerate() {
                *fields_of(c).add(i) = bits;
                mark_reference(c, i, is_ref_kind);
            }
        }
    }

    /// Garante ao objeto `handle` pelo menos `n` campos (os novos,
    /// zerados): num corpo de fora (`Heap::swap_fields`).
    pub fn ensure_fields(&mut self, handle: i64, n: usize) {
        let Some(o) = self.object(handle) else { return };
        if o.len() >= n {
            return;
        }
        let mut new_ones = o.to_vec();
        new_ones.resize(n, (0, false));
        let b = self.live_block(handle);
        self.swap_fields(b, &new_ones);
    }
    /// O endereço dos campos do objeto `handle` (palavras de 8 bytes; o
    /// caminho de `dartforge_object_campos`); `None` se não é objeto vivo.
    /// Quem pede o ponteiro pode gravar por ele: conta como barreira de
    /// escrita, mas o mapa de referências fica com quem grava.
    #[inline]
    pub fn object_fields(&mut self, handle: i64) -> Option<*mut i64> {
        let b = self.space_block(handle)?;
        // SAFETY: bloco do espaço.
        #[allow(unsafe_code)]
        unsafe {
            if (*b).estado == LIVRE {
                return None;
            }
            self.objects.remember(b);
            if self.arc.is_some() {
                self.arc_snapshot(b, handle);
            }
            Some(fields_of(body(b)))
        }
    }
    /// Atualiza campo com tag explícita; valores escalares jamais são raízes.
    pub fn set(&mut self, handle: i64, index: i64, bits: i64, is_ref: bool) {
        self.set_field(handle, usize::try_from(index).expect("índice inválido"), bits, is_ref);
    }
    /// Marca raízes e arestas tipadas iterativamente e libera inclusive ciclos inalcançáveis.
    /// Marca tudo o que é alcançável a partir de `pending`; devolve quantos
    /// objetos marcou.
    fn mark_pending(&mut self) -> usize {
        // Uma cópia da marcação para cada tipo de coleta: o teste do estado
        // de cada objeto (jovem na menor, não marcado na completa) sai sem
        // desvio pelo tipo.
        if self.minor_collection { self.mark::<true>() } else { self.mark::<false>() }
    }

    fn mark<const MINOR: bool>(&mut self) -> usize {
        let minor = MINOR;
        let mut live = 0_usize;
        // A pilha e os contadores em variáveis locais: gravar em `self` a
        // cada objeto (o `Vec` e o trabalho) impedia o compilador de os
        // manter em registradores (as gravações nos objetos podem, para
        // ele, apontar para `self`).
        let mut stack = std::mem::take(&mut self.pending);
        let mut work = 0usize;
        let mut marked_objects = 0usize;
        // As arestas vêm de raízes e de campos `is_ref`: a validação
        // completa do handle (a página e o início do bloco) fica para o
        // `--gc-stress`, o modo que caça raiz faltando (N4).
        let validate = self.stress;
        // Esta coleta percorre o objeto no estado `e`? Na menor, só o jovem
        // (um velho conta como vivo, e as referências dele a jovens estão
        // nos lembrados); na completa, o que ainda não foi marcado.
        // Na completa, o marcado é o do bit no mapa de marcas da página (a
        // varredura lê só os bits).
        // SAFETY (dos usos): `b` é bloco de uma página do espaço.
        #[allow(unsafe_code)]
        // O estático (`PERMANENTE`) é conferido antes do mapa de marcas: o
        // mapa dele cairia fora de uma página do heap (§2.8).
        let walks = |b: *const Header| unsafe {
            if minor { (*b).estado == JOVEM } else { (*b).estado != PERMANENTE && !marked(b) }
        };
        // Uma aresta: o primeiro filho objeto ainda por marcar vai para
        // `next_offset` (seguido sem a pilha); os demais, para a pilha.
        #[allow(unsafe_code)]
        let visit_ref = |bits: i64, next_offset: &mut i64, stack: &mut Vec<i64>| {
            if !smi::e_handle(bits) {
                return;
            }
            if is_object(bits) {
                if !validate && !walks((bits - DESLOCAMENTO_DO_HANDLE) as *const Header) {
                    return;
                }
                if *next_offset == 0 {
                    *next_offset = bits;
                    return;
                }
            }
            stack.push(bits);
        };
        while let Some(handle) = stack.pop() {
            // null e `Smi` (R10) não são arestas: o coletor nunca segue um
            // `Smi`, que não aponta para o heap.
            if !smi::e_handle(handle) {
                continue;
            }
            if is_object(handle) {
                let mut current = handle;
                // Segue direto o primeiro filho ainda por marcar (a lista
                // ligada, o ramo de uma árvore) sem passar pela pilha.
                while current != 0 {
                    if validate && self.objects.block_of(current).is_none() && is_static(current) {
                        break;
                    }
                    let b = if validate {
                        self.live_block(current)
                    } else {
                        debug_assert!(self.objects.block_of(current).is_some(), "aresta inválida {current}");
                        (current - DESLOCAMENTO_DO_HANDLE) as *mut Header
                    };
                    let mut next_offset = 0;
                    // SAFETY: bloco vivo do espaço.
                    #[allow(unsafe_code)]
                    unsafe {
                        if !walks(b) {
                            break;
                        }
                        // As duas promovem já na marcação (o jovem alcançado
                        // fica velho, o lembrado volta a velho na completa:
                        // a varredura só lê os bits) e acendem o bit.
                        if (*b).estado != VELHO {
                            (*b).estado = VELHO;
                        }
                        mark_block(b);
                        live += 1;
                        marked_objects += 1;
                        self.visit_attachment_edges(current, &mut |action| visit_ref(action, &mut next_offset, &mut stack));
                        // O corpo pelo formato (§2.8): `BRUTO` não tem
                        // referências; `REFS`, as palavras `1..=word 0`;
                        // `INSTANCIA`, os campos que o mapa diz referência. O
                        // que esta coleta não percorre (velho na menor, já
                        // marcado) nem entra na pilha; o primeiro que falta
                        // marcar é seguido direto.
                        let tipo = shape(b);
                        if tipo != flags_do_bloco::INSTANCIA {
                            if tipo == flags_do_bloco::REFS {
                                let w = self.objects.body_words(b);
                                let n = body_refs(b, w);
                                work += n;
                                let p = fields_of(b);
                                for i in 1..=n {
                                    visit_ref(*p.add(i), &mut next_offset, &mut stack);
                                }
                                if !minor && (*b).flags & flags_do_bloco::CARTOES != 0 {
                                    crate::espaco::clear_cards(b, n);
                                }
                            }
                            current = next_offset;
                            continue;
                        }
                        let c = body(b);
                        let n = usize::from((*c).n);
                        work += n;
                        let fields = fields_of(c);
                        let visit = visit_ref;
                        let mut m = (*c).map;
                        while m != 0 {
                            let i = m.trailing_zeros() as usize;
                            visit(*fields.add(i), &mut next_offset, &mut stack);
                            m &= m - 1;
                        }
                        if n > 32 {
                            let ext = fields.add(capacidade(n)).cast::<u64>();
                            for w in 0..palavras_do_mapa(n) {
                                let mut m = *ext.add(w);
                                while m != 0 {
                                    let i = 32 + w * 64 + m.trailing_zeros() as usize;
                                    visit(*fields.add(i), &mut next_offset, &mut stack);
                                    m &= m - 1;
                                }
                            }
                        }
                    }
                    current = next_offset;
                }
                continue;
            }
            // Raiz ou aresta que não é handle de objeto: as mensagens N4.
            Self::invalid_handle(handle);
        }
        self.pending = stack;
        self.marking_work += work;
        self.objects.marked_set += marked_objects;
        live
    }

    /// As posições de referência forte do objeto `h`, pela forma do corpo, as
    /// mesmas que a marcação percorre (§2.8): `REFS`, as palavras
    /// `1..=word 0`; `INSTANCIA`, os campos do mapa (e do mapa estendido);
    /// `BRUTO`, nenhuma. Um estático ou um bloco livre não tem arestas aqui (o
    /// estático é imortal no ARC, docs/ARC-CICLOS-ESPECIFICACAO.md §19.1).
    fn ref_positions(&self, h: i64, f: &mut dyn FnMut(*mut i64)) {
        let Some(b) = self.space_block(h) else { return };
        // SAFETY: bloco do espaço; só os vivos são percorridos.
        #[allow(unsafe_code)]
        unsafe {
            if (*b).estado == LIVRE {
                return;
            }
            let tipo = shape(b);
            if tipo == flags_do_bloco::REFS {
                let w = self.objects.body_words(b);
                let n = body_refs(b, w);
                let p = fields_of(b);
                for i in 1..=n {
                    f(p.add(i));
                }
                return;
            }
            if tipo != flags_do_bloco::INSTANCIA {
                return;
            }
            let c = body(b);
            let n = usize::from((*c).n);
            let fields = fields_of(c);
            let mut m = (*c).map;
            while m != 0 {
                f(fields.add(m.trailing_zeros() as usize));
                m &= m - 1;
            }
            if n > 32 {
                let ext = fields.add(capacidade(n)).cast::<u64>();
                for w in 0..palavras_do_mapa(n) {
                    let mut m = *ext.add(w);
                    while m != 0 {
                        f(fields.add(32 + w * 64 + m.trailing_zeros() as usize));
                        m &= m - 1;
                    }
                }
            }
        }
    }

    /// Enumera ocorrências fortes do corpo e dos anexos pertencentes ao dono.
    #[allow(unsafe_code)]
    fn visit_strong_references(&self, h: Ref, f: &mut dyn FnMut(Ref)) {
        // SAFETY: as posições pertencem ao corpo vivo, sem mutação durante a visita.
        self.ref_positions(h, &mut |p| f(unsafe { *p }));
        self.visit_attachment_edges(h, f);
    }

    /// Visita só as ocorrências do dono, preservando aliases de anexos distintos.
    fn visit_attachment_edges(&self, owner: Ref, f: &mut dyn FnMut(Ref)) {
        if self.attachments_by_owner.is_empty() { return; }
        if let Some(indices) = self.attachments_by_owner.get(&owner) {
            for &ix in indices {
                if let FinalizerAction::Dart(action) = self.attachments[ix].action { f(action); }
            }
        }
    }

    /// Remoções deslocam posições; refaz o índice antes do próximo percurso forte.
    fn reindex_attachments(&mut self) {
        Self::reindex_attachment_list(&self.attachments, &mut self.attachments_by_owner);
    }

    fn reindex_attachment_list(attachments: &[FinalizerAttachment], index_by_owner: &mut crate::hash::HashMap<Ref, Vec<usize>>) {
        index_by_owner.clear();
        for (ix, a) in attachments.iter().enumerate() {
            if matches!(a.action, FinalizerAction::Dart(_)) {
                index_by_owner.entry(a.owner).or_default().push(ix);
            }
        }
    }

    /// Registra a aresta lateral sem transformar o dono em raiz global.
    pub(crate) fn add_attachment(&mut self, attachment: FinalizerAttachment) {
        self.check_live(attachment.owner);
        if let FinalizerAction::Dart(action) = attachment.action {
            self.check_live(action);
            if let Some(b) = self.space_block(attachment.owner) {
                self.arc_swapped(b, attachment.owner, 0, action);
            }
            self.remember_object(attachment.owner);
            if let Some(arc) = &mut self.arc { arc.estado.mark_side_edges(attachment.owner); }
            self.attachments_by_owner.entry(attachment.owner).or_default().push(self.attachments.len());
        }
        if let FinalizerAction::Native(_, _, bytes) = attachment.action {
            let delta = isize::try_from(bytes).expect("externalSize excede o tamanho de endereço");
            self.count_external(delta);
        }
        self.attachments.push(attachment);
    }

    /// Cancela os anexos ainda registrados da chave e solta suas arestas Dart.
    pub(crate) fn detach_finalizer(&mut self, owner: Ref, detach: Ref) {
        if owner == 0 { return; }
        let mut actions = Vec::new();
        let mut external = Vec::new();
        self.attachments.retain(|a| {
            if a.owner != owner || a.detach != detach { return true; }
            if let FinalizerAction::Dart(action) = a.action { actions.push(action); }
            if let FinalizerAction::Native(_, _, bytes) = a.action { external.push(bytes); }
            false
        });
        self.reindex_attachments();
        for action in actions { self.arc_raw_write(owner, action, 0); }
        for bytes in external { self.count_external(-(bytes as isize)); }
    }

    /// A coleta completa (o `dartforge_gc_collect` e os testes): marca a
    /// partir das raízes e varre tudo; o que sobrevive fica velho.
    pub fn collect(&mut self) {
        self.coletar(false);
    }

    /// Empilha em `dest` as raízes: as permanentes do runtime, os
    /// globais, as finalizações prontas e os quadros (os do runtime e a
    /// pilha-sombra do código gerado).
    fn root_list(&self, dest: &mut Vec<i64>) {
        dest.extend(self.message_owners.values().copied());
        dest.extend(self.code_owners.keys().copied());
        dest.extend(self.enum_values.values().copied());
        dest.extend(self.tearoffs.values().copied());
        dest.extend(self.literals.values().copied());
        dest.extend(self.globals.values().copied());
        dest.extend(self.runtime_roots.iter().copied().filter(|&h| h != 0));
        dest.extend(self.ready_finalizations.iter().copied());
        dest.extend(self.frames.iter().flat_map(|(_, roots, _)| roots.iter().copied()));
        visit_frames(|h| dest.push(h));
        let from_maps = dest.len();
        visit_frames_by_maps(|h| dest.push(h));
        if checked_walk() {
            check_walk(&dest[from_maps..]);
        }
    }

    /// `DARTFORGE_GC_VERIFICAR=1`, depois da marcação de uma coleta menor:
    /// uma travessia completa a partir das raízes, sem gerações, não pode
    /// achar jovem sem marca — seria uma referência de velho para jovem que
    /// a barreira de escrita não lembrou.
    ///
    /// # Panics
    /// Com a primeira referência assim.
    fn verify_minor_collection(&self) {
        let mut stack = Vec::new();
        self.root_list(&mut stack);
        let mut seen = crate::hash::HashSet::default();
        let mut origin: crate::hash::HashMap<i64, i64> = crate::hash::HashMap::default();
        while let Some(h) = stack.pop() {
            if !smi::e_handle(h) || h < 0 || !seen.insert(h) {
                continue;
            }
            let before = stack.len();
            if is_object(h) {
                if self.objects.block_of(h).is_none() && is_static(h) {
                    continue;
                }
                let b = self.live_block(h);
                // SAFETY: bloco vivo do espaço.
                #[allow(unsafe_code)]
                let estado = unsafe { (*b).estado };
                if estado == JOVEM {
                    // O pai: cid, estado, flags e a posição do filho no corpo.
                    let parent_node = origin.get(&h).copied();
                    let desc = |x: i64| {
                        let c = self.header(x);
                        let pos = self.words(x).iter().position(|&w| w == h);
                        format!("cid {} estado {} flags {:#x} n {} posição {pos:?}", c.class_id, c.estado, c.flags, c.n)
                    };
                    panic!(
                        "bug do coletor: jovem alcançável sem marca na coleta menor (barreira de escrita faltando)\nobjeto {h} (cid {}), alcançado de {parent_node:?} ({})",
                        self.header(h).class_id,
                        parent_node.map(desc).unwrap_or_default()
                    );
                }
                self.visit_strong_references(h, &mut |child| stack.push(child));
            } else {
                Self::invalid_handle(h);
            }
            for &f in &stack[before..] {
                origin.entry(f).or_insert(h);
            }
        }
    }

    /// Uma coleta: `minor` marca e varre só os jovens (os velhos contam como
    /// vivos, e os lembrados pela barreira de escrita entram como raízes);
    /// a completa, tudo. Em ambas o que sobrevive fica velho.
    fn coletar(&mut self, minor: bool) {
        // O ARC puro não rastreia: a drenagem decide a vida pelo RC e pelas
        // raízes diretas (docs/ARC-IMPLEMENTACAO.md §6).
        if self.arc.as_ref().is_some_and(|a| a.pure) {
            self.drain_arc(!minor);
            return;
        }
        check_collection_allowed();
        // No ARC a marcação é sempre a menor (os jovens); a vida dos velhos é
        // o RC, e a completa pedida é a rodada de ciclos e a reclamação
        // (`sync_arc`).
        let full_arc = self.arc.is_some() && !minor;
        let minor = minor || self.arc.is_some();
        if let Some(a) = &self.schedule {
            a.pending.set(false);
        }
        // O que a TLAB não usou volta a ser livre.
        self.return_tlabs();
        self.minor_collection = minor;
        self.stats.collections += 1;
        self.stats.roots_scanned += self.enum_values.len() as u64;
        self.stats.roots_scanned += self.tearoffs.len() as u64;
        self.stats.roots_scanned += self.literals.len() as u64;
        self.stats.roots_scanned += self.message_owners.len() as u64;
        self.stats.roots_scanned += self.code_owners.len() as u64;
        self.stats.roots_scanned += self
            .frames
            .iter()
            .map(|(_, roots, _)| roots.len() as u64)
            .sum::<u64>();
        if !minor {
            self.objects.clear_marks();
        }
        self.marking_work = 0;
        let mut pending_list = std::mem::take(&mut self.pending);
        pending_list.clear();
        self.root_list(&mut pending_list);
        if minor {
            // Os velhos lembrados: as referências deles podem ser jovens.
            for &b in &self.objects.remembered {
                // SAFETY: bloco lembrado, vivo.
                #[allow(unsafe_code)]
                let work = unsafe { self.objects.push_remembered(b, &mut pending_list) };
                self.marking_work += work;
                let owner = b as i64 + DESLOCAMENTO_DO_HANDLE;
                self.visit_attachment_edges(owner, &mut |action| pending_list.push(action));
            }
        }
        self.pending = pending_list;
        let mut live = self.mark_pending();
        // Os efêmeros: o valor de um portador vivo é alcançado quando a
        // chave é — o que pode tornar vivas outras chaves, até o ponto fixo.
        if !self.ephemerons.is_empty() {
            loop {
                let objects = &self.objects;
                let marked = |h: i64| marked_in_collection(objects, minor, h);
                let before = self.pending.len();
                for (&holder, &(key, value)) in &self.ephemerons {
                    if marked(holder) && (marked(key) || !smi::e_handle(key)) && smi::e_handle(value) && !marked(value) {
                        self.pending.push(value);
                    }
                }
                if self.pending.len() == before {
                    break;
                }
                live += self.mark_pending();
            }
        }
        // Tabelas laterais: só ficam os handles que sobreviveram (G6).
        // A verificação percorre o heap vivo inteiro: no `--gc-stress` (uma
        // menor por alocação), espaçada como as completas, para o custo por
        // alocação não crescer com o heap.
        if minor
            && self.verify
            && (!self.stress
                || self.minors_since_full % (self.work_in_full / WORK_PER_MINOR_UNDER_STRESS).max(1) as u64 == 0)
        {
            self.verify_minor_collection();
        }
        let promotion = if self.arc.is_some() { Some(self.promotion_data()) } else { None };
        let objects = &self.objects;
        let alive = |h: &i64| marked_in_collection(objects, minor, *h);
        self.weak_refs.retain(|holder, target| {
            if smi::e_handle(*target) && !alive(target) {
                *target = 0;
            }
            alive(holder)
        });
        self.ephemerons.retain(|holder, (key, value)| {
            if smi::e_handle(*key) && !alive(key) {
                *key = 0;
                *value = 0;
            }
            alive(holder)
        });
        // Na menor, só as marcas `late` gravadas desde a última coleta podem
        // ser de um jovem (os velhos não morrem nela): conferir só essas, e
        // não a tabela inteira (uma por objeto vivo com campo `late`).
        if minor {
            for key in self.new_lates.drain(..) {
                if !alive(&key.0) {
                    self.late_fields_initialized.remove(&key);
                }
            }
        } else {
            self.new_lates.clear();
            self.late_fields_initialized.retain(|(h, _)| alive(h));
        }
        let mut ready = Vec::new();
        let mut attachment_edges = Vec::new();
        let mut natives = Vec::new();
        self.attachments.retain_mut(|a| {
            if matches!(a.action, FinalizerAction::Dart(_)) && !alive(&a.owner) { return false; }
            if matches!(a.action, FinalizerAction::Native(_, _, _)) && smi::e_handle(a.owner) && !alive(&a.owner) {
                a.owner = 0;
            }
            if smi::e_handle(a.detach) && !alive(&a.detach) {
                a.detach = 0;
            }
            if alive(&a.value) {
                return true;
            }
            match a.action {
                FinalizerAction::Dart(action) => {
                        ready.push(action);
                        attachment_edges.push((a.owner, action));
                    },
                FinalizerAction::Native(f, token, bytes) => natives.push((f, token, bytes)),
            }
            false
        });
        if let Some(arc) = &mut self.arc {
            for &action in &ready {
                arc.estado.retain_ref(action).unwrap_or_else(|e| arc_failure(e));
            }
        }
        self.ready_finalizations.extend(ready);
        // As tabelas de identidade entre portas: purgadas pelo bit de marca
        // como as demais (quem precisa da constante viva a guarda num global,
        // o getter já grava lá; §2.8).
        self.permanentes.retain(|h| alive(h));
        self.constants.retain(|h, _| alive(h));
        self.tearoff_code.retain(|h, _| alive(h));
        let mut finalize = Vec::new();
        self.finalizable.retain(|h, &mut pair| {
            let stays = alive(h);
            if !stays {
                finalize.push(pair);
            }
            stays
        });
        self.reindex_attachments();
        for (owner, action) in attachment_edges {
            if !promotion.as_ref().is_some_and(|(promoted, _)| promoted.contains(&owner)) {
                self.arc_raw_write(owner, action, 0);
            }
        }
        // A estimativa: a menor desconta o que soltou (blocos, regiões grandes,
        // corpos de fora e anexos dos mortos); a completa a refaz dos vivos, com
        // os bytes externos.
        if minor {
            let (dead_set, released_bytes) = self.objects.sweep_young();
            self.stats.reclaimed += dead_set as u64;
            self.subtract_estimated_bytes(released_bytes);
        } else {
            let (dead_set, _, object_bytes) = self.objects.sweep();
            self.stats.reclaimed += dead_set as u64;
            self.set_estimated_bytes((object_bytes as u128).saturating_add(self.external));
        }
        for (finalizer, pair) in finalize {
            finalizer(pair);
        }
        for (f, token, bytes) in natives {
            self.count_external(-(bytes as isize));
            #[allow(unsafe_code)]
            // SAFETY: a `NativeFinalizerFunction` do anexo, `void f(void*)`;
            // como na VM, roda durante a coleta e não pode tocar o heap.
            let f: extern "C" fn(usize) = unsafe { std::mem::transmute(f) };
            f(token);
        }
        self.allocations = 0;
        self.young_bytes = 0;
        self.minor_collection = false;
        if let Some((promoted, remembered)) = promotion {
            self.sync_arc(full_arc, promoted, remembered);
        } else if minor {
            self.minors_since_full += 1;
        } else {
            self.minors_since_full = 0;
            self.work_in_full = self.marking_work + live;
            self.recompute_triggers(live);
        }
        if self.memory > 1 || (self.memory == 1 && !minor) {
            eprintln!("[memória] depois da coleta {}", if minor { "menor" } else { "completa" });
            eprint!("{}", self.memory_report());
        }
        if self.trace {
            eprintln!(
                "[gc] {} marcados={live} trabalho={} estimados={} gatilho={} paginas={} vivos={}",
                if minor { "menor" } else { "completa" },
                self.marking_work,
                self.stats.estimated_bytes,
                self.byte_threshold,
                self.objects.n_pages(),
                self.objects.live_set
            );
        }
    }

    /// A quebra da memória (`DARTFORGE_GC_MEMORIA=1`): o espaço de objetos, as
    /// tabelas laterais do runtime (entradas e bytes de capacidade) e o
    /// processo (residente, pico, heap do sistema confirmado).
    pub fn memory_report(&self) -> String {
        use std::fmt::Write;
        fn tabela<K, V>(m: &crate::hash::HashMap<K, V>) -> (usize, usize) {
            (m.len(), m.capacity() * (std::mem::size_of::<(K, V)>() + 1) / 1024)
        }
        fn item_set<K>(m: &crate::hash::HashSet<K>) -> (usize, usize) {
            (m.len(), m.capacity() * (std::mem::size_of::<K>() + 1) / 1024)
        }
        let mut s = self.objects.memory_report();
        let literals: usize = self.literals.keys().map(|k| k.capacity() * 2).sum::<usize>() / 1024;
        let _ = writeln!(
            s,
            "[memória] tabelas (entradas, KiB): globais {:?}, literais {:?} (+{literals} KiB de chaves), constantes {:?}, permanentes {:?}, enum {:?}, tearoffs {:?}, código de tearoff {:?}, late {:?}, fracas {:?}, efêmeros {:?}, finalizáveis {:?}; estimados {} KiB, gatilho {} KiB",
            tabela(&self.globals),
            tabela(&self.literals),
            tabela(&self.constants),
            item_set(&self.permanentes),
            tabela(&self.enum_values),
            tabela(&self.tearoffs),
            tabela(&self.tearoff_code),
            item_set(&self.late_fields_initialized),
            tabela(&self.weak_refs),
            tabela(&self.ephemerons),
            tabela(&self.finalizable),
            self.stats.estimated_bytes / 1024,
            self.byte_threshold / 1024
        );
        let (rss, peak, system_heap) = crate::espaco::process_memory();
        let _ = writeln!(
            s,
            "[memória] processo: residente {} KiB, pico {} KiB, heap do sistema confirmado {} KiB",
            rss / 1024,
            peak / 1024,
            system_heap / 1024
        );
        s
    }

    /// Os gatilhos da próxima coleta, a partir do que sobreviveu a esta.
    ///
    /// Sem teto, o heap cresce [`GROWTH`]% (o geométrico). Com teto, a
    /// próxima coleta vem quando se gastar metade da folga que resta — a
    /// histerese: uma coleta que achou quase tudo vivo não se repete na
    /// alocação seguinte (antes, passar da metade do teto coletava em TODA
    /// alocação, e um programa com muito dado vivo parava de andar). O passo
    /// mínimo garante progresso mesmo com a folga no fim; o teto em si é
    /// conferido à parte (`allocate`).
    fn recompute_triggers(&mut self, live_set: usize) {
        const MIN_STEP: usize = 256 * 1024;
        // [`GROWTH`]% do que sobreviveu, e não menos que três quartos do gatilho
        // anterior (a histerese: uma estrutura grande que morre e volta — a
        // lista refeita a cada rodada — não faz o gatilho despencar e voltar
        // a subir com uma coleta completa a cada dobra).
        // E pelo menos dois semiespaços jovens acima do que sobreviveu: com
        // pouco dado vivo, o dobro dele vinha antes do gatilho da coleta
        // menor, e toda coleta era completa (a árvore longa de
        // `objetos_escapam/arvores` remarcada 250 vezes).
        let growth = (self
            .stats
            .estimated_bytes
            .saturating_mul(self.growth))
            / 100;
        let growth = growth
            .max(self.stats.estimated_bytes.saturating_add(2 * self.young_limit))
            .max(self.byte_threshold - self.byte_threshold / 4);
        self.byte_threshold = if self.byte_limit == usize::MAX {
            growth
        } else {
            let folga = self.byte_limit.saturating_sub(self.total_bytes(0));
            growth.min(self.stats.estimated_bytes.saturating_add((folga / 2).max(MIN_STEP)))
        };
        // Não há mais gatilho por contagem para a coleta completa: ele
        // existia porque toda coleta varria a tabela de slots inteira, e com
        // poucos vivos coletava a cada 256 alocações (um `sort` de 100 mil
        // `int` com alguns `_Mint` percorria a lista toda a cada vez). A
        // coleta menor varre só os jovens e vem por bytes ou por
        // `YOUNG_COUNT` alocações; a completa, pelos bytes.
        let _ = live_set;
    }

    /// Obtém contadores sem percorrer os objetos ou suas raízes.
    pub fn stats(&self) -> HeapStats {
        HeapStats {
            live_objects: self.objects.live_set,
            reserved_slots: 0,
            permanent_roots: self.enum_values.len()
                + self.tearoffs.len()
                + self.literals.len(),
            ..self.stats
        }
    }



}

// ---------------------------------------------------------------------------
// A API do espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md §3.2): todo valor
// do runtime é um bloco do espaço de objetos; as vistas por tipo estão em
// `textos.rs`, `caixas.rs`, `listas.rs` e `tipadas.rs`.
// ---------------------------------------------------------------------------

/// Um valor sem representação de heap decidida (o que era a `TaggedValue`) nas
/// APIs do runtime (a caixa, se for preciso, é de quem grava numa posição `Ref`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Ref(Ref),
    Int(i64),
    Double(f64),
    Bool(bool),
}

/// As caixas estáticas de `false` e `true` (cid 5, `BRUTO`, `PERMANENTE`): o
/// cabeçalho e o valor, na forma de um bloco; o handle é o endereço + 2. O
/// código gerado as referencia como `@dartforge_falso`/`@dartforge_verdadeiro`
/// (`llvm/externs.rs`). São estáticos do runtime (§2.11): nunca marcados,
/// varridos nem gravados; [`is_static`] os reconhece pelo endereço.
#[allow(unsafe_code, non_upper_case_globals)]
#[unsafe(no_mangle)]
pub static dartforge_falso: [u64; 3] = [
    crate::layout::palavra_do_cabecalho(PERMANENTE, crate::layout::flags::BRUTO, 1, crate::layout::cid::BOOL),
    0,
    0,
];
#[allow(unsafe_code, non_upper_case_globals)]
#[unsafe(no_mangle)]
pub static dartforge_verdadeiro: [u64; 3] = [
    crate::layout::palavra_do_cabecalho(PERMANENTE, crate::layout::flags::BRUTO, 1, crate::layout::cid::BOOL),
    0,
    1,
];

/// As faixas `[início, end)` da seção de objetos estáticos (`.dfimg`) de cada
/// imagem carregada (o executável, a DLL do SDK): registradas por
/// `dartforge_registrar_imagem`, que o `@df.preparar_isolado` de cada imagem
/// chama. Um handle de objeto fora das páginas do heap só é aceito como
/// estático dentro de uma delas (a validação do `--gc-stress` e da verificação,
/// e a leitura do runtime); os estáticos são do processo, compartilhados por
/// todos os isolados.
static IMAGES: std::sync::RwLock<Vec<(usize, usize)>> = std::sync::RwLock::new(Vec::new());

/// Registra a seção de estáticos `[start, end)` de uma imagem (idempotente).
pub fn register_image(start: usize, end: usize) {
    if start >= end {
        return;
    }
    let mut v = IMAGES.write().unwrap_or_else(std::sync::PoisonError::into_inner);
    if !v.contains(&(start, end)) {
        v.push((start, end));
    }
}

/// As faixas das imagens registradas, na ordem do registro (o índice dos
/// literais estáticos, `textos.rs`).
pub fn images() -> Vec<(usize, usize)> {
    IMAGES.read().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
}

/// `h` é um objeto estático (`PERMANENTE`): uma das caixas de `bool` do runtime
/// ou um bloco da seção de estáticos de uma imagem registrada? Nunca lê memória
/// fora dessas faixas.
#[allow(unsafe_code)]
pub fn is_static(h: Ref) -> bool {
    if !is_object(h) {
        return false;
    }
    let b = (h - DESLOCAMENTO_DO_HANDLE) as usize;
    let from_runtime = b == std::ptr::addr_of!(dartforge_falso) as usize || b == std::ptr::addr_of!(dartforge_verdadeiro) as usize;
    let in_image = from_runtime || {
        let v = IMAGES.read().unwrap_or_else(std::sync::PoisonError::into_inner);
        v.iter().any(|&(i, f)| i <= b && b + std::mem::size_of::<Header>() <= f)
    };
    // SAFETY: `b` está numa faixa registrada (memória legível da imagem) ou é
    // uma das caixas do runtime.
    in_image && unsafe { (*(b as *const Header)).estado } == PERMANENTE
}


impl Heap {
    /// Um bloco zerado de `words` palavras de corpo, classe `cid` e `flags`
    /// (formato, forma compacta, cartões…), estado `JOVEM`; coleta antes se um
    /// gatilho pede (as referências que o chamador segura precisam de raiz). O `n`
    /// do cabeçalho é `words` (saturado): para `INSTANCIA`, use
    /// [`Heap::allocate_instance`]. Quem aloca uma `_List` geral grande acende
    /// `CARTOES` (`layout::has_cards`) e reserva as palavras deles
    /// (`layout::list_words`).
    pub fn allocate(&mut self, cid: i32, words: usize, flags: u8) -> Ref {
        debug_assert!(
            flags & flags_do_bloco::FORMA != flags_do_bloco::INSTANCIA || words <= 32,
            "INSTANCIA com mapa estendido por `alocar`: use `alocar_instancia`"
        );
        self.alocar_bloco(cid, words, flags, words)
    }
    /// Um objeto `INSTANCIA` de `fields` campos zerados (o `allocate_object` de hoje).
    pub fn allocate_instance(&mut self, cid: i32, fields: usize) -> Ref {
        self.allocate_object(i64::from(cid), fields)
    }
    /// O cabeçalho do bloco `h` (também de um estático). Pânico N4 para null,
    /// `Smi`, morto ou inválido.
    #[inline]
    #[allow(unsafe_code)]
    pub fn header(&self, h: Ref) -> &Header {
        let b = self.live_block(h);
        // SAFETY: bloco vivo de uma página do espaço (ou estático); o
        // empréstimo do heap o mantém (nenhuma coleta com `&self`).
        unsafe { &*b }
    }
    /// A classe de `h`: 1 para null, 2 para `Smi`, o cid do cabeçalho para um
    /// objeto. Pânico N4 para morto ou inválido.
    pub fn class(&self, h: Ref) -> i32 {
        if h == 0 {
            crate::layout::cid::NULL
        } else if smi::is_smi(h) {
            crate::layout::cid::SMI
        } else if is_object(h) {
            self.header(h).class_id
        } else {
            Self::invalid_handle(h)
        }
    }
    /// `h` é um objeto vivo do espaço, ou um estático (sem pânico)?
    #[allow(unsafe_code)]
    pub fn is_live_object(&self, h: Ref) -> bool {
        if !self.validate_handles {
            // SAFETY: `validate_handles` (um handle de objeto é um bloco ou um estático).
            return is_object(h) && unsafe { (*((h - DESLOCAMENTO_DO_HANDLE) as *const Header)).estado } != LIVRE;
        }
        match self.objects.block_of(h) {
            // SAFETY: bloco de uma página do espaço.
            Some(b) => (unsafe { (*b).estado }) != LIVRE,
            None => is_static(h),
        }
    }
    /// `h` é um objeto estático (seção da imagem, `PERMANENTE`)?
    #[allow(unsafe_code)]
    pub fn is_static(&self, h: Ref) -> bool {
        if !self.validate_handles {
            // SAFETY: `validate_handles` (um handle de objeto é um bloco ou um estático).
            return is_object(h) && unsafe { (*((h - DESLOCAMENTO_DO_HANDLE) as *const Header)).estado } == PERMANENTE;
        }
        self.objects.block_of(h).is_none() && is_static(h)
    }
    /// As palavras do corpo do bloco `b` (legível): o corpo `INSTANCIA` (o de
    /// fora, se houver) e o número de palavras dele.
    #[inline]
    #[allow(unsafe_code)]
    fn body_and_words(&self, b: *mut Header) -> (*mut Header, usize) {
        // SAFETY: bloco legível (de `live_block`).
        unsafe {
            if shape(b) == flags_do_bloco::INSTANCIA {
                let c = body(b);
                return (c, crate::layout::palavras_de_instancia(usize::from((*c).n)));
            }
            if (*b).estado == PERMANENTE {
                let n = usize::from((*b).n);
                if n != usize::from(u16::MAX) {
                    return (b, n);
                }
                // Um literal estático com mais de 65 534 palavras: o comprimento
                // diz (as strings são os únicos estáticos grandes).
                let c = (*b).class_id;
                assert!(crate::layout::cid::is_string(c), "bug do compilador: estático grande que não é string");
                let len = usize::try_from(*fields_of(b)).unwrap_or(0);
                return (b, crate::layout::palavras_de_texto(len, c == crate::layout::cid::TWO_BYTE_STRING));
            }
            (b, self.objects.body_words(b))
        }
    }
    /// O corpo inteiro de `h`, em palavras (a palavra 0 é `b+16`).
    #[inline]
    #[allow(unsafe_code)]
    pub fn words(&self, h: Ref) -> &[i64] {
        let (c, w) = self.body_and_words(self.live_block(h));
        // SAFETY: o corpo do bloco vivo tem `w` palavras; o empréstimo do heap
        // o mantém.
        unsafe { std::slice::from_raw_parts(fields_of(c), w) }
    }
    /// O corpo inteiro de `h`, gravável **sem barreira**: só `BRUTO`, ou antes de
    /// publicar o objeto (quem grava um `Ref` num objeto publicado chama
    /// [`Heap::remember`]/[`Heap::remember_element`]).
    #[inline]
    #[allow(unsafe_code)]
    pub fn words_mut(&mut self, h: Ref) -> &mut [i64] {
        let b = self.live_block(h);
        Self::check_writable(b);
        if self.arc.is_some() {
            self.arc_snapshot(b, h);
        }
        let (c, w) = self.body_and_words(b);
        // SAFETY: o corpo do bloco vivo tem `w` palavras; o empréstimo mutável
        // do heap é exclusivo.
        unsafe { std::slice::from_raw_parts_mut(fields_of(c), w) }
    }
    /// O corpo inteiro de `h`, em bytes.
    #[allow(unsafe_code)]
    pub fn bytes(&self, h: Ref) -> &[u8] {
        let p = self.words(h);
        // SAFETY: as mesmas palavras, vistas como bytes.
        unsafe { std::slice::from_raw_parts(p.as_ptr().cast(), p.len() * 8) }
    }
    /// O corpo inteiro de `h`, em bytes graváveis (as regras de [`Heap::words_mut`]).
    #[allow(unsafe_code)]
    pub fn bytes_mut(&mut self, h: Ref) -> &mut [u8] {
        let p = self.words_mut(h);
        // SAFETY: as mesmas palavras, vistas como bytes.
        unsafe { std::slice::from_raw_parts_mut(p.as_mut_ptr().cast(), p.len() * 8) }
    }
    /// Troca os bits `mask` de `flags` por `value`: só `FORMA`/`ELEMENTO` de uma
    /// `_List`/`_ImmutableList` (§2.16).
    #[allow(unsafe_code)]
    pub fn set_flags(&mut self, h: Ref, mask: u8, value: u8) {
        assert!(
            mask & !(flags_do_bloco::FORMA | flags_do_bloco::ELEMENTO) == 0,
            "bug do runtime: definir_flags só troca FORMA e ELEMENTO"
        );
        let b = self.live_block(h);
        Self::check_writable(b);
        // A forma decide que palavras são referências: o ARC fotografa as
        // contadas antes da troca.
        if self.arc.is_some() {
            self.arc_snapshot(b, h);
        }
        // SAFETY: bloco vivo e gravável.
        unsafe {
            assert!(crate::layout::cid::is_fixed_list((*b).class_id), "bug do runtime: definir_flags fora de _List/_ImmutableList");
            (*b).flags = ((*b).flags & !mask) | (value & mask);
        }
    }
    /// A barreira de elemento de §2.7 depois de gravar `v` no elemento `i` (índice
    /// a partir de 0, a palavra `i + 1`) do bloco `REFS` `b`.
    #[allow(unsafe_code)]
    fn element_barrier(&mut self, b: *mut Header, i: usize, v: Ref) {
        if !self.young_child(v) {
            return;
        }
        // SAFETY: bloco vivo do espaço.
        unsafe {
            let e = (*b).estado;
            if e != VELHO && e != REMEMBERED {
                return;
            }
            if (*b).flags & flags_do_bloco::CARTOES != 0 {
                let len = body_refs(b, self.objects.body_words(b));
                if i < len {
                    crate::espaco::dirty_card(b, len, i);
                }
            }
        }
        self.objects.remember(b);
    }
    /// O bloco `REFS` gravável de `h`.
    #[allow(unsafe_code)]
    fn refs_block(&self, h: Ref) -> *mut Header {
        let b = self.live_block(h);
        Self::check_writable(b);
        // SAFETY: bloco vivo.
        assert!(unsafe { shape(b) } == flags_do_bloco::REFS, "bug do runtime: gravar_ref num objeto que não é REFS");
        b
    }
    /// Grava o `Ref` `v` na palavra `word ≥ 1` do corpo `REFS` de `h`, com
    /// barreira (e cartão).
    #[allow(unsafe_code)]
    pub fn write_ref(&mut self, h: Ref, word: usize, v: Ref) {
        if smi::e_handle(v) && self.stress {
            self.check_live(v);
        }
        let b = self.refs_block(h);
        let (c, w) = self.body_and_words(b);
        assert!(word >= 1 && word < w, "palavra {word} fora do corpo REFS de {w} palavras");
        // SAFETY: `word < w`.
        let old = unsafe { std::mem::replace(&mut *fields_of(c).add(word), v) };
        if self.arc.is_some() {
            self.arc_swapped(b, h, old, v);
        }
        self.element_barrier(b, word - 1, v);
    }
    /// Copia `v` para as palavras `word..` do corpo `REFS` de `h`, com uma
    /// barreira só: num velho com cartões, suja os cartões da faixa inteira (a
    /// descompactação no lugar e o preenchimento de `listas.rs` contam com isso)
    /// e o lembra se algum valor é jovem.
    #[allow(unsafe_code)]
    pub fn write_refs(&mut self, h: Ref, word: usize, v: &[Ref]) {
        let b = self.refs_block(h);
        let (c, w) = self.body_and_words(b);
        assert!(word >= 1 && word + v.len() <= w, "palavras {word}..+{} fora do corpo REFS de {w} palavras", v.len());
        // O ARC conta cada troca: as palavras de antes, uma a uma.
        let olds: Vec<Ref> = if self.arc.is_some() {
            // SAFETY: a faixa está dentro do corpo.
            unsafe { std::slice::from_raw_parts(fields_of(c).add(word), v.len()).to_vec() }
        } else {
            Vec::new()
        };
        // SAFETY: a faixa está dentro do corpo; `v` não aponta para dentro do
        // heap mutável (é uma fatia do chamador).
        unsafe { std::ptr::copy(v.as_ptr(), fields_of(c).add(word), v.len()) };
        if v.is_empty() {
            return;
        }
        for (&old, &new) in olds.iter().zip(v) {
            self.arc_swapped(b, h, old, new);
        }
        // SAFETY: bloco vivo do espaço.
        unsafe {
            let e = (*b).estado;
            if e != VELHO && e != REMEMBERED {
                return;
            }
            if (*b).flags & flags_do_bloco::CARTOES != 0 {
                let len = body_refs(b, self.objects.body_words(b));
                let (de, until) = ((word - 1).min(len), (word - 1 + v.len()).min(len));
                let mut i = de;
                while i < until {
                    crate::espaco::dirty_card(b, len, i);
                    i = (i / crate::layout::ELEMENTOS_POR_CARTAO + 1) * crate::layout::ELEMENTOS_POR_CARTAO;
                }
            }
        }
        if v.iter().any(|&x| self.young_child(x)) {
            self.objects.remember(b);
        }
    }
    /// A barreira explícita (quem gravou por [`Heap::words_mut`] num objeto
    /// publicado): lembra o velho `h`; numa lista com cartões, suja todos.
    #[allow(unsafe_code)]
    pub fn remember(&mut self, h: Ref) {
        let Some(b) = self.space_block(h) else { return };
        // SAFETY: bloco do espaço.
        unsafe {
            let e = (*b).estado;
            if (e == VELHO || e == REMEMBERED) && shape(b) == flags_do_bloco::REFS && (*b).flags & flags_do_bloco::CARTOES != 0 {
                let len = body_refs(b, self.objects.body_words(b));
                crate::espaco::dirty_all_cards(b, len);
            }
        }
        self.objects.remember(b);
    }
    /// A barreira de elemento explícita: lembra o velho `h` e suja o cartão do
    /// elemento `i` (índice a partir de 0).
    #[allow(unsafe_code)]
    pub fn remember_element(&mut self, h: Ref, i: usize) {
        let Some(b) = self.space_block(h) else { return };
        // SAFETY: bloco do espaço.
        unsafe {
            let e = (*b).estado;
            if e != VELHO && e != REMEMBERED {
                return;
            }
            if shape(b) == flags_do_bloco::REFS && (*b).flags & flags_do_bloco::CARTOES != 0 {
                let len = body_refs(b, self.objects.body_words(b));
                if i < len {
                    crate::espaco::dirty_card(b, len, i);
                }
            }
        }
        self.objects.remember(b);
    }
    /// Anexa ao bloco `h` (`ATTACHMENT`) o ponteiro nativo `ptr`, que ocupa `bytes`
    /// bytes fora do heap: gravado em `b+16`, `release(ptr)` quando o dono morre
    /// (ou quando o heap acaba). Os bytes contam para os gatilhos.
    ///
    /// # Safety
    /// `release(ptr)` é a única liberação de `ptr`, e é válida; o corpo de `h` tem
    /// pelo menos uma palavra e é `BRUTO`.
    #[allow(unsafe_code)]
    pub unsafe fn attach(&mut self, h: Ref, release: unsafe fn(*mut u8), ptr: *mut u8, bytes: usize) {
        let b = self.live_block(h);
        Self::check_writable(b);
        // SAFETY: o contrato da função; bloco vivo com a palavra 0.
        unsafe {
            // O bloco pode ter nascido com `ATTACHMENT` (`allocate(.., BRUTO | ATTACHMENT)`, a
            // forma de §2.4): o duplicado é um ponteiro já gravado.
            assert!(
                (*b).flags & flags_do_bloco::ATTACHMENT == 0 || *fields_of(b) == 0,
                "bug do runtime: anexo duplicado"
            );
            (*b).flags |= flags_do_bloco::ATTACHMENT;
            *fields_of(b) = ptr as i64;
        }
        self.objects.attach(b, release, ptr, bytes);
        self.add_estimated_bytes(bytes);
        self.young_bytes = self.young_bytes.saturating_add(bytes);
    }
    /// Os bytes do anexo de `h` passam a ser `bytes` (o acumulador do
    /// `StringBuffer` cresceu): a diferença entra nos gatilhos de coleta, como
    /// [`Heap::count_external`], e sai quando o dono morre.
    pub fn adjust_attachment(&mut self, h: Ref, bytes: usize) {
        let b = self.live_block(h);
        let delta = self.objects.adjust_attachment(b, bytes);
        if delta >= 0 {
            self.add_estimated_bytes(delta.unsigned_abs());
            self.young_bytes = self.young_bytes.saturating_add(delta.unsigned_abs());
        } else {
            self.subtract_estimated_bytes(delta.unsigned_abs());
        }
    }
    /// O ponteiro nativo anexado a `h` (nulo sem anexo).
    #[allow(unsafe_code)]
    pub fn attachment(&self, h: Ref) -> *mut u8 {
        let b = self.live_block(h);
        // SAFETY: bloco vivo.
        unsafe { if (*b).flags & flags_do_bloco::ATTACHMENT != 0 { *fields_of(b) as *mut u8 } else { std::ptr::null_mut() } }
    }
    /// Projeta a estimativa sem perder bytes que serão descontados posteriormente.
    fn set_estimated_bytes(&mut self, bytes: u128) {
        self.exact_estimated_bytes = bytes;
        self.stats.estimated_bytes = usize::try_from(bytes).unwrap_or(usize::MAX);
        self.stats.peak_estimated_bytes = self.stats.peak_estimated_bytes.max(self.stats.estimated_bytes);
    }

    fn add_estimated_bytes(&mut self, bytes: usize) {
        self.set_estimated_bytes(self.exact_estimated_bytes.saturating_add(bytes as u128));
    }

    fn subtract_estimated_bytes(&mut self, bytes: usize) {
        self.set_estimated_bytes(self.exact_estimated_bytes.saturating_sub(bytes as u128));
    }

    /// Soma `delta` aos bytes de fora do heap (contam para os gatilhos até quem os
    /// contou descontá-los).
    pub fn count_external(&mut self, delta: isize) {
        if delta >= 0 {
            let d = delta.unsigned_abs();
            self.external = self.external.saturating_add(d as u128);
            self.add_estimated_bytes(d);
            self.young_bytes = self.young_bytes.saturating_add(d);
        } else {
            let d = delta.unsigned_abs();
            self.external = self.external.saturating_sub(d as u128);
            self.subtract_estimated_bytes(d);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Enums preservam identidade e nomes sem frames externos, mesmo em stress.
    #[test]
    fn enum_singletons_survive_collection_and_keep_nominal_identity() {
        let mut heap = Heap::new(true);
        let first = heap.enum_value(1, 0, "red");
        let second = heap.enum_value(1, 1, "blue");
        let other_class = heap.enum_value(2, 0, "red");
        heap.collect();
        assert_eq!(heap.enum_value(1, 0, "red"), first);
        assert_ne!(first, second);
        assert_ne!(first, other_class);
        let fields = heap.object(first).expect("enum deve ser objeto");
        assert_eq!(fields.class_id, 1);
        assert_eq!(fields.field(0), (0, false));
        assert!(heap.text(fields.field(1).0).is_some_and(|name| name == "red"));
        assert_eq!(heap.stats().permanent_roots, 3);
        assert_eq!(heap.stats().live_objects, 6);
        assert_eq!(heap.stats().root_slots, 0);
    }
    /// Um único objeto raiz mantém transitivamente ciclos e strings de seus campos.
    #[test]
    fn tagged_edges_keep_unrooted_children_alive() {
        let mut heap = Heap::new(true);
        let outer = heap.push_frame();
        let parent = heap.new_object(1, &[(0, true)]);
        heap.root(outer, parent);
        let inner = heap.push_frame();
        let text = heap.allocate_str("ação 🦀");
        heap.root(inner, text);
        heap.set(parent, 0, text, true);
        heap.pop_frame(inner);
        heap.collect();
        assert!(heap.text(text).is_some_and(|s| s == "ação 🦀"));
        heap.set(parent, 0, 0, true);
        heap.collect();
        assert!(!heap.is_live_object(text));
        heap.pop_frame(outer);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 0);
    }
    /// Ciclos sobrevivem por raiz, depois são coletados e seus slots reutilizados.
    #[test]
    fn cycles_and_slot_reuse() {
        let mut heap = Heap::new(true);
        let frame = heap.push_frame();
        let a = heap.new_object(1, &[(0, true)]);
        heap.root(frame, a);
        let b = heap.new_object(2, &[(a, true)]);
        heap.root(frame, b);
        heap.set(a, 0, b, true);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 2);
        heap.pop_frame(frame);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 0);
        // Os blocos dos mortos voltam à lista livre do tamanho deles.
        let c = heap.new_object(3, &[(0, false)]);
        assert!(c == a || c == b);
        assert_eq!(heap.stats().live_objects, 1);
    }
    /// Bits escalares coincidentes com handles não retêm objetos.
    #[test]
    fn scalar_bits_do_not_trace() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame();
        let text = heap.allocate_str("descartável");
        let object = heap.new_object(1, &[(text, false)]);
        heap.root(frame, object);
        heap.collect();
        assert!(!heap.is_live_object(text));
        assert!(heap.is_live_object(object));
    }
    /// Frames aninhados preservam argumentos; stress coleta durante milhares de alocações.
    #[test]
    fn nested_frames_stress() {
        let mut heap = Heap::new(true);
        let outer = heap.push_frame();
        let permanent = heap.allocate_str("vivo");
        heap.root(outer, permanent);
        for _ in 0..1000 {
            let inner = heap.push_frame();
            let temporary = heap.allocate_str("temporário");
            heap.root(inner, temporary);
            heap.collect();
            assert!(heap.text(permanent).is_some_and(|s| s == "vivo"));
            heap.pop_frame(inner);
        }
        heap.collect();
        assert_eq!(heap.stats().live_objects, 1);
    }
}

#[cfg(test)]
mod object_space {
    //! O espaço de objetos: blocos por número de campos, handles com o bit
    //! 1, listas livres refeitas na varredura e páginas soltas.
    use super::*;


    #[test]
    fn object_handles_and_scalars_are_distinct() {
        let mut heap = Heap::new(false);
        let o = heap.allocate_object(7, 3);
        let s = heap.allocate_str("x");
        assert!(is_object(o) && is_object(s));
        assert!(matches!(heap.object(o), Some(f) if f.class_id == 7 && f.len() == 3));
        // A string não é `INSTANCIA`: não tem vista de campos.
        assert!(heap.object(s).is_none());
        // Escalares quaisquer não são objetos vivos (nem derrubam a consulta).
        for x in [2, 6, 18, o + 4, o - 4, o + 8, -2, i64::MAX - 1] {
            assert!(!heap.is_live_object(x) || x == o || x == s, "{x}");
        }
        heap.set(o, 1, s, true);
        assert_eq!(heap.object(o).expect("objeto").field(1), (s, true));
    }

    #[test]
    fn collection_returns_blocks_and_keeps_reachable() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame_with_slots(1);
        // Uma lista ligada de 10 mil nós alcançável e 10 mil mortos.
        let mut head = 0;
        for i in 0..10_000 {
            let garbage = heap.allocate_object(1, 2);
            heap.set(garbage, 0, i, false);
            let no = heap.allocate_object(2, 2);
            heap.set(no, 0, i, false);
            heap.set(no, 1, head, true);
            head = no;
            heap.set_root(frame, 0, head);
        }
        heap.collect();
        assert_eq!(heap.stats().live_objects, 10_000);
        let mut total_sum = 0;
        let mut no = head;
        while no != 0 {
            let fields = heap.object(no).expect("objeto");
            total_sum += fields.field(0).0;
            no = fields.field(1).0;
        }
        assert_eq!(total_sum, (0..10_000).sum::<i64>());
        heap.pop_frame(frame);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 0);
        // As páginas ficam enquanto o pico recente de vivos as pede (a
        // estrutura pode voltar); o pico decai a cada coleta completa, e as
        // vazias além da folga voltam ao sistema.
        let retained_list = heap.objects.pages.len();
        assert!(retained_list > 2);
        for _ in 0..20 {
            heap.collect();
        }
        assert!(heap.objects.pages.len() <= 2, "{} páginas de {retained_list}", heap.objects.pages.len());
    }

    #[test]
    fn growing_object_followed_through_field_pointer() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame();
        let o = heap.allocate_object(3, 1);
        heap.root(frame, o);
        heap.set(o, 0, 41, false);
        heap.ensure_fields(o, 5);
        heap.set(o, 4, 42, false);
        heap.collect();
        let fields = heap.object(o).expect("objeto");
        assert_eq!((fields.len(), fields.field(0).0, fields.field(4).0), (5, 41, 42));
        heap.pop_frame(frame);
        heap.collect();
        // O bloco voltou ao molde: um objeto novo de 1 campo o reusa zerado.
        let new = heap.allocate_object(4, 1);
        assert_eq!(new, o);
        assert!(matches!(heap.object(new), Some(f) if f.class_id == 4 && f.len() == 1 && f.field(0) == (0, false)));
    }

    #[test]
    fn identity_hash_is_stable_and_distinct() {
        let mut heap = Heap::new(false);
        let a = heap.allocate_object(1, 0);
        let b = heap.allocate_object(1, 0);
        let ha = heap.identity_hash(a).expect("objeto");
        assert_eq!(heap.identity_hash(a), Some(ha));
        assert_ne!(heap.identity_hash(b), Some(ha));
        assert!(ha > 0 && ha <= 0x3fff_ffff);
    }

    /// A alocação em linha do código gerado: avança o cursor da faixa da
    /// TLAB e grava o cabeçalho (`llvm/mod.rs`).
    #[allow(unsafe_code)]
    fn allocate_like_generated_code(heap: &mut Heap, class_id: i64, n: usize) -> i64 {
        // A TLAB é por palavras do corpo (`n ≤ TLAB_N` campos: `max(n, 1)`).
        let w = crate::layout::palavras_de_instancia(n);
        let size = block_size(n);
        let b = CONTEXTO.with(|c| {
            let (cursor, end) = (c.tlab[w][0].get(), c.tlab[w][1].get());
            if (end as usize).saturating_sub(cursor as usize) < size {
                return None;
            }
            c.tlab[w][0].set(cursor.wrapping_add(size));
            Some(cursor)
        });
        let Some(b) = b else {
            let h = heap.allocate_object(class_id, n);
            heap.refill_tlab(w);
            return h;
        };
        // SAFETY: bloco livre da faixa.
        unsafe {
            *b.add(16).cast::<i64>() = 0;
            *b.cast::<u64>() = 1 | (n as u64) << 16 | (class_id as u64) << 32;
        }
        b as i64 + DESLOCAMENTO_DO_HANDLE
    }

    #[test]
    fn tlab_ranges_survive_and_return_to_list() {
        // Numa thread nova: o `Context` dela começa sem TLAB.
        std::thread::spawn(|| {
            let mut heap = Heap::for_isolate(false);
            let frame = heap.push_frame_with_slots(1);
            let mut head = 0;
            let mut garbage = Vec::new();
            for i in 0..10_000 {
                let link = allocate_like_generated_code(&mut heap, 5, 2);
                heap.set(link, 0, smi::de(i).unwrap(), false);
                heap.set(link, 1, head, true);
                if i % 3 == 0 {
                    head = link;
                    heap.set_root(frame, 0, head);
                } else {
                    garbage.push(link);
                }
                if i % 2_500 == 1_249 {
                    heap.coletar(i % 5_000 != 1_249);
                }
            }
            heap.coletar(true);
            let mut n = 0;
            let mut h = head;
            while h != 0 {
                let o = heap.object(h).expect("elo vivo");
                assert_eq!(o.class_id, 5);
                n += 1;
                h = o.field(1).0;
            }
            assert_eq!(n, 3_334);
            assert_eq!(heap.stats().live_objects, 3_334);
            // Os mortos voltaram à lista: alocar de novo não cresce o heap.
            let pages = heap.objects.pages.len();
            for _ in 0..2_000 {
                allocate_like_generated_code(&mut heap, 6, 2);
            }
            assert_eq!(heap.objects.pages.len(), pages);
            heap.set_root(frame, 0, 0);
            heap.coletar(false);
            assert_eq!(heap.stats().live_objects, 0);
        })
        .join()
        .unwrap();
    }

    #[test]
    fn sweep_without_touching_dead_hands_out_zeroed_blocks() {
        // O caminho de fora dos testes: as varreduras só leem o mapa de
        // marcas, e a entrega zera a faixa suja.
        std::thread::spawn(|| {
            let mut heap = Heap::for_isolate(false);
            heap.objects.clear_dead = false;
            let frame = heap.push_frame_with_slots(1);
            let mut head = 0;
            for round in 0..6 {
                for i in 0..20_000 {
                    let link = allocate_like_generated_code(&mut heap, 5, 3);
                    // O código gerado só zera o primeiro campo: o resto (e o
                    // mapa de referências) tem de vir zerado da entrega.
                    let o = heap.object(link).expect("recém-alocado");
                    assert_eq!((o.field(1), o.field(2)), ((0, false), (0, false)), "bloco sujo entregue");
                    heap.set(link, 0, smi::de(i).unwrap(), false);
                    heap.set(link, 1, head, true);
                    heap.set(link, 2, 77, false);
                    if i % 4 == 0 {
                        head = link;
                        heap.set_root(frame, 0, head);
                    }
                    if i % 3_000 == 2_999 {
                        heap.coletar(i % 9_000 != 8_999);
                    }
                }
                // A lista inteira sobrevive; a cada rodada, a de antes morre.
                let mut n = 0;
                let mut h = head;
                while h != 0 {
                    let o = heap.object(h).expect("elo vivo");
                    assert_eq!((o.class_id, o.field(2).0), (5, 77));
                    n += 1;
                    h = o.field(1).0;
                }
                assert_eq!(n, 5_000 * (round % 2 + 1));
                if round % 2 == 1 {
                    head = 0;
                    heap.set_root(frame, 0, 0);
                    heap.coletar(false);
                    assert_eq!(heap.stats().live_objects, 0);
                }
            }
        })
        .join()
        .unwrap();
    }

    #[test]
    fn minor_collection_follows_remembered_and_releases_dead_young() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame_with_slots(1);
        let old_gen = heap.allocate_object(1, 1);
        heap.set_root(frame, 0, old_gen);
        heap.coletar(false);
        // Jovem só alcançável pelo velho (a barreira de `set` o lembra).
        let young = heap.allocate_object(2, 1);
        heap.set(young, 0, 7, false);
        heap.set(old_gen, 0, young, true);
        let garbage = heap.allocate_object(3, 1);
        heap.coletar(true);
        assert!(matches!(heap.object(young), Some(f) if f.class_id == 2 && f.field(0).0 == 7));
        assert!(!heap.is_live_object(garbage));
        assert_eq!(heap.stats().live_objects, 2);
        // Depois dela, o jovem ficou velho: sem barreira nova, outra menor
        // não o percorre, e ele continua vivo.
        heap.coletar(true);
        assert!(heap.is_live_object(young));
        // A completa solta o que só os velhos mortos alcançavam.
        heap.set_root(frame, 0, 0);
        heap.coletar(false);
        assert_eq!(heap.stats().live_objects, 0);
    }

    #[test]
    #[should_panic(expected = "barreira de escrita faltando")]
    fn verification_finds_missing_barrier() {
        let mut heap = Heap::new(false);
        heap.verify = true;
        let frame = heap.push_frame_with_slots(1);
        let old_gen = heap.allocate_object(1, 1);
        heap.set_root(frame, 0, old_gen);
        heap.coletar(false);
        let young = heap.allocate_object(2, 0);
        // Grava sem barreira (como faria código gerado sem ela).
        let b = heap.objects.block_of(old_gen).expect("objeto");
        #[allow(unsafe_code)]
        // SAFETY: bloco vivo de 1 campo.
        unsafe {
            *fields_of(b) = young;
            mark_reference(b, 0, true);
        }
        heap.coletar(true);
    }

    #[test]
    #[should_panic(expected = "barreira de escrita faltando")]
    fn verification_finds_missing_barrier_in_side_attachment() {
        let mut heap = Heap::new(false);
        heap.verify = true;
        let frame = heap.push_frame_with_slots(1);
        let owner = heap.allocate_object(1, 0);
        heap.set_root(frame, 0, owner);
        heap.coletar(false);
        let action = heap.allocate_object(2, 0);
        // Sabota a publicação: não chama a barreira de adicionar_anexo.
        heap.attachments.push(FinalizerAttachment {
            owner, value: owner, detach: 0, action: FinalizerAction::Dart(action),
        });
        heap.reindex_attachments();
        heap.coletar(true);
    }

    #[test]
    fn verification_accepts_side_attachment_barrier() {
        let mut heap = Heap::new(false);
        heap.verify = true;
        let frame = heap.push_frame_with_slots(1);
        let owner = heap.allocate_object(1, 0);
        heap.set_root(frame, 0, owner);
        heap.coletar(false);
        let action = heap.allocate_object(2, 0);
        heap.add_attachment(FinalizerAttachment {
            owner, value: owner, detach: 0, action: FinalizerAction::Dart(action),
        });
        heap.coletar(true);
        assert!(heap.is_live_object(action));
        heap.pop_frame(frame);
        heap.collect();
        assert!(!heap.is_live_object(action));
    }

    /// A carga de `bench/performance/objetos_escapam.dart` (`lista_ligada`)
    /// direto no heap, para medir o coletor sem compilar Dart:
    /// `cargo test --release -p dartforge-runtime lista_ligada_no_heap --
    /// --ignored --nocapture` (com `DARTFORGE_GC_RASTRO=1`, as coletas).
    #[test]
    #[ignore = "medição"]
    fn linked_list_in_heap() {
        let mut heap = Heap::new(false);
        heap.trace = std::env::var("DARTFORGE_GC_RASTRO").as_deref() == Ok("1");
        let frame = heap.push_frame_with_slots(1);
        for _ in 0..6 {
            let t = std::time::Instant::now();
            let mut cab = 0;
            for i in 0..1_000_000 {
                let no = heap.allocate_object(1, 2);
                heap.set(no, 0, i, false);
                heap.set(no, 1, cab, true);
                cab = no;
                heap.set_root(frame, 0, cab);
            }
            eprintln!("rodada: {:?} coletas={}", t.elapsed(), heap.stats().collections);
        }
    }

    #[test]
    fn reference_map_beyond_32_fields() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame_with_slots(1);
        let o = heap.allocate_object(1, 40);
        heap.set_root(frame, 0, o);
        let mut children = Vec::new();
        for i in [0usize, 31, 32, 39] {
            let f = heap.allocate_object(2, 0);
            heap.set(o, i as i64, f, true);
            children.push(f);
        }
        // Um escalar que parece handle não segura nada.
        let garbage = heap.allocate_object(3, 0);
        heap.set(o, 33, garbage, false);
        heap.coletar(false);
        for f in &children {
            assert!(heap.object(*f).is_some());
        }
        assert!(heap.object(garbage).is_none());
        let obj = heap.object(o).expect("objeto");
        assert_eq!(obj.field(32), (children[2], true));
        assert!(!obj.field(33).1);
        // Trocar a referência por escalar apaga o bit.
        heap.set(o, 39, 5, false);
        heap.coletar(false);
        assert!(heap.object(children[3]).is_none());
    }

    #[test]
    fn out_of_line_body_followed_by_collection_and_barrier() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame_with_slots(1);
        let o = heap.allocate_object(1, 1);
        heap.set_root(frame, 0, o);
        heap.coletar(false);
        // O objeto velho cresce (corpo de fora) e recebe um jovem no campo
        // novo: a coleta menor o acha pelo lembrado.
        heap.ensure_fields(o, 3);
        let young = heap.allocate_object(2, 0);
        heap.set(o, 2, young, true);
        heap.coletar(true);
        assert!(heap.object(young).is_some());
        assert_eq!(heap.object(o).expect("objeto").len(), 3);
        heap.pop_frame(frame);
        heap.coletar(false);
        assert_eq!(heap.stats().live_objects, 0);
    }

    #[test]
    fn large_object_has_own_page() {
        // Acima da maior classe média (`layout::LARGEST_MEDIUM` palavras): uma
        // região grande só dele, devolvida na coleta completa.
        let mut heap = Heap::new(false);
        let n = crate::layout::LARGEST_MEDIUM + 100;
        let o = heap.allocate_object(9, n);
        heap.set(o, (n - 1) as i64, 5, false);
        assert!(matches!(heap.object(o), Some(f) if f.field(n - 1).0 == 5));
        heap.collect();
        assert!(!heap.is_live_object(o));
        assert_eq!(heap.objects.n_pages(), 0);
    }
}

#[cfg(test)]
mod handle_failures {
    //! N4: cada contrato quebrado tem mensagem própria.
    use super::*;

    #[test]
    #[should_panic(expected = "handle null (0) desreferenciado")]
    fn null_dereferenced() {
        Heap::new(false).header(0);
    }

    #[test]
    #[should_panic(expected = "handle negativo")]
    fn negative_handle() {
        Heap::new(false).header(-2);
    }

    #[test]
    #[should_panic(expected = "handle além da tabela")]
    fn handle_beyond_table() {
        let mut heap = Heap::new(false);
        heap.allocate_str("x");
        heap.header(42);
    }

    #[test]
    #[should_panic(expected = "handle já coletado")]
    fn collected_handle() {
        let mut heap = Heap::new(false);
        let h = heap.allocate_str("x");
        heap.collect();
        heap.header(h);
    }
}

#[cfg(test)]
mod runtime_roots {
    //! G6: raízes que não moram num frame, e as tabelas laterais.
    use super::*;

    #[test]
    fn pending_exception_survives_collection_without_frame() {
        let mut heap = Heap::new(true);
        let error = heap.allocate_str("falhou");
        heap.set_runtime_root(0, error);
        heap.allocate_str("outra");
        heap.collect();
        assert!(heap.text(error).is_some_and(|s| s == "falhou"));
        heap.set_runtime_root(0, 0);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 0);
    }

    #[test]
    fn collection_without_open_frame() {
        // O portão antigo só coletava com frame; agora o limiar basta (a
        // coleta menor, por contagem de alocações).
        let mut heap = Heap::new(false);
        let n = YOUNG_COUNT + 1000;
        for _ in 0..n {
            heap.allocate_str("lixo");
        }
        assert!(heap.stats().collections > 0);
        assert!(heap.stats().live_objects < n);
    }

    #[test]
    fn side_table_purged_when_block_is_reused() {
        let mut heap = Heap::new(false);
        let o = heap.allocate_object(200, 1);
        heap.mark_late((o, 0));
        heap.mark_late((o, -1));
        heap.mark_late((o, -2));
        heap.collect();
        assert!(heap.late_fields_initialized.is_empty());
        let new_area = heap.allocate_object(200, 1);
        assert!(!heap.late_initialized((new_area, 0)));
        assert!(!heap.late_initialized((new_area, -1)));
        assert!(!heap.late_initialized((new_area, -2)));
    }

    #[test]
    fn minor_collection_purges_only_dead_young_marks() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame_with_slots(1);
        let old_gen = heap.allocate_object(200, 0);
        heap.set_root(frame, 0, old_gen);
        heap.mark_late((old_gen, 0));
        heap.coletar(true);
        assert!(heap.late_initialized((old_gen, 0)), "o velho vivo continua marcado");
        let young = heap.allocate_object(200, 0);
        heap.mark_late((young, 3));
        heap.coletar(true);
        assert!(!heap.late_initialized((young, 3)), "o jovem morto perde a marca");
        assert!(heap.late_initialized((old_gen, 0)));
        heap.pop_frame(frame);
    }

}

#[cfg(test)]
mod review_tests {
    use super::*;
    /// Igualdade preserva Unicode, NUL embutido e nulidade sem normalização implícita.
    #[test]
    fn unicode_nul_nullable_and_concat() {
        let mut heap = Heap::new(true);
        let frame = heap.push_frame();
        let a = heap.allocate_str("á\0🦀");
        heap.root(frame, a);
        let b = heap.allocate_str("á\0🦀");
        heap.root(frame, b);
        assert_ne!(a, b);
        assert!(heap.strings_equal(a, b));
        let c = heap.allocate_str("a\u{301}\0🦀");
        heap.root(frame, c);
        assert!(!heap.strings_equal(a, c));
    }
    /// Retorno transfere raiz sem coleta entre pop e registro no chamador.
    #[test]
    fn returned_handles_and_identity() {
        let mut heap = Heap::new(true);
        let caller = heap.push_frame();
        let callee = heap.push_frame();
        let a = heap.new_object(7, &[]);
        heap.root(callee, a);
        heap.pop_frame(callee);
        heap.root(caller, a);
        let b = heap.new_object(7, &[]);
        heap.root(caller, b);
        assert_ne!(a, b);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 2);
    }
    /// Tracing e destruição de um ciclo grande não dependem da pilha de chamadas.
    #[test]
    fn large_cycle_is_iterative() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame();
        let first = heap.new_object(1, &[(0, true)]);
        heap.root(frame, first);
        let mut previous = first;
        for _ in 1..20_000 {
            let next = heap.new_object(1, &[(first, true)]);
            heap.set(previous, 0, next, true);
            previous = next;
        }
        heap.collect();
        assert_eq!(heap.stats().live_objects, 20_000);
        heap.pop_frame(frame);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 0);
        assert_eq!(heap.stats().reclaimed, 20_000);
    }
    /// Microbenchmark reproduzível: tempos incluem raízes e coletas automáticas declaradas.
    #[test]
    #[ignore = "microbenchmark; execute em release com --ignored --nocapture"]
    fn gc_microbenchmark() {
        for (count, stress) in [(100_000, false), (2_000, true)] {
            let mut heap = Heap::new(stress);
            let frame = heap.push_frame();
            let started = std::time::Instant::now();
            for index in 0..count {
                let handle = heap.new_object(1, &[(index, false)]);
                heap.root(frame, handle);
                std::hint::black_box(handle);
            }
            let allocation_ns = started.elapsed().as_nanos();
            let started = std::time::Instant::now();
            heap.collect();
            let collect_live_ns = started.elapsed().as_nanos();
            heap.pop_frame(frame);
            let started = std::time::Instant::now();
            heap.collect();
            let collect_dead_ns = started.elapsed().as_nanos();
            let stats = heap.stats();
            println!(
                "count={count} stress={stress} alloc_root_auto_gc_ns={allocation_ns} collect_live_ns={collect_live_ns} collect_dead_ns={collect_dead_ns} stats={stats:?}"
            );
            assert_eq!(stats.live_objects, 0);
            assert_eq!(stats.reclaimed, count as u64);
        }
    }
}

#[cfg(test)]
mod fixed_root_tests {
    use super::*;
    /// Slots substituídos não crescem com iterações; cópias locais têm raízes independentes.
    #[test]
    fn fixed_slots_preserve_copies_and_nested_returns() {
        let mut heap = Heap::new(true);
        let frame = heap.push_frame_with_slots(2);
        let kept = heap.allocate_str("keep");
        heap.set_root(frame, 0, kept);
        heap.set_root(frame, 1, kept);
        for _ in 0..1000 {
            let inner = heap.push_frame_with_slots(1);
            let temporary = heap.allocate_str("next");
            heap.set_root(inner, 0, temporary);
            heap.pop_frame(inner);
            heap.set_root(frame, 0, temporary);
            heap.collect();
            assert!(heap.text(kept).is_some_and(|s| s == "keep"));
        }
        assert_eq!(heap.stats().peak_root_slots, 3);
        assert_eq!(heap.stats().live_roots, 2);
        heap.set_root(frame, 0, 0);
        heap.set_root(frame, 1, 0);
        heap.collect();
        assert_eq!(heap.stats().live_roots, 0);
        assert_eq!(heap.stats().estimated_bytes, 0);
        assert_eq!(heap.stats().live_objects, 0);
        assert!(heap.stats().reserved_slots <= 3);
        heap.pop_frame(frame);
        assert_eq!(heap.stats().root_slots, 0);
    }
    /// Payload grande dispara coleta antes do limiar por quantidade de objetos.
    #[test]
    fn byte_trigger_collects_large_payloads() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame_with_slots(1);
        for _ in 0..8 {
            let handle = heap.allocate(crate::layout::cid::tipada(1), 2 * 1024 * 1024 / 8, crate::layout::flags::BRUTO);
            heap.set_root(frame, 0, handle);
        }
        let stats = heap.stats();
        assert!(stats.collections >= 3);
        assert!(stats.reclaimed >= 5);
        // O pico: o jovem entre coletas menores e o lixo velho (cada payload
        // enraizado vira velho e morre na volta seguinte) até a completa,
        // que vem dois semiespaços jovens acima do que sobreviveu.
        assert!(stats.peak_estimated_bytes < 2 * YOUNG_LIMIT + 3 * 2 * 1024 * 1024, "{}", stats.peak_estimated_bytes);
        heap.pop_frame(frame);
        heap.collect();
        assert_eq!(heap.stats().estimated_bytes, 0);
    }
    /// Mede alocações transientes com uma única raiz sobrescrita, sem alegar superioridade.
    #[test]
    #[ignore = "microbenchmark de slots fixos; execute release --ignored --nocapture"]
    fn fixed_slot_microbenchmark() {
        for stress in [false, true] {
            let mut heap = Heap::new(stress);
            let frame = heap.push_frame_with_slots(1);
            let start = std::time::Instant::now();
            for _ in 0..100_000 {
                let handle = heap.allocate_str("temporary string");
                heap.set_root(frame, 0, handle);
            }
            let allocation_ns = start.elapsed().as_nanos();
            let start = std::time::Instant::now();
            heap.collect();
            let collection_ns = start.elapsed().as_nanos();
            let stats = heap.stats();
            println!(
                "fixed count=100000 stress={stress} alloc_root_auto_gc_ns={allocation_ns} collect_ns={collection_ns} stats={stats:?}"
            );
            assert_eq!(stats.peak_root_slots, 1);
            assert_eq!(stats.live_objects, 1);
            assert_eq!(stats.reclaimed, 99_999);
            assert!(stats.reserved_slots <= 257);
        }
    }
}
/// A época dos layouts de objeto do processo (J03): cresce a cada migração
/// que uma recarga do JIT define (`dartforge_definir_migracao`).
pub static LAYOUT_EPOCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

thread_local! {
    /// `DARTFORGE_EFEITOS=check` (`efeitos_conferir.rs`): as externs em
    /// execução nesta thread que a tabela de efeitos marca `collection = 0`. Com
    /// alguma, uma coleta é a marca errada — o defeito de um mapa de raízes
    /// errado, só que do nosso lado — e encerra o processo dizendo qual.
    static COLLECTION_FORBIDDEN: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Proíbe a coleta até o [`allow_collection`] casado: a extern `name` está
/// marcada `collection = 0`.
pub fn forbid_collection(name: String) {
    COLLECTION_FORBIDDEN.with(|p| p.borrow_mut().push(name));
}

/// Desfaz o último [`forbid_collection`].
pub fn allow_collection() {
    COLLECTION_FORBIDDEN.with(|p| {
        p.borrow_mut().pop();
    });
}

/// Encerra o processo se uma extern marcada `collection = 0` está em execução.
fn check_collection_allowed() {
    let culprit = COLLECTION_FORBIDDEN.with(|p| p.borrow().last().cloned());
    if let Some(name) = culprit {
        eprintln!("dartforge: a extern {name} está marcada coleta = 0 em efeitos.tsv e coletou");
        std::process::abort();
    }
}

#[cfg(test)]
mod unified_space {
    //! O coletor do espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md §2.3–§2.8,
    //! §4.2 passo 9): um teste por formato, cartões, anexos, regiões grandes,
    //! estáticos, a purga das tabelas de identidade e os bytes externos. Todos
    //! com a verificação da coleta menor ligada.
    use super::*;
    use crate::layout::{self, cid, flags};

    /// Um heap de teste com a verificação da coleta menor (a travessia completa
    /// que acusa barreira faltando).
    fn heap() -> Heap {
        let mut h = Heap::new(false);
        h.verify = true;
        h
    }

    /// Um `_List` geral (`REFS`) de `len` elementos nulos, com cartões se grande.
    fn list(heap: &mut Heap, len: usize) -> Ref {
        let mut f = flags::REFS;
        if layout::has_cards(len) {
            f |= flags::CARTOES;
        }
        let h = heap.allocate(cid::LIST, layout::list_words(len), f);
        heap.words_mut(h)[0] = len as i64;
        h
    }

    #[test]
    fn raw_keeps_bits_and_follows_nothing() {
        let mut heap = heap();
        let frame = heap.push_frame_with_slots(1);
        let target = heap.allocate_object(200, 1);
        let m = heap.allocate(cid::MINT, 2, flags::BRUTO);
        heap.set_root(frame, 0, m);
        // Bits que parecem um handle, num corpo BRUTO: o coletor não os segue.
        heap.words_mut(m)[0] = i64::MAX;
        heap.words_mut(m)[1] = target;
        heap.collect();
        assert!(!heap.is_live_object(target), "BRUTO seguido como referência");
        assert_eq!(heap.words(m)[0], i64::MAX);
        assert_eq!(heap.class(m), cid::MINT);
        assert_eq!(heap.header(m).n, 2);
        heap.pop_frame(frame);
    }

    #[test]
    fn refs_follows_elements_and_barrier_remembers() {
        let mut heap = heap();
        let frame = heap.push_frame_with_slots(1);
        let l = list(&mut heap, 3);
        heap.set_root(frame, 0, l);
        let a = heap.allocate_object(200, 0);
        heap.write_ref(l, 1, a);
        heap.write_ref(l, 3, smi::de(7).unwrap());
        heap.collect();
        assert!(heap.is_live_object(a), "elemento REFS não seguido");
        // A lista é velha: um jovem gravado nela sobrevive à coleta menor
        // pela barreira (sem ela, a verificação acusaria).
        let b = heap.allocate_object(201, 0);
        heap.write_ref(l, 2, b);
        assert_eq!(heap.header(l).estado, REMEMBERED);
        heap.coletar(true);
        assert!(heap.is_live_object(b), "jovem de lista velha coletado");
        assert_eq!(heap.words(l)[1..], [a, b, smi::de(7).unwrap()]);
        // Sem referência, some.
        heap.write_refs(l, 1, &[0, 0]);
        heap.collect();
        assert!(!heap.is_live_object(a) && !heap.is_live_object(b));
        heap.pop_frame(frame);
    }

    #[test]
    fn instance_follows_only_reference_fields() {
        let mut heap = heap();
        let frame = heap.push_frame_with_slots(1);
        let o = heap.allocate_instance(200, 40);
        heap.set_root(frame, 0, o);
        let child = heap.allocate_object(201, 0);
        let released = heap.allocate_object(202, 0);
        heap.set_field(o, 35, child, true);
        heap.set_field(o, 36, released, false);
        heap.collect();
        assert!(heap.is_live_object(child));
        assert!(!heap.is_live_object(released), "campo escalar seguido");
        assert_eq!(heap.words(o).len(), layout::palavras_de_instancia(40));
        heap.pop_frame(frame);
    }

    #[test]
    fn cards_keep_young_written_into_large_old_list() {
        let mut heap = heap();
        let frame = heap.push_frame_with_slots(1);
        let len = 100_000;
        let l = list(&mut heap, len);
        heap.set_root(frame, 0, l);
        assert!(heap.header(l).flags & flags::CARTOES != 0);
        heap.collect();
        // Velha: gravações velho → jovem espalhadas, cada uma sujando o cartão.
        let mut young_set = Vec::new();
        for i in [5, 2048, 40_000, len - 1] {
            let j = heap.allocate_object(200, 0);
            heap.write_ref(l, 1 + i, j);
            young_set.push((i, j));
        }
        heap.coletar(true);
        for &(i, j) in &young_set {
            assert!(heap.is_live_object(j), "jovem no elemento {i} coletado (cartão não percorrido)");
            assert_eq!(heap.words(l)[1 + i], j);
        }
        // A coleta menor zerou os cartões.
        let cards = &heap.words(l)[1 + len..];
        assert!(cards.iter().all(|&c| c == 0), "cartões não zerados");
        // A barreira explícita (gravação por fora) suja todos.
        let k = heap.allocate_object(201, 0);
        heap.words_mut(l)[1 + 77_777] = k;
        heap.remember(l);
        heap.coletar(true);
        assert!(heap.is_live_object(k));
        heap.pop_frame(frame);
    }

    static RELEASED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    #[allow(unsafe_code)]
    unsafe fn release_box(p: *mut u8) {
        // SAFETY: `p` veio de `Box::into_raw` no teste.
        drop(unsafe { Box::from_raw(p.cast::<[u8; 1000]>()) });
        RELEASED.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    #[test]
    #[allow(unsafe_code)]
    fn attachment_released_when_owner_dies() {
        let mut heap = heap();
        let before = RELEASED.load(std::sync::atomic::Ordering::SeqCst);
        let frame = heap.push_frame_with_slots(1);
        let a = heap.allocate(cid::STRING_ACCUMULATOR, 1, flags::BRUTO);
        let p = Box::into_raw(Box::new([0u8; 1000])).cast::<u8>();
        let bytes_before = heap.stats().estimated_bytes;
        // SAFETY: `release_box` é a única liberação de `p`.
        unsafe { heap.attach(a, release_box, p, 1000) };
        assert_eq!(heap.attachment(a), p);
        assert_eq!(heap.stats().estimated_bytes, bytes_before + 1000);
        // O anexo cresce (o acumulador do `StringBuffer`): conta nos gatilhos.
        heap.adjust_attachment(a, 3000);
        assert_eq!(heap.stats().estimated_bytes, bytes_before + 3000);
        assert_eq!(heap.objects.attachment_bytes, 3000);
        heap.set_root(frame, 0, a);
        heap.coletar(true);
        heap.collect();
        assert_eq!(RELEASED.load(std::sync::atomic::Ordering::SeqCst), before, "anexo solto com o dono vivo");
        heap.pop_frame(frame);
        heap.collect();
        assert_eq!(RELEASED.load(std::sync::atomic::Ordering::SeqCst), before + 1, "anexo do morto não solto");
        assert_eq!(heap.objects.attachment_bytes, 0);
    }

    #[test]
    fn large_region_returned() {
        let mut heap = heap();
        let pages = heap.objects.n_pages();
        let t = heap.allocate(cid::tipada(1), 5000, flags::BRUTO);
        assert!(heap.objects.block_of(t).is_some());
        assert_eq!(heap.words(t).len(), 5000, "o n saturado lido da região");
        assert_eq!(heap.objects.n_pages(), pages + 1);
        heap.collect();
        assert_eq!(heap.objects.n_pages(), pages, "região grande morta não saiu do espaço");
        assert!(heap.objects.cached_bytes() > 0, "região solta sem cache");
        // Reusada do cache pela alocação do mesmo tamanho.
        let u = heap.allocate(cid::tipada(1), 5000, flags::BRUTO);
        assert_eq!(heap.objects.cached_bytes(), 0);
        assert!(heap.words(u).iter().all(|&w| w == 0), "região reusada suja");
        heap.collect();
        heap.collect();
        heap.collect();
        assert_eq!(heap.objects.cached_bytes(), 0, "região parada não voltou ao sistema");
        // Uma classe média de 16 KiB não é região grande.
        let pages = heap.objects.n_pages();
        let m = heap.allocate(cid::tipada(1), layout::LARGEST_MEDIUM, flags::BRUTO);
        assert_eq!(heap.words(m).len(), layout::LARGEST_MEDIUM);
        assert!(heap.objects.n_pages() <= pages + 1);
    }

    /// Um estático na forma de §2.11 (em memória só de leitura do teste).
    static STATIC_STRING: [u64; 3] = [
        layout::palavra_do_cabecalho(layout::estado::PERMANENTE, flags::BRUTO, 2, cid::ONE_BYTE_STRING),
        0,
        3,
    ];

    #[test]
    fn static_never_marked_nor_swept() {
        let start = std::ptr::addr_of!(STATIC_STRING) as usize;
        register_image(start, start + std::mem::size_of_val(&STATIC_STRING));
        let s = start as i64 + layout::DESLOCAMENTO_DO_HANDLE;
        let v = Heap::box_bool(true);
        let mut heap = heap();
        heap.stress = true;
        assert!(heap.is_static(s) && heap.is_static(v) && heap.is_live_object(s));
        assert_eq!(heap.class(s), cid::ONE_BYTE_STRING);
        let frame = heap.push_frame_with_slots(2);
        let l = list(&mut heap, 2);
        heap.set_root(frame, 0, l);
        heap.set_root(frame, 1, s);
        heap.write_ref(l, 1, s);
        heap.write_ref(l, 2, v);
        let o = heap.allocate_object(200, 1);
        heap.set_field(o, 0, s, true);
        heap.set_global_root(1, o);
        heap.weak_refs.insert(o, s);
        for minor in [true, false, true, false] {
            heap.coletar(minor);
        }
        assert_eq!(heap.header(s).estado, layout::estado::PERMANENTE);
        assert_eq!(heap.words(s)[0], 3);
        assert_eq!(heap.weak_refs.get(&o), Some(&s), "referência fraca a estático zerada");
        assert!(heap.is_permanent(s));
        assert_eq!(heap.identity_hash(s), heap.identity_hash(s));
        heap.pop_frame(frame);
    }

    #[test]
    #[should_panic(expected = "gravação num objeto estático")]
    fn static_is_not_written() {
        let mut heap = heap();
        heap.words_mut(Heap::box_bool(false));
    }

    #[test]
    fn permanents_and_constants_purged() {
        let mut heap = heap();
        let o = heap.allocate_object(200, 0);
        heap.mark_permanent(o);
        heap.mark_constant(o, 0x1234);
        assert!(heap.is_permanent(o));
        heap.collect();
        assert!(heap.permanentes.is_empty() && heap.constants.is_empty(), "handle morto nas tabelas de identidade");
        // Vivo por um global, fica.
        let g = heap.allocate_object(200, 0);
        heap.mark_constant(g, 0x5678);
        heap.set_global_root(9, g);
        heap.collect();
        assert_eq!(heap.constant_getter(g), Some(0x5678));
    }

    #[test]
    fn discount_external_after_saturation_preserves_other_bytes() {
        for mode in [None, Some(false), Some(true)] {
            let mut heap = heap();
            if let Some(pure) = mode {
                heap.enable_arc();
                heap.arc.as_mut().unwrap().pure = pure;
            }
            let frame = heap.push_owning_frame(1);
            let object = list(&mut heap, 0);
            heap.set_root(frame, 0, object);
            let bytes_of_object = heap.stats.estimated_bytes;
            heap.count_external(17);
            for _ in 0..3 { heap.count_external(isize::MAX); }
            assert_eq!(heap.stats.estimated_bytes, usize::MAX);
            heap.collect();
            assert_eq!(heap.stats.estimated_bytes, usize::MAX);
            for _ in 0..3 { heap.count_external(-isize::MAX); }
            assert_eq!(heap.external, 17);
            assert_eq!(heap.stats.estimated_bytes, bytes_of_object + 17);
            heap.count_external(-17);
            assert_eq!(heap.stats.estimated_bytes, bytes_of_object);
            heap.pop_frame(frame);
            heap.collect();
            assert_eq!(heap.stats.estimated_bytes, 0);
        }
    }

    #[test]
    fn extreme_external_byte_debt_keeps_the_trigger() {
        let mut heap = heap();
        heap.schedule = None;
        heap.byte_threshold = usize::MAX;
        heap.byte_limit = usize::MAX;
        heap.count_external(isize::MAX);
        heap.count_external(isize::MAX);
        heap.count_external(1);
        assert!(heap.needs_collection(16), "a dívida não pode voltar a zero por overflow");
        heap.count_external(1);
        assert_eq!(heap.young_bytes, usize::MAX);
        assert!(heap.needs_collection(16));
    }

    #[test]
    fn external_bytes_fire_the_trigger() {
        let mut heap = heap();
        assert!(!heap.needs_collection(16));
        let too_many = heap.byte_threshold + 1;
        heap.count_external(too_many as isize);
        assert!(heap.needs_collection(16), "bytes externos fora do gatilho");
        heap.collect();
        assert!(heap.stats().estimated_bytes >= too_many, "a coleta completa perdeu os bytes externos");
        heap.count_external(-(too_many as isize));
        assert!(heap.stats().estimated_bytes < too_many);
    }

    #[test]
    fn set_flags_changes_only_the_shape() {
        let mut heap = heap();
        let l = heap.allocate(cid::LIST, 3, flags::BRUTO | flags::ELEMENTO_INT);
        heap.set_flags(l, flags::FORMA | flags::ELEMENTO, flags::REFS);
        assert_eq!(heap.header(l).flags, flags::REFS);
    }

    #[test]
    fn medium_and_exact_classes_coexist() {
        let mut heap = heap();
        let frame = heap.push_frame_with_slots(1);
        let l = list(&mut heap, 200);
        heap.set_root(frame, 0, l);
        let mut hs = Vec::new();
        for w in [1, 7, 64, 65, 70, 71, 300, 1000, 2014] {
            let h = heap.allocate(cid::tipada(1), w, flags::BRUTO);
            heap.words_mut(h)[w - 1] = w as i64;
            hs.push((w, h));
        }
        for (i, &(_, h)) in hs.iter().enumerate() {
            heap.write_ref(l, 1 + i, h);
        }
        heap.coletar(true);
        heap.collect();
        for &(w, h) in &hs {
            assert_eq!(heap.words(h).len(), w);
            assert_eq!(heap.words(h)[w - 1], w as i64, "corpo de {w} palavras perdido");
        }
        heap.pop_frame(frame);
    }
}

/// O estado do ARC no heap (docs/ARC-IMPLEMENTACAO.md, sobre
/// docs/ARC-CICLOS-ESPECIFICACAO.md §19 e §22): o RC das ocorrências fortes
/// entre objetos velhos e nos slots proprietários. Raízes observacionais são
/// vistas na sincronização, sem contar RC; jovens usam coleta menor até promoção.
pub struct HeapArc {
    pub estado: crate::arc::EstadoDoArc,
    /// Os velhos entregues crus nesta época ([`Heap::words_mut`],
    /// [`Heap::object_fields`], [`Heap::set_flags`]): as ocorrências
    /// contadas que tinham antes. A sincronização conta as de agora e solta
    /// estas; até lá, as gravações contadas na hora pulam o objeto.
    snapshots: crate::hash::HashMap<Ref, Vec<Ref>>,
    ephemerons: ArcEphemerons,
    /// `DARTFORGE_ARC_CONFERIR=1`: depois de cada sincronização, nenhum
    /// alcançável morto e o RC de cada vivo igual às ocorrências recontadas.
    check: bool,
    /// `DARTFORGE_ARC_CICLOS=sempre`: a rodada de ciclos (*trial
    /// deletion*) em toda sincronização; `nunca`: nem ela nem o ponto fixo
    /// da completa (o diagnóstico); o padrão: o ponto fixo na completa.
    cycles: Option<bool>,
    /// O ARC puro (o padrão; docs/ARC-IMPLEMENTACAO.md §6): toda gravação
    /// de referência conta na hora, também num jovem, e a drenagem
    /// ([`Heap::drain_arc`]) decide a vida sem percorrer o heap. Com
    /// `DARTFORGE_ARC_BERCARIO=1`, o modo da primeira etapa: o berçário pela
    /// coleta menor rastreada e as arestas do jovem contadas na promoção.
    pure: bool,
}

impl std::fmt::Debug for HeapArc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArcDoHeap")
            .field("registrados", &self.estado.registered_set())
            .field("estatisticas", &self.estado.statistics)
            .field("fotos", &self.snapshots.len())
            .field("efemeros", &self.ephemerons.counted.len())
            .finish()
    }
}

/// As entradas de efêmero contadas: cada uma é uma ocorrência do valor,
/// atribuída à chave no grafo (o valor vive enquanto a chave e o portador
/// vivem; a morte do portador solta a entrada na sincronização).
#[derive(Default)]
struct ArcEphemerons {
    counted: crate::hash::HashMap<Ref, (Ref, Ref)>,
    by_key: crate::hash::HashMap<Ref, Vec<Ref>>,
}

impl ArcEphemerons {
    fn insert_entry(&mut self, holder: Ref, key: Ref, value: Ref) {
        self.counted.insert(holder, (key, value));
        self.by_key.entry(key).or_default().push(holder);
    }
    fn remove_it(&mut self, holder: Ref) -> Option<(Ref, Ref)> {
        let (key, value) = self.counted.remove(&holder)?;
        if let Some(l) = self.by_key.get_mut(&key) {
            l.retain(|&p| p != holder);
            if l.is_empty() {
                self.by_key.remove(&key);
            }
        }
        Some((key, value))
    }
}

/// O grafo do ARC sobre o heap: as posições que a marcação percorre e as
/// entradas de efêmero contadas (docs/ARC-CICLOS-ESPECIFICACAO.md §19.2).
struct HeapGraph<'a> {
    heap: &'a mut Heap,
    ephemerons: &'a mut ArcEphemerons,
}

impl crate::arc::ArcGraph for HeapGraph<'_> {
    fn edges(&self, h: Ref, f: &mut dyn FnMut(Ref)) {
        // SAFETY: as posições são palavras do corpo de um bloco vivo.
        #[allow(unsafe_code)]
        self.heap.visit_strong_references(h, f);
        // Sem efêmero contado (o caso comum), nada de hash por objeto.
        if self.ephemerons.by_key.is_empty() {
            return;
        }
        if let Some(ps) = self.ephemerons.by_key.get(&h) {
            for p in ps {
                if let Some(&(k, v)) = self.ephemerons.counted.get(p)
                    && k == h
                {
                    f(v);
                }
            }
        }
    }
    fn take_edges(&mut self, h: Ref, f: &mut dyn FnMut(Ref)) {
        if !self.heap.attachments_by_owner.is_empty() && self.heap.attachments_by_owner.contains_key(&h) {
            self.heap.attachments.retain(|a| {
                if a.owner == h && let FinalizerAction::Dart(action) = a.action {
                    f(action);
                    return false;
                }
                true
            });
            self.heap.reindex_attachments();
        }
        // SAFETY: as posições são palavras do corpo de um bloco vivo.
        #[allow(unsafe_code)]
        self.heap.ref_positions(h, &mut |p| unsafe {
            f(*p);
            *p = 0;
        });
        if !self.ephemerons.by_key.is_empty() {
            // As arestas condicionais (a chave segura o valor) e o
            // rompimento delas, como em `edges` e `break_off`.
            let mut value_list = Vec::new();
            if let Some(ps) = self.ephemerons.by_key.get(&h) {
                for p in ps {
                    if let Some(&(k, v)) = self.ephemerons.counted.get(p)
                        && k == h
                    {
                        value_list.push(v);
                    }
                }
            }
            for v in value_list {
                f(v);
            }
            if let Some(ps) = self.ephemerons.by_key.remove(&h) {
                for p in ps {
                    self.ephemerons.counted.remove(&p);
                    if let Some(e) = self.heap.ephemerons.get_mut(&p)
                        && e.0 == h
                    {
                        *e = (0, 0);
                    }
                }
            }
        }
    }
    fn break_off(&mut self, h: Ref) {
        if !self.heap.attachments_by_owner.is_empty() && self.heap.attachments_by_owner.contains_key(&h) {
            self.heap.attachments.retain(|a| a.owner != h || !matches!(a.action, FinalizerAction::Dart(_)));
            self.heap.reindex_attachments();
        }
        // SAFETY: as posições são palavras do corpo de um bloco vivo.
        #[allow(unsafe_code)]
        self.heap.ref_positions(h, &mut |p| unsafe { *p = 0 });
        if self.ephemerons.by_key.is_empty() {
            return;
        }
        if let Some(ps) = self.ephemerons.by_key.remove(&h) {
            for p in ps {
                self.ephemerons.counted.remove(&p);
                if let Some(e) = self.heap.ephemerons.get_mut(&p)
                    && e.0 == h
                {
                    *e = (0, 0);
                }
            }
        }
    }
}

/// O objeto `h` tem corpo `BRUTO` (nenhuma posição de referência: string,
/// caixa numérica, dados tipados)? O filtro de candidatos a ciclo do ARC
/// (`EstadoDoArc::without_edges`).
fn block_without_refs(h: Ref) -> bool {
    // SAFETY: o ARC só chama com handles registrados, blocos vivos do espaço
    // (a imagem não tem metadados).
    #[allow(unsafe_code)]
    unsafe {
        (*((h - DESLOCAMENTO_DO_HANDLE) as *const Header)).flags & flags_do_bloco::FORMA == flags_do_bloco::BRUTO
    }
}

/// Uma violação do contador é bug do compilador ou do runtime, nunca erro
/// Dart (docs/ARC-CICLOS-ESPECIFICACAO.md §19.3).
#[cold]
fn arc_failure(e: crate::arc::ArcError) -> ! {
    panic!("bug do ARC: {e:?}")
}

/// A geometria da página do bloco `h` para a tabela de metadados do ARC
/// (`arc::Geometry`); `None` fora do espaço (o metadado vai para a reserva).
fn arc_geometry(espaco: &ObjectSpace, h: Ref) -> Option<crate::arc::Geometry> {
    espaco.geometry(h).map(|(first, size, blocks)| crate::arc::Geometry { first, size, blocks })
}

/// ARC puro, no laço da drenagem (depois do passo 4, em que todo jovem que
/// sobrevive é registrado): o objeto do espaço sem metadados morreu nesta
/// drenagem — o jovem que ninguém contou, ou o morto pelo RC, cujos
/// metadados `take_dead` já tirou. O estático (`PERMANENTE`) não é do
/// espaço e nunca morre.
#[allow(unsafe_code)]
fn died_in_drain(espaco: &ObjectSpace, validate: bool, estado: &crate::arc::EstadoDoArc, h: Ref) -> bool {
    if !is_object(h) {
        return false;
    }
    let in_space = if validate {
        espaco.block_of(h).is_some()
    } else {
        // SAFETY: um handle de objeto é um bloco ou um estático (o contrato
        // de `space_block` sem a validação).
        unsafe { (*((h - DESLOCAMENTO_DO_HANDLE) as *const Header)).estado != PERMANENTE }
    };
    in_space && estado.meta(h).is_none()
}

#[allow(unsafe_code)]
impl Heap {
    /// O ARC está ligado.
    pub fn arc_active(&self) -> bool {
        self.arc.is_some()
    }

    /// Os contadores do ARC, se ligado.
    pub fn arc_statistics(&self) -> Option<crate::arc::ArcStats> {
        self.arc.as_ref().map(|a| a.estado.statistics)
    }

    /// A troca de `old` por `new` numa posição forte do bloco `b` (de
    /// `h`), contada na hora se o dono não foi fotografado. No ARC puro
    /// conta sempre, e o destino ainda sem metadados é registrado (o jovem
    /// que ganha a primeira ocorrência). No modo do berçário só conta com o
    /// dono velho (as arestas do jovem entram na promoção).
    fn arc_swapped(&mut self, b: *mut Header, h: Ref, old: Ref, new: Ref) {
        let new_in_space = self.space_block(new).is_some();
        let Some(arc) = self.arc.as_deref_mut() else { return };
        // SAFETY: bloco vivo do espaço.
        let e = unsafe { (*b).estado };
        if old == new || arc.snapshots.contains_key(&h) || (!arc.pure && e != VELHO && e != REMEMBERED) {
            return;
        }
        if !arc.pure {
            arc.estado.retain_ref(new).unwrap_or_else(|e| arc_failure(e));
        } else if !arc.estado.retain_if_registered(new).unwrap_or_else(|e| arc_failure(e)) && new_in_space {
            arc.estado.register_live_at(new, arc_geometry(&self.objects, new));
            arc.estado.retain_ref(new).unwrap_or_else(|e| arc_failure(e));
        }
        arc.estado.release(old).unwrap_or_else(|e| arc_failure(e));
    }

    /// O objeto do espaço `h` ainda sem metadados ganha registro (RC zero,
    /// vivo) antes da primeira ocorrência contada.
    fn arc_register_if_needed(espaco: &ObjectSpace, arc: &mut HeapArc, h: Ref) {
        if smi::e_handle(h) && arc.estado.meta(h).is_none() {
            arc.estado.register_live_at(h, arc_geometry(espaco, h));
        }
    }

    /// Retém `v` (registrando o objeto do espaço sem metadados): uma
    /// ocorrência nova, no ARC puro.
    fn arc_retain_registering(&self, arc: &mut HeapArc, v: Ref) {
        if arc.estado.retain_if_registered(v).unwrap_or_else(|e| arc_failure(e)) {
            return;
        }
        if self.space_block(v).is_some() {
            arc.estado.register_live_at(v, arc_geometry(&self.objects, v));
            arc.estado.retain_ref(v).unwrap_or_else(|e| arc_failure(e));
        }
    }

    /// As referências que o código gerado gravou em linha num objeto recém
    /// alocado (`AllocObject`, contexto, célula e closure em linha,
    /// `dartforge_arc_inicial`): no ARC puro, cada uma conta. No modo do
    /// berçário não faz nada (contam na promoção).
    pub fn arc_count_initial(&mut self, h: Ref) {
        if !smi::e_handle(h) || self.space_block(h).is_none() {
            return;
        }
        let Some(mut arc) = self.arc.take() else { return };
        if arc.pure {
            // A retenção só altera metadados de ARC: não coleta nem muda o
            // corpo visitado. Contar diretamente evita um vetor por objeto
            // e conserva cada ocorrência, inclusive referências repetidas.
            self.visit_strong_references(h, &mut |v| self.arc_retain_registering(&mut arc, v));
        }
        self.arc = Some(arc);
    }

    /// A gravação crua de um native que não passa pelo `Heap` (o `_data` do
    /// `Map` em `nativos_hash.rs`, a pilha do JSON): a troca de `old` por
    /// `new` numa posição forte de `h`.
    pub fn arc_raw_write(&mut self, h: Ref, old: Ref, new: Ref) {
        if let Some(b) = self.space_block(h) {
            self.arc_swapped(b, h, old, new);
        }
    }

    /// A foto das ocorrências contadas de `h` antes de uma escrita crua
    /// (uma por época): do velho no modo do berçário, de todo objeto no ARC
    /// puro (o jovem também conta as gravações).
    fn arc_snapshot(&mut self, b: *mut Header, h: Ref) {
        // SAFETY: bloco vivo do espaço.
        let e = unsafe { (*b).estado };
        if e != VELHO && e != REMEMBERED && !self.arc.as_ref().is_some_and(|a| a.pure) {
            return;
        }
        if self.arc.as_ref().is_some_and(|a| a.snapshots.contains_key(&h)) {
            return;
        }
        let mut value_list = Vec::new();
        // SAFETY: as posições são palavras do corpo de um bloco vivo.
        self.visit_strong_references(h, &mut |v| value_list.push(v));
        let arc = self.arc.as_deref_mut().expect("ARC ligado");
        value_list.retain(|&v| arc.estado.meta(v).is_some());
        arc.snapshots.insert(h, value_list);
    }

    /// Os jovens promovidos pela marcação menor em curso e os velhos
    /// lembrados, como handles (antes da varredura dos jovens).
    fn promotion_data(&self) -> (Vec<Ref>, Vec<Ref>) {
        let mut promoted = Vec::new();
        self.objects.young_marked(&mut |b| promoted.push(b as i64 + DESLOCAMENTO_DO_HANDLE));
        let remembered = self.objects.remembered.iter().map(|&b| b as i64 + DESLOCAMENTO_DO_HANDLE).collect();
        (promoted, remembered)
    }

    /// Liga o ARC (`dartforge_memoria_arc_v1`, na entrada do programa): uma
    /// coleta completa por rastreamento e o registro de todo vivo com o RC
    /// das ocorrências que chegam a ele, incluindo slots proprietários.
    /// Raízes observacionais apenas protegem os objetos que veem.
    pub fn enable_arc(&mut self) {
        if self.arc.is_some() {
            return;
        }
        self.coletar(false);
        let mut arc = Box::new(HeapArc {
            estado: crate::arc::EstadoDoArc::new(),
            snapshots: crate::hash::HashMap::default(),
            ephemerons: ArcEphemerons::default(),
            check: std::env::var("DARTFORGE_ARC_CONFERIR").is_ok_and(|v| v == "1"),
            cycles: match std::env::var("DARTFORGE_ARC_CICLOS").as_deref() {
                Ok("sempre") => Some(true),
                Ok("nunca") => Some(false),
                _ => None,
            },
            pure: !std::env::var("DARTFORGE_ARC_BERCARIO").is_ok_and(|v| v == "1"),
        });
        arc.estado.trace_log = std::env::var("DARTFORGE_ARC_TRACO").is_ok_and(|v| v == "1");
        arc.estado.without_edges = Some(block_without_refs);
        let reached_set = self.reachable_by_tracing();
        for &x in &reached_set {
            arc.estado.register_live_at(x, arc_geometry(&self.objects, x));
        }
        let mut value_list = Vec::new();
        for &x in &reached_set {
            // SAFETY: as posições são palavras do corpo de um bloco vivo.
            self.visit_strong_references(x, &mut |v| value_list.push(v));
            for v in value_list.drain(..) {
                arc.estado.retain_ref(v).unwrap_or_else(|e| arc_failure(e));
            }
        }
        for (&p, &(k, v)) in &self.ephemerons {
            if arc.estado.alive(p) && smi::e_handle(k) && smi::e_handle(v) {
                arc.estado.retain_ref(v).unwrap_or_else(|e| arc_failure(e));
                arc.ephemerons.insert_entry(p, k, v);
            }
        }
        for h in self.owning_roots() {
            self.arc_retain_registering(&mut arc, h);
        }
        self.mark_side_edges(&mut arc);
        for &x in &reached_set {
            arc.estado.review(x);
        }
        self.arc = Some(arc);
    }

    /// Restaura indicações laterais após ativação ou reconstrução de metadados.
    fn mark_side_edges(&self, arc: &mut HeapArc) {
        for &owner in self.attachments_by_owner.keys() {
            arc.estado.mark_side_edges(owner);
        }
        for &(key, _) in arc.ephemerons.counted.values() {
            arc.estado.mark_side_edges(key);
        }
    }

    /// Os objetos do espaço alcançáveis das raízes, com a regra dos efêmeros
    /// (o valor quando o portador e a chave são alcançados), sem marcas.
    fn reachable_by_tracing(&self) -> Vec<Ref> {
        let mut stack = Vec::new();
        self.root_list(&mut stack);
        let mut seen: crate::hash::HashSet<Ref> = crate::hash::HashSet::default();
        let mut out = Vec::new();
        loop {
            while let Some(h) = stack.pop() {
                if !smi::e_handle(h) || !is_object(h) || self.space_block(h).is_none() || !seen.insert(h) {
                    continue;
                }
                out.push(h);
                // SAFETY: as posições são palavras do corpo de um bloco vivo.
                self.visit_strong_references(h, &mut |v| stack.push(v));
            }
            let reached = |h: &Ref| !smi::e_handle(*h) || seen.contains(h) || (is_object(*h) && self.space_block(*h).is_none());
            for (p, (k, v)) in &self.ephemerons {
                if reached(p) && reached(k) && smi::e_handle(*v) && !seen.contains(v) {
                    stack.push(*v);
                }
            }
            if stack.is_empty() {
                break;
            }
        }
        out
    }

    /// O ponto fixo e a transação de descarte global da completa (§22.3):
    /// `L` é o alcance pelas raízes reais pelas arestas fortes, com a regra
    /// condicional dos efêmeros (o valor entra quando o portador e a chave
    /// estão em `L`); todo registrado vivo fora de `L` morre num lote só,
    /// inclusive o de RC positivo só por ciclos.
    ///
    /// Entre as completas a entrada de efêmero é uma ocorrência do valor
    /// atribuída à chave: segura o valor enquanto a chave vive, mesmo com o
    /// portador já inalcançável (retenção, nunca morte antes da hora). Aqui o
    /// portador conta: a entrada de portador fora de `L` sai da contagem
    /// antes do lote, e o valor que sobrevive perde a ocorrência. A de chave
    /// fora de `L` sai pelo próprio descarte (o rompimento da chave zera a
    /// entrada e solta o valor).
    fn arc_fixpoint(&mut self, arc: &mut HeapArc) {
        let l: crate::hash::HashSet<Ref> = self.reachable_by_tracing().into_iter().collect();
        let d: Vec<Ref> = arc
            .estado
            .live_set()
            .filter(|h| !l.contains(h) && arc.estado.meta(*h).is_some_and(|m| m.estado == crate::arc::ArcState::Live && !m.immortal))
            .collect();
        if arc.estado.trace_log {
            eprintln!("[arc-traco] ponto fixo: {} alcançados, {} mortos", l.len(), d.len());
        }
        if d.is_empty() {
            return;
        }
        let in_d: crate::hash::HashSet<Ref> = d.iter().copied().collect();
        let leaving: Vec<Ref> = arc
            .ephemerons
            .counted
            .iter()
            .filter(|&(p, &(k, _))| in_d.contains(p) && !in_d.contains(&k))
            .map(|(&p, _)| p)
            .collect();
        for p in leaving {
            if let Some((_, v)) = arc.ephemerons.remove_it(p)
                && !in_d.contains(&v)
            {
                arc.estado.release(v).unwrap_or_else(|e| arc_failure(e));
            }
        }
        let n = d.len();
        let HeapArc { estado, ephemerons, .. } = arc;
        let mut graph = HeapGraph { heap: self, ephemerons };
        estado.discard(&mut graph, &d).unwrap_or_else(|e| arc_failure(e));
        estado.statistics.dead_by_cycle += n as u64;
    }

    /// A sincronização do ARC, no fim da coleta menor (§19.3, §19.4, §22):
    ///
    /// 1. os promovidos ganham metadados (`rc=0`);
    /// 2. os incrementos: as ocorrências que saem dos promovidos, as que os
    ///    velhos lembrados têm para promovidos, as atuais dos fotografados e
    ///    as entradas de efêmero novas;
    /// 3. os decrementos: as fotos e as entradas de efêmero trocadas;
    /// 4. os zeros morrem em cascata, menos os vistos pelas raízes (que
    ///    ficam na fila); na completa, o ponto fixo global
    ///    ([`Heap::arc_fixpoint`]); com `DARTFORGE_ARC_CICLOS=sempre`, a
    ///    rodada de ciclos em toda sincronização;
    /// 5. as tabelas laterais esquecem os mortos (fracas, efêmeros,
    ///    finalizadores) até não haver morte nova;
    /// 6. na completa, a reclamação: as marcas são os vivos do RC e a
    ///    varredura completa devolve os blocos dos mortos.
    fn sync_arc(&mut self, full: bool, promoted: Vec<Ref>, remembered: Vec<Ref>) {
        let mut arc = self.arc.take().expect("ARC ligado");
        if arc.estado.trace_log {
            eprintln!("[arc-traco] sincronizar completa={full} promovidos={promoted:?} lembrados={remembered:?} fotos={:?}", arc.snapshots.keys().collect::<Vec<_>>());
        }
        let new_ones: crate::hash::HashSet<Ref> = promoted.iter().copied().collect();
        for &h in &promoted {
            arc.estado.register_live_at(h, arc_geometry(&self.objects, h));
        }
        // A promoção reconstruiu RC; repõe cada ocorrência proprietária,
        // sem reter novamente os owners dos objetos que já eram velhos.
        for h in self.owning_roots() {
            if new_ones.contains(&h) { self.arc_retain_registering(&mut arc, h); }
        }
        let mut value_list = Vec::new();
        let mut release: Vec<Ref> = Vec::new();
        for &h in &promoted {
            // SAFETY: as posições são palavras do corpo de um bloco vivo.
            self.visit_strong_references(h, &mut |v| value_list.push(v));
            for v in value_list.drain(..) {
                arc.estado.retain_ref(v).unwrap_or_else(|e| arc_failure(e));
            }
        }
        for &x in &remembered {
            if new_ones.contains(&x) || arc.snapshots.contains_key(&x) || !arc.estado.alive(x) {
                continue;
            }
            // SAFETY: as posições são palavras do corpo de um bloco vivo.
            self.visit_strong_references(x, &mut |v| value_list.push(v));
            for v in value_list.drain(..) {
                if new_ones.contains(&v) {
                    arc.estado.retain_ref(v).unwrap_or_else(|e| arc_failure(e));
                }
            }
        }
        for (x, snapshot) in std::mem::take(&mut arc.snapshots) {
            if arc.estado.alive(x) {
                // SAFETY: as posições são palavras do corpo de um bloco vivo.
                self.visit_strong_references(x, &mut |v| value_list.push(v));
                for v in value_list.drain(..) {
                    arc.estado.retain_ref(v).unwrap_or_else(|e| arc_failure(e));
                }
            }
            release.extend(snapshot);
        }
        // Os efêmeros: as entradas vivas de agora contra as contadas.
        let mut currents: crate::hash::HashSet<Ref> = crate::hash::HashSet::default();
        for (&p, &(k, v)) in &self.ephemerons {
            if !(arc.estado.alive(p) && smi::e_handle(k) && smi::e_handle(v)) {
                continue;
            }
            currents.insert(p);
            match arc.ephemerons.counted.get(&p) {
                Some(&(k0, v0)) if k0 == k && v0 == v => {}
                before => {
                    if before.is_some()
                        && let Some((_, v0)) = arc.ephemerons.remove_it(p)
                    {
                        release.push(v0);
                    }
                    arc.estado.retain_ref(v).unwrap_or_else(|e| arc_failure(e));
                    arc.ephemerons.insert_entry(p, k, v);
                }
            }
        }
        let exited: Vec<Ref> = arc.ephemerons.counted.keys().copied().filter(|p| !currents.contains(p)).collect();
        for p in exited {
            if let Some((_, v0)) = arc.ephemerons.remove_it(p) {
                release.push(v0);
            }
        }
        for v in release {
            arc.estado.release(v).unwrap_or_else(|e| arc_failure(e));
        }
        self.mark_side_edges(&mut arc);
        for &h in &promoted {
            arc.estado.review(h);
        }
        // Todas as raízes protegem os zeros que veem; os slots proprietários
        // já tiveram suas ocorrências incluídas no RC.
        let mut root_list = Vec::new();
        self.root_list(&mut root_list);
        let root_list: crate::hash::HashSet<Ref> = root_list.into_iter().filter(|&h| smi::e_handle(h)).collect();
        let protected = |h: Ref| root_list.contains(&h);
        for &h in &root_list {
            arc.estado.add_candidate(h);
        }
        // Com `DARTFORGE_ARC_CICLOS=sempre`, a rodada de ciclos se repete a
        // cada volta com morte nova: o descarte (e o efêmero que sai com o
        // portador) solta sobreviventes que podem ter ficado num ciclo sem
        // raiz. Na completa, o ponto fixo decide a vida de todos (§22.3).
        let cycles = arc.cycles == Some(true);
        let mut fixpoint = full && arc.cycles != Some(false);
        let mut ready = Vec::new();
        let mut attachment_edges = Vec::new();
        let mut natives = Vec::new();
        let mut finalize = Vec::new();
        arc.estado.resume_deferred();
        loop {
            {
                let HeapArc { estado, ephemerons, .. } = &mut *arc;
                let mut graph = HeapGraph { heap: self, ephemerons };
                estado.drain_zeros(&mut graph, usize::MAX, &protected).unwrap_or_else(|e| arc_failure(e));
                if cycles {
                    estado.collect_cycles(&mut graph, &protected).unwrap_or_else(|e| arc_failure(e));
                    estado.drain_zeros(&mut graph, usize::MAX, &protected).unwrap_or_else(|e| arc_failure(e));
                }
            }
            if fixpoint {
                fixpoint = false;
                self.arc_fixpoint(&mut arc);
                let HeapArc { estado, ephemerons, .. } = &mut *arc;
                let mut graph = HeapGraph { heap: self, ephemerons };
                estado.drain_zeros(&mut graph, usize::MAX, &protected).unwrap_or_else(|e| arc_failure(e));
            }
            let dead_set = arc.estado.take_dead();
            if dead_set.is_empty() {
                break;
            }
            let dead: crate::hash::HashSet<Ref> = dead_set.into_iter().collect();
            self.weak_refs.retain(|holder, target| {
                if dead.contains(target) {
                    *target = 0;
                }
                !dead.contains(holder)
            });
            let holders: Vec<Ref> = self.ephemerons.keys().copied().filter(|p| dead.contains(p)).collect();
            for p in holders {
                self.ephemerons.remove(&p);
                if let Some((_, v)) = arc.ephemerons.remove_it(p) {
                    arc.estado.release(v).unwrap_or_else(|e| arc_failure(e));
                }
            }
            self.attachments.retain_mut(|a| {
                if matches!(a.action, FinalizerAction::Native(_, _, _)) && dead.contains(&a.owner) {
                    a.owner = 0;
                }
                if dead.contains(&a.detach) {
                    a.detach = 0;
                }
                if !dead.contains(&a.value) {
                    return true;
                }
                match a.action {
                    FinalizerAction::Dart(action) => {
                        ready.push(action);
                        attachment_edges.push((a.owner, action));
                    },
                    FinalizerAction::Native(f, token, bytes) => natives.push((f, token, bytes)),
                }
                false
            });
            Self::reindex_attachment_list(&self.attachments, &mut self.attachments_by_owner);
            self.late_fields_initialized.retain(|(h, _)| !dead.contains(h));
            self.new_lates.retain(|(h, _)| !dead.contains(h));
            self.permanentes.retain(|h| !dead.contains(h));
            self.constants.retain(|h, _| !dead.contains(h));
            self.tearoff_code.retain(|h, _| !dead.contains(h));
            self.finalizable.retain(|h, &mut pair| {
                let stays = !dead.contains(h);
                if !stays {
                    finalize.push(pair);
                }
                stays
            });
        }
        self.reindex_attachments();
        for &action in &ready { self.arc_retain_registering(&mut arc, action); }
        for (_, action) in attachment_edges {
            arc.estado.release(action).unwrap_or_else(|e| arc_failure(e));
        }
        self.ready_finalizations.extend(ready);
        if full {
            self.reclaim_arc(&arc);
        } else {
            self.minors_since_full += 1;
        }
        if arc.check {
            self.check_arc(&mut arc);
        }
        if self.trace {
            let e = arc.estado.statistics;
            eprintln!(
                "[arc] {} promovidos={} contados={} retains={} releases={} mortos_rc={} mortos_ciclo={} rodadas={} examinados={} candidatos={}",
                if full { "completa" } else { "menor" },
                promoted.len(),
                arc.estado.registered_set(),
                e.retains,
                e.releases,
                e.dead_by_rc,
                e.dead_by_cycle,
                e.cycle_rounds,
                e.examined,
                arc.estado.candidates()
            );
        }
        self.arc = Some(arc);
        for (finalizer, pair) in finalize {
            finalizer(pair);
        }
        for (f, token, bytes) in natives {
            self.count_external(-(bytes as isize));
            // SAFETY: a `NativeFinalizerFunction` do anexo, `void f(void*)`;
            // como na VM, roda durante a coleta e não pode tocar o heap.
            let f: extern "C" fn(usize) = unsafe { std::mem::transmute(f) };
            f(token);
        }
    }

    /// A drenagem do ARC puro (docs/ARC-IMPLEMENTACAO.md §6), no lugar da
    /// coleta menor, sem percorrer o heap:
    ///
    /// 1. as fotos (escritas cruas desde a última drenagem) contam as
    ///    ocorrências de agora e soltam as da foto (a vida do fotografado
    ///    segue o RC e as raízes, como a de qualquer objeto);
    /// 2. o jovem (entregue desde a última drenagem) que só uma raiz direta
    ///    vê ganha registro (RC zero, protegido);
    /// 3. as entradas de efêmero vivas contra as contadas (o valor é uma
    ///    ocorrência atribuída à chave);
    /// 4. o jovem registrado (por uma ocorrência contada ou pelo passo 2)
    ///    fica (marcado, velho, candidato a ciclo); o resto morreu sem
    ///    ninguém o ver: solta as ocorrências que contou;
    /// 5. os zeros morrem em cascata, menos os que as raízes veem; na
    ///    completa, o ponto fixo (com efêmeros) ou a rodada de ciclos até não
    ///    haver morte nova; as tabelas laterais esquecem os mortos;
    /// 6. a memória: os jovens mortos pela varredura dos jovens (sem marca),
    ///    os mortos pelo RC um a um.
    ///
    /// A pilha-sombra (ou os mapas), quadros observacionais e tabelas ainda
    /// não migradas protegem o que veem diretamente, sem contar RC.
    /// Slots proprietários, globais, exceção/rastro e arestas fortes de objetos
    /// contam ocorrências.
    fn drain_arc(&mut self, full: bool) {
        check_collection_allowed();
        let trace = self.trace;
        let start = trace.then(std::time::Instant::now);
        // O tempo de cada etapa, para o rastro (raízes, fotos, jovens das
        // raízes, efêmeros, decisão dos jovens, soltura dos jovens mortos,
        // raízes candidatas, zeros/ciclos, memória).
        // Sem rastro, não reservar métricas nem consultar o relógio.
        let mut phases: Vec<u128> = if trace { Vec::with_capacity(10) } else { Vec::new() };
        if let Some(a) = &self.schedule {
            a.pending.set(false);
        }
        self.return_tlabs();
        self.stats.collections += 1;
        let mut arc = self.arc.take().expect("ARC ligado");
        let mut root_list = Vec::new();
        self.root_list(&mut root_list);
        let protected_set: crate::hash::HashSet<Ref> = root_list.into_iter().filter(|&h| smi::e_handle(h)).collect();
        if let Some(start) = start {
            phases.push(start.elapsed().as_micros());
        }
        // 1. As fotos.
        let mut value_list = Vec::new();
        let n_snapshots = arc.snapshots.len();
        for (x, snapshot) in std::mem::take(&mut arc.snapshots) {
            if self.space_block(x).is_some() && !arc.estado.dead(x) {
                // SAFETY: as posições são palavras do corpo de um bloco vivo.
                self.visit_strong_references(x, &mut |v| value_list.push(v));
                for v in value_list.drain(..) {
                    self.arc_retain_registering(&mut arc, v);
                }
                // Registrado, o fotografado segue o RC: sem ocorrência nem
                // raiz, morre adiante e solta as que acabou de contar.
                Self::arc_register_if_needed(&self.objects, &mut arc, x);
            }
            for v in snapshot {
                arc.estado.release(v).unwrap_or_else(|e| arc_failure(e));
            }
        }
        if let Some(start) = start {
            phases.push(start.elapsed().as_micros());
        }
        // 2. Os jovens que só as raízes veem.
        let mut young_set = Vec::new();
        self.objects.young_handed_out(&mut |b| young_set.push(b));
        // Só as raízes podem acrescentar registros nesta etapa. Consultar
        // todos os jovens aqui custava uma busca de metadados por alocação,
        // embora a decisão de cada jovem já ocorra no passo 4.
        for &h in &protected_set {
            if self.young_child(h) && arc.estado.meta(h).is_none() {
                arc.estado.register_live_at(h, arc_geometry(&self.objects, h));
            }
        }
        if let Some(start) = start {
            phases.push(start.elapsed().as_micros());
        }
        // 3. Os efêmeros: as entradas vivas de agora contra as contadas.
        let mut release: Vec<Ref> = Vec::new();
        let mut currents: crate::hash::HashSet<Ref> = crate::hash::HashSet::default();
        let entries: Vec<(Ref, Ref, Ref)> = self.ephemerons.iter().map(|(&p, &(k, v))| (p, k, v)).collect();
        for (p, k, v) in entries {
            if !(arc.estado.alive(p) && smi::e_handle(k) && smi::e_handle(v)) {
                continue;
            }
            currents.insert(p);
            match arc.ephemerons.counted.get(&p) {
                Some(&(k0, v0)) if k0 == k && v0 == v => {}
                before => {
                    if before.is_some()
                        && let Some((_, v0)) = arc.ephemerons.remove_it(p)
                    {
                        release.push(v0);
                    }
                    self.arc_retain_registering(&mut arc, v);
                    arc.ephemerons.insert_entry(p, k, v);
                }
            }
        }
        let exited: Vec<Ref> = arc.ephemerons.counted.keys().copied().filter(|p| !currents.contains(p)).collect();
        for p in exited {
            if let Some((_, v0)) = arc.ephemerons.remove_it(p) {
                release.push(v0);
            }
        }
        for v in release {
            arc.estado.release(v).unwrap_or_else(|e| arc_failure(e));
        }
        if let Some(start) = start {
            phases.push(start.elapsed().as_micros());
        }
        // 4. Os jovens: o registrado fica, o resto morreu.
        let mut dead_young: Vec<Ref> = Vec::new();
        let n_young = young_set.len();
        for &b in &young_set {
            let h = b as i64 + DESLOCAMENTO_DO_HANDLE;
            if arc.estado.decide_young(h) {
                // SAFETY: bloco entregue desde a última drenagem, ocupado.
                unsafe {
                    (*b).estado = VELHO;
                    mark_block(b);
                }
            } else {
                dead_young.push(h);
            }
        }
        if let Some(start) = start {
            phases.push(start.elapsed().as_micros());
        }
        for &h in &dead_young {
            // SAFETY: as posições são palavras do corpo de um bloco ainda não solto.
            self.visit_strong_references(h, &mut |v| value_list.push(v));
            for v in value_list.drain(..) {
                arc.estado.release(v).unwrap_or_else(|e| arc_failure(e));
            }
        }
        if let Some(start) = start {
            phases.push(start.elapsed().as_micros());
        }
        self.mark_side_edges(&mut arc);
        // As raízes não geram decremento ao sumir: o que elas seguram volta a
        // ser candidato a ciclo.
        for &h in &protected_set {
            arc.estado.add_candidate(h);
        }
        let protected = |h: Ref| protected_set.contains(&h);
        if let Some(start) = start {
            phases.push(start.elapsed().as_micros());
        }
        // 5. Zeros, ciclos e o ponto fixo da completa.
        let round = full && arc.cycles != Some(false);
        let mut fixpoint = round && !self.ephemerons.is_empty();
        let cycles = arc.cycles == Some(true) || (round && !fixpoint);
        let mut ready = Vec::new();
        let mut attachment_edges = Vec::new();
        let mut natives = Vec::new();
        let mut finalize = Vec::new();
        let mut dead_rc: Vec<Ref> = Vec::new();
        let mut first_one = true;
        arc.estado.resume_deferred();
        // O laço por partes, para o rastro: zeros, ciclos, ponto fixo dos
        // efêmeros, tabelas laterais (µs).
        let mut run_loop = [0u128; 4];
        loop {
            let t0 = trace.then(std::time::Instant::now);
            {
                let HeapArc { estado, ephemerons, .. } = &mut *arc;
                let mut graph = HeapGraph { heap: self, ephemerons };
                estado.drain_zeros(&mut graph, usize::MAX, &protected).unwrap_or_else(|e| arc_failure(e));
                if let Some(t0) = t0 {
                    run_loop[0] += t0.elapsed().as_micros();
                }
                if cycles {
                    let t1 = trace.then(std::time::Instant::now);
                    estado.collect_cycles(&mut graph, &protected).unwrap_or_else(|e| arc_failure(e));
                    estado.drain_zeros(&mut graph, usize::MAX, &protected).unwrap_or_else(|e| arc_failure(e));
                    if let Some(t1) = t1 {
                        run_loop[1] += t1.elapsed().as_micros();
                    }
                }
            }
            if fixpoint {
                let t2 = trace.then(std::time::Instant::now);
                fixpoint = false;
                self.arc_fixpoint(&mut arc);
                let HeapArc { estado, ephemerons, .. } = &mut *arc;
                let mut graph = HeapGraph { heap: self, ephemerons };
                estado.drain_zeros(&mut graph, usize::MAX, &protected).unwrap_or_else(|e| arc_failure(e));
                if let Some(t2) = t2 {
                    run_loop[2] += t2.elapsed().as_micros();
                }
            }
            let t3 = trace.then(std::time::Instant::now);
            let amount = arc.estado.transfer_dead(&mut dead_rc);
            // Jovens também exigem limpeza lateral na primeira rodada, mas
            // são reclamados pela varredura: não entram nos mortos por RC.
            let has_dead = amount != 0 || (first_one && !dead_young.is_empty());
            first_one = false;
            if !has_dead {
                break;
            }
            // As tabelas laterais (pequenas) perdem os mortos: o objeto do
            // espaço sem metadados morreu nesta drenagem — depois do passo 4,
            // todo vivo do espaço é registrado. Um conjunto com os mortos da
            // rodada (milhões, nas árvores que caem de uma vez) custava mais
            // que a cascata.
            let validate = self.validate_handles;
            let espaco = &self.objects;
            let estado = &arc.estado;
            let dead = |h: &Ref| died_in_drain(espaco, validate, estado, *h);
            self.weak_refs.retain(|holder, target| {
                if dead(target) {
                    *target = 0;
                }
                !dead(holder)
            });
            let holders: Vec<Ref> = self.ephemerons.keys().copied().filter(|p| dead(p)).collect();
            // A chave morta zera a entrada (o rompimento da chave registrada
            // já zerou; a do jovem morto, aqui).
            for e in self.ephemerons.values_mut() {
                if dead(&e.0) {
                    *e = (0, 0);
                }
            }
            self.attachments.retain_mut(|a| {
                if matches!(a.action, FinalizerAction::Native(_, _, _)) && dead(&a.owner) {
                    a.owner = 0;
                }
                if dead(&a.detach) {
                    a.detach = 0;
                }
                if !dead(&a.value) {
                    return true;
                }
                match a.action {
                    FinalizerAction::Dart(action) => {
                        ready.push(action);
                        attachment_edges.push((a.owner, action));
                    },
                    FinalizerAction::Native(f, token, bytes) => natives.push((f, token, bytes)),
                }
                false
            });
            Self::reindex_attachment_list(&self.attachments, &mut self.attachments_by_owner);
            self.late_fields_initialized.retain(|(h, _)| !dead(h));
            self.new_lates.retain(|(h, _)| !dead(h));
            self.permanentes.retain(|h| !dead(h));
            self.constants.retain(|h, _| !dead(h));
            self.tearoff_code.retain(|h, _| !dead(h));
            self.finalizable.retain(|h, &mut pair| {
                let stays = !dead(h);
                if !stays {
                    finalize.push(pair);
                }
                stays
            });
            for p in holders {
                self.ephemerons.remove(&p);
                if let Some((_, v)) = arc.ephemerons.remove_it(p) {
                    arc.estado.release(v).unwrap_or_else(|e| arc_failure(e));
                }
            }
            if let Some(t3) = t3 {
                run_loop[3] += t3.elapsed().as_micros();
            }
        }
        self.reindex_attachments();
        for &action in &ready { self.arc_retain_registering(&mut arc, action); }
        for (_, action) in attachment_edges {
            arc.estado.release(action).unwrap_or_else(|e| arc_failure(e));
        }
        self.ready_finalizations.extend(ready);
        if let Some(start) = start {
            phases.push(start.elapsed().as_micros());
        }
        // 6. A memória.
        let (dead_swept, mut released_set) = self.objects.sweep_young();
        for &h in &dead_rc {
            if let Some(b) = self.space_block(h) {
                // SAFETY: bloco do morto pelo RC, fora da lista dos jovens
                // (a varredura acima a esvaziou), que ninguém mais lê.
                released_set += unsafe { self.objects.release_dead(b) };
            }
        }
        self.stats.reclaimed += (dead_swept + dead_rc.len()) as u64;
        self.subtract_estimated_bytes(released_set);
        self.allocations = 0;
        self.young_bytes = 0;
        if full {
            let live_set = self.objects.live_set;
            self.minors_since_full = 0;
            self.work_in_full = live_set;
            self.recompute_triggers(live_set);
        } else {
            self.minors_since_full += 1;
        }
        if let Some(start) = start {
            phases.push(start.elapsed().as_micros());
        }
        if arc.check {
            self.check_arc(&mut arc);
        }
        if let Some(start) = start {
            let mut previous = 0;
            let stages: Vec<String> = phases
                .iter()
                .map(|&t| {
                    let d = t - previous;
                    previous = t;
                    d.to_string()
                })
                .collect();
            let e = arc.estado.statistics;
            eprintln!(
                "[arc] {} us={} fases={} laco={}/{}/{}/{} jovens={n_young} mortos_jovens={} fotos={n_snapshots} contados={} retains={} releases={} mortos_rc={} mortos_ciclo={} rodadas={} examinados={} candidatos={} zeros_vistos={} adiados={}",
                if full { "completa" } else { "drenagem" },
                start.elapsed().as_micros(),
                stages.join("/"),
                run_loop[0],
                run_loop[1],
                run_loop[2],
                run_loop[3],
                dead_young.len(),
                arc.estado.registered_set(),
                e.retains,
                e.releases,
                e.dead_by_rc,
                e.dead_by_cycle,
                e.cycle_rounds,
                e.examined,
                arc.estado.candidates(),
                e.seen_zeros,
                e.deferred_zeros
            );
        }
        self.arc = Some(arc);
        for (finalizer, pair) in finalize {
            finalizer(pair);
        }
        for (f, token, bytes) in natives {
            self.count_external(-(bytes as isize));
            // SAFETY: a `NativeFinalizerFunction` do anexo, `void f(void*)`;
            // como na VM, roda durante a coleta e não pode tocar o heap.
            let f: extern "C" fn(usize) = unsafe { std::mem::transmute(f) };
            f(token);
        }
    }

    /// A reclamação física (§19.4): as marcas passam a ser os vivos do RC e
    /// a varredura completa devolve os blocos dos mortos (e solta os corpos
    /// de fora e os anexos deles).
    fn reclaim_arc(&mut self, arc: &HeapArc) {
        self.objects.clear_marks();
        let mut live_set = 0usize;
        for h in arc.estado.live_set() {
            if let Some(b) = self.space_block(h) {
                // SAFETY: bloco vivo do espaço.
                unsafe { mark_block(b) };
                live_set += 1;
            }
        }
        self.objects.marked_set += live_set;
        let (dead_set, _, object_bytes) = self.objects.sweep();
        self.stats.reclaimed += dead_set as u64;
        self.set_estimated_bytes((object_bytes as u128).saturating_add(self.external));
        self.minors_since_full = 0;
        self.work_in_full = live_set;
        self.recompute_triggers(live_set);
    }

    /// `DARTFORGE_ARC_CONFERIR=1` (§20.3, o modo auditor): nenhum objeto
    /// alcançável das raízes morreu, e o RC de cada vivo é a soma das
    /// ocorrências recontadas.
    ///
    /// # Panics
    /// Com a primeira divergência.
    fn check_arc(&mut self, arc: &mut HeapArc) {
        let mut stack = Vec::new();
        self.root_list(&mut stack);
        let root_list: crate::hash::HashSet<Ref> = stack.iter().copied().collect();
        let mut origin: crate::hash::HashMap<Ref, Ref> = crate::hash::HashMap::default();
        let mut seen: crate::hash::HashSet<Ref> = crate::hash::HashSet::default();
        while let Some(h) = stack.pop() {
            if !smi::e_handle(h) || !is_object(h) || self.space_block(h).is_none() || !seen.insert(h) {
                continue;
            }
            if !arc.estado.alive(h) {
                let parent_node = origin.get(&h).copied();
                let desc = |x: Ref| {
                    let c = self.header(x);
                    let pos = self.words(x).iter().position(|&w| w == h);
                    format!(
                        "cid {} estado {} flags {:#x} n {} posição {pos:?} metadados {:?} foto {}",
                        c.class_id,
                        c.estado,
                        c.flags,
                        c.n,
                        arc.estado.meta(x),
                        arc.snapshots.contains_key(&x)
                    )
                };
                panic!(
                    "bug do ARC: objeto alcançável sem RC vivo: handle {h} (cid {}, estado {}), metadados {:?}, raiz {}; alcançado de {parent_node:?} ({})",
                    self.header(h).class_id,
                    self.header(h).estado,
                    arc.estado.meta(h),
                    root_list.contains(&h),
                    parent_node.map(desc).unwrap_or_default()
                );
            }
            // SAFETY: as posições são palavras do corpo de um bloco vivo.
            self.visit_strong_references(h, &mut |v| {
                if smi::e_handle(v) && !seen.contains(&v) {
                    origin.entry(v).or_insert(h);
                    stack.push(v);
                }
            });
        }
        let HeapArc { estado, ephemerons, .. } = arc;
        let owning_set = self.owning_roots();
        let graph = HeapGraph { heap: self, ephemerons };
        if let Err(e) = estado.audit(&graph, &owning_set) {
            if let crate::arc::ArcError::Audit { handle, .. } = e {
                panic!("bug do ARC: {e:?} (cid {})", graph.heap.header(handle).class_id);
            }
            arc_failure(e);
        }
    }
}

#[cfg(test)]
mod arc_no_heap {
    //! O ARC sobre o heap real (docs/ARC-CICLOS-ESPECIFICACAO.md §19.2, §22.1).
    use super::*;
    use crate::layout::{self, cid, flags};

    fn list(heap: &mut Heap, len: usize) -> Ref {
        let h = heap.allocate(cid::LIST, layout::list_words(len), flags::REFS);
        heap.words_mut(h)[0] = len as i64;
        h
    }

    fn heap_arc() -> Heap {
        let mut heap = Heap::new(false);
        heap.verify = true;
        heap.enable_arc();
        heap.arc.as_mut().unwrap().check = true;
        heap
    }

    #[test]
    fn global_receives_token_without_retaining_and_keeps_owner_apart_from_scalar() {
        for mode in [None, Some(false), Some(true)] {
            let mut heap = if mode.is_some() { heap_arc() } else { Heap::new(true) };
            if let Some(pure) = mode { heap.arc.as_mut().unwrap().pure = pure; }
            let value = heap.allocate_str("global owned");
            heap.retain_code_owner(value);
            heap.move_code_to_global(10, value);
            assert!(heap.code_owners.is_empty());
            heap.collect();
            assert!(heap.is_live_object(value));
            if let Some(arc) = &heap.arc { assert_eq!(arc.estado.meta(value).unwrap().rc, 1); }
            heap.retain_code_owner(value);
            heap.move_code_to_global(10, value);
            if let Some(arc) = &heap.arc { assert_eq!(arc.estado.meta(value).unwrap().rc, 1); }
            assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                heap.move_code_to_global(10, value);
            })).is_err());
            assert_eq!(heap.globals[&10], value);
            heap.retain_code_owner(value);
            heap.move_code_to_global(10, smi::de(42).unwrap());
            assert!(!heap.globals.contains_key(&10), "owner não é armazenamento escalar");
            heap.collect();
            assert!(heap.is_live_object(value), "token SSA independente protege carga");
            heap.release_code_owner(value);
            heap.collect();
            assert!(!heap.is_live_object(value));
            heap.move_code_to_global(10, 0);
        }
    }

    #[test]
    fn slots_and_tokens_transfer_without_retain_on_move_and_with_alias_at_destination() {
        for mode in [None, Some(false), Some(true)] {
            let mut heap = if mode.is_some() {
                heap_arc()
            } else {
                Heap::new(true)
            };
            if let Some(pure) = mode {
                heap.arc.as_mut().unwrap().pure = pure;
            }
            let frame = heap.push_owning_frame(2);
            let a = heap.allocate_str("carga owned");
            heap.set_root(frame, 0, a);
            let old = heap.allocate_str("destino substituído");
            heap.set_root(frame, 1, old);
            assert_eq!(heap.copy_root_to_code(frame, 0), a);
            assert_eq!(heap.copy_root_to_code(frame, 0), a);
            heap.move_code_to_root(frame, 1, a);
            heap.collect();
            assert!(!heap.is_live_object(old));
            if let Some(arc) = &heap.arc {
                assert_eq!(arc.estado.meta(a).unwrap().rc, 3);
            }
            // O destino já contém a: consome o token e a ocorrência antiga.
            heap.move_code_to_root(frame, 1, a);
            assert!(heap.code_owners.is_empty());
            if let Some(arc) = &heap.arc {
                assert_eq!(arc.estado.meta(a).unwrap().rc, 2);
            }
            assert_eq!(heap.copy_root_to_code(frame, 1), a);
            heap.pop_frame(frame);
            heap.collect();
            assert!(heap.is_live_object(a));
            if let Some(arc) = &heap.arc {
                assert_eq!(arc.estado.meta(a).unwrap().rc, 1);
            }
            heap.release_code_owner(a);
            heap.collect();
            assert!(!heap.is_live_object(a));
        }
    }

    #[test]
    fn transfer_rejects_invalid_destination_or_token_before_mutating() {
        let mut heap = heap_arc();
        let frame = heap.push_owning_frame(1);
        let a = heap.allocate_str("owner do slot não é token do código");
        heap.set_root(frame, 0, a);
        let failure = |heap: &mut Heap, q, s, value| {
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    heap.move_code_to_root(q, s, value);
                }))
                .is_err()
            );
        };
        failure(&mut heap, frame, 0, a);
        assert!(heap.code_owners.is_empty());
        heap.retain_code_owner(a);
        failure(&mut heap, frame, 1, a);
        failure(&mut heap, -1, 0, a);
        let observational = heap.push_frame_with_slots(1);
        failure(&mut heap, observational, 0, a);
        assert_eq!(heap.code_owners[&a], 1);
        assert_eq!(heap.arc.as_ref().unwrap().estado.meta(a).unwrap().rc, 2);
        heap.pop_frame(observational);
        heap.collect();
        assert!(heap.is_live_object(a));
        heap.release_code_owner(a);
        heap.pop_frame(frame);
        heap.collect();
        assert!(!heap.is_live_object(a));
    }

    #[test]
    fn code_tokens_count_aliases_without_duplicating_observational_roots() {
        for mode in [None, Some(false), Some(true)] {
            let mut heap = if mode.is_some() { heap_arc() } else { Heap::new(true) };
            if let Some(pure) = mode { heap.arc.as_mut().unwrap().pure = pure; }
            let frame = heap.push_frame_with_slots(1);
            let value = heap.allocate_str("tokens do código");
            heap.set_root(frame, 0, value);
            heap.retain_code_owner(value);
            heap.retain_code_owner(value);
            heap.collect();
            assert!(heap.is_live_object(value));
            if let Some(arc) = &heap.arc {
                assert_eq!(arc.estado.meta(value).unwrap().rc, 2);
            }
            heap.set_root(frame, 0, 0);
            heap.release_code_owner(value);
            heap.collect();
            assert!(heap.is_live_object(value));
            if let Some(arc) = &heap.arc {
                assert_eq!(arc.estado.meta(value).unwrap().rc, 1);
            }
            heap.release_code_owner(value);
            assert!(heap.is_live_object(value), "release não coleta");
            heap.collect();
            assert!(!heap.is_live_object(value));
            assert!(heap.code_owners.is_empty());
            heap.pop_frame(frame);
            for scalar in [0, smi::de(42).unwrap()] {
                heap.retain_code_owner(scalar);
                heap.release_code_owner(scalar);
            }
            assert!(heap.code_owners.is_empty());
        }
    }

    #[test]
    #[should_panic(expected = "release ARC sem token do código")]
    fn code_release_does_not_consume_frame_owner() {
        let mut heap = heap_arc();
        let frame = heap.push_owning_frame(1);
        let value = heap.allocate_str("somente slot");
        heap.set_root(frame, 0, value);
        heap.release_code_owner(value);
    }

    #[test]
    fn arc_activation_recounts_tokens_and_last_release_allows_collecting_cycle() {
        let mut heap = Heap::new(true);
        let a = list(&mut heap, 1);
        heap.retain_code_owner(a);
        heap.retain_code_owner(a);
        let b = list(&mut heap, 1);
        heap.write_refs(a, 1, &[b]);
        heap.write_refs(b, 1, &[a]);
        heap.enable_arc();
        heap.arc.as_mut().unwrap().check = true;
        heap.collect();
        assert_eq!(heap.arc.as_ref().unwrap().estado.meta(a).unwrap().rc, 3);
        heap.release_code_owner(a);
        heap.collect();
        assert!(heap.is_live_object(a) && heap.is_live_object(b));
        heap.release_code_owner(a);
        assert!(heap.is_live_object(a) && heap.is_live_object(b));
        heap.collect();
        assert!(!heap.is_live_object(a) && !heap.is_live_object(b));
    }

    #[test]
    fn finalization_queue_counts_aliases_and_consumption() {
        for pure in [false, true] {
            let mut heap = heap_arc();
            heap.arc.as_mut().unwrap().pure = pure;
            let action = heap.allocate_str("duas finalizações");
            heap.add_ready_finalization(action);
            heap.add_ready_finalization(action);
            heap.collect();
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(action).unwrap().rc, 2);
            assert_eq!(heap.finish_ready_finalization(), Some(action));
            heap.collect();
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(action).unwrap().rc, 1);
            heap.encerrar_finalizadores();
            assert_eq!(heap.finish_ready_finalization(), None);
            heap.collect();
            assert!(!heap.is_live_object(action));
        }
    }

    #[test]
    fn dead_attachment_publishes_action_owner_in_queue() {
        for (pure, minor) in [(false, false), (false, true), (true, false), (true, true)] {
            let mut heap = heap_arc();
            heap.arc.as_mut().unwrap().pure = pure;
            let frame = heap.push_owning_frame(3);
            let owner = heap.allocate_str("finalizador");
            heap.set_root(frame, 0, owner);
            let action = heap.allocate_str("ação pronta");
            heap.set_root(frame, 1, action);
            let value = heap.allocate_str("alvo fraco");
            heap.set_root(frame, 2, value);
            heap.add_attachment(FinalizerAttachment {
                owner, value, detach: 0, action: FinalizerAction::Dart(action),
            });
            heap.set_root(frame, 2, 0);
            heap.coletar(minor);
            assert!(heap.attachments.is_empty());
            assert_eq!(heap.ready_finalizations.front(), Some(&action));
            assert!(!heap.is_live_object(value));
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(action).unwrap().rc, 2);
            heap.pop_frame(frame);
            heap.collect();
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(action).unwrap().rc, 1);
            assert_eq!(heap.finish_ready_finalization(), Some(action));
            heap.collect();
            assert!(!heap.is_live_object(action));
        }
    }

    #[test]
    fn dart_attachment_does_not_root_cycle_between_owner_and_action() {
        for mode in [None, Some(false), Some(true)] {
            let mut heap = if mode.is_some() { heap_arc() } else { Heap::new(true) };
            if let Some(pure) = mode { heap.arc.as_mut().unwrap().pure = pure; }
            let frame = heap.push_owning_frame(3);
            let owner = list(&mut heap, 0);
            heap.set_root(frame, 0, owner);
            let action = list(&mut heap, 1);
            heap.set_root(frame, 1, action);
            heap.write_refs(action, 1, &[owner]);
            let target = list(&mut heap, 0);
            heap.set_root(frame, 2, target);
            heap.add_attachment(FinalizerAttachment {
                owner, value: target, detach: 0, action: FinalizerAction::Dart(action),
            });
            heap.pop_frame(frame);
            heap.collect();
            assert!(!heap.is_live_object(owner), "o anexo não é raiz do finalizador");
            assert!(!heap.is_live_object(action));
            assert!(!heap.is_live_object(target));
            assert!(heap.attachments.is_empty());
            assert!(heap.ready_finalizations.is_empty(), "dono e alvo mortos cancelam a ação Dart");
        }
    }

    #[test]
    fn attachment_index_preserves_aliases_after_removing_middle_position() {
        for pure in [false, true] {
            let mut heap = heap_arc();
            heap.arc.as_mut().unwrap().pure = pure;
            let frame = heap.push_owning_frame(3);
            let a = list(&mut heap, 0);
            heap.set_root(frame, 0, a);
            let b = list(&mut heap, 0);
            heap.set_root(frame, 1, b);
            let action = list(&mut heap, 0);
            heap.set_root(frame, 2, action);
            for (owner, key) in [(a, 3), (b, 7), (a, 5)] {
                heap.add_attachment(FinalizerAttachment {
                    owner, value: owner, detach: key, action: FinalizerAction::Dart(action),
                });
            }
            heap.set_root(frame, 2, 0);
            heap.collect();
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(action).unwrap().rc, 3);
            heap.detach_finalizer(b, 7);
            let mut edges = Vec::new();
            heap.visit_attachment_edges(a, &mut |v| edges.push(v));
            assert_eq!(edges, [action, action]);
            heap.visit_attachment_edges(b, &mut |_| panic!("anexo removido no índice"));
            heap.collect();
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(action).unwrap().rc, 2);
            heap.detach_finalizer(a, 3);
            heap.collect();
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(action).unwrap().rc, 1);
            heap.detach_finalizer(a, 5);
            heap.collect();
            assert!(!heap.is_live_object(action));
            assert!(heap.attachments_by_owner.is_empty());
            heap.pop_frame(frame);
        }
    }

    #[test]
    fn finalizer_external_size_leaves_once_per_transition() {
        static RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        extern "C" fn finalize(_: usize) {
            RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        for mode in [None, Some(false), Some(true)] {
            let mut heap = if mode.is_some() { heap_arc() } else { Heap::new(false) };
            if let Some(pure) = mode { heap.arc.as_mut().unwrap().pure = pure; }
            heap.count_external(17);
            let frame = heap.push_owning_frame(2);
            let owner = list(&mut heap, 0);
            heap.set_root(frame, 0, owner);
            let target = list(&mut heap, 0);
            heap.set_root(frame, 1, target);
            let size = heap.byte_threshold + 1;
            let before = RUNS.load(std::sync::atomic::Ordering::Relaxed);
            let attachment = FinalizerAttachment {
                owner, value: target, detach: target,
                action: FinalizerAction::Native(finalize as *const () as usize, 0, size),
            };
            heap.add_attachment(attachment);
            assert!(heap.needs_collection(16), "externalSize deve provocar pressão de memória");
            heap.collect();
            assert_eq!(heap.external, size as u128 + 17);
            heap.detach_finalizer(owner, target);
            heap.detach_finalizer(owner, target);
            assert_eq!(heap.external, 17);
            assert_eq!(RUNS.load(std::sync::atomic::Ordering::Relaxed), before);

            heap.add_attachment(attachment);
            heap.set_root(frame, 1, 0);
            heap.collect();
            heap.collect();
            assert_eq!(heap.external, 17);
            assert_eq!(RUNS.load(std::sync::atomic::Ordering::Relaxed), before + 1);

            let target = list(&mut heap, 0);
            heap.set_root(frame, 1, target);
            heap.add_attachment(FinalizerAttachment { value: target, ..attachment });
            heap.encerrar_finalizadores();
            heap.encerrar_finalizadores();
            assert_eq!(heap.external, 17);
            assert_eq!(RUNS.load(std::sync::atomic::Ordering::Relaxed), before + 2);
            heap.pop_frame(frame);
        }
    }

    #[test]
    fn native_attachment_rejects_owner_reused_after_wrapper_death() {
        static RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        extern "C" fn finalize(token: usize) {
            assert_eq!(token, 17);
            RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        for mode in [None, Some(false), Some(true)] {
            let mut heap = if mode.is_some() { heap_arc() } else { Heap::new(false) };
            if let Some(pure) = mode { heap.arc.as_mut().unwrap().pure = pure; }
            let frame = heap.push_owning_frame(2);
            let owner = list(&mut heap, 0);
            heap.set_root(frame, 0, owner);
            let target = list(&mut heap, 0);
            heap.set_root(frame, 1, target);
            heap.add_attachment(FinalizerAttachment {
                owner, value: target, detach: target,
                action: FinalizerAction::Native(finalize as *const () as usize, 17, 0),
            });
            let before = RUNS.load(std::sync::atomic::Ordering::Relaxed);
            heap.set_root(frame, 0, 0);
            heap.collect();
            assert_eq!(heap.attachments.len(), 1);
            assert_eq!(heap.attachments[0].owner, 0);
            let retained = heap.push_owning_frame(4096);
            let new = (0..4096).find_map(|slot| {
                let new = list(&mut heap, 0);
                heap.set_root(retained, slot, new);
                (new == owner).then_some(new)
            }).expect("reutilizar o bloco após consumir a faixa livre da página");
            heap.set_root(frame, 0, new);
            heap.detach_finalizer(new, target);
            assert_eq!(heap.attachments.len(), 1);
            assert_eq!(RUNS.load(std::sync::atomic::Ordering::Relaxed), before);
            heap.pop_frame(retained);
            heap.pop_frame(frame);
            heap.collect();
            assert!(heap.attachments.is_empty());
            assert_eq!(RUNS.load(std::sync::atomic::Ordering::Relaxed), before + 1);
        }
    }

    #[test]
    fn old_owner_attachment_keeps_young_action_and_detach_releases_chain() {
        for pure in [false, true] {
            let mut heap = heap_arc();
            heap.arc.as_mut().unwrap().pure = pure;
            let frame = heap.push_owning_frame(1);
            let owner = list(&mut heap, 0);
            heap.set_root(frame, 0, owner);
            heap.collect();
            let temp_value = heap.push_owning_frame(2);
            let target = list(&mut heap, 0);
            heap.set_root(temp_value, 0, target);
            let action = list(&mut heap, 1);
            heap.set_root(temp_value, 1, action);
            heap.write_refs(action, 1, &[target]);
            heap.add_attachment(FinalizerAttachment {
                owner, value: target, detach: 3, action: FinalizerAction::Dart(action),
            });
            heap.pop_frame(temp_value);
            heap.coletar(true);
            assert!(heap.is_live_object(target));
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(action).unwrap().rc, 1);
            assert!(heap.ready_finalizations.is_empty());
            heap.detach_finalizer(owner, 3);
            heap.collect();
            assert!(!heap.is_live_object(action));
            assert!(!heap.is_live_object(target));
            assert!(heap.is_live_object(owner));
            heap.pop_frame(frame);
        }
    }

    fn alive(heap: &Heap, h: Ref) -> bool {
        heap.arc.as_ref().unwrap().estado.alive(h)
    }

    #[test]
    fn owning_frames_count_copies_swaps_and_promotion() {
        for pure in [true, false] {
            let mut heap = heap_arc();
            heap.arc.as_mut().unwrap().pure = pure;
            let owner = heap.push_owning_frame(2);
            let observer = heap.push_frame_with_slots(1);
            let a = list(&mut heap, 0);
            heap.set_root(owner, 0, a);
            heap.set_root(owner, 1, a);
            heap.set_root(observer, 0, a);
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(a).unwrap().rc, 2);
            heap.set_root(owner, 0, a);
            for _ in 0..2 {
                heap.coletar(true);
                assert_eq!(heap.arc.as_ref().unwrap().estado.meta(a).unwrap().rc, 2);
            }
            let b = list(&mut heap, 0);
            heap.set_root(owner, 0, b);
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(a).unwrap().rc, 1);
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(b).unwrap().rc, 1);
            heap.set_root(owner, 1, 0);
            heap.coletar(true);
            assert!(alive(&heap, a), "a raiz observacional ainda protege o zero");
            heap.pop_frame(observer);
            heap.pop_frame(owner);
            heap.collect();
            assert!(!alive(&heap, a) && !alive(&heap, b));
        }
    }

    #[test]
    fn owners_and_strong_edges_count_distinct_occurrences() {
        for pure in [true, false] {
            let mut heap = heap_arc();
            heap.arc.as_mut().unwrap().pure = pure;
            let frame = heap.push_owning_frame(1);
            let a = list(&mut heap, 1);
            heap.set_root(frame, 0, a);
            let b = list(&mut heap, 1);
            heap.root(frame, b);
            heap.write_ref(a, 1, b);
            heap.write_ref(b, 1, a);
            heap.collect();
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(a).unwrap().rc, 2);
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(b).unwrap().rc, 2);
            heap.set_root(frame, 0, smi::de(7).unwrap());
            heap.collect();
            assert!(alive(&heap, a) && alive(&heap, b));
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(a).unwrap().rc, 1);
            heap.pop_frame(frame);
            heap.collect();
            assert!(!alive(&heap, a) && !alive(&heap, b));
        }
    }

    #[test]
    fn owning_move_consuming_alias_and_return_preserves_rc() {
        for pure in [true, false] {
            let mut heap = heap_arc();
            heap.arc.as_mut().unwrap().pure = pure;
            let caller = heap.push_owning_frame(2);
            let callee = heap.push_owning_frame(1);
            let a = list(&mut heap, 0);
            heap.set_root(callee, 0, a);
            heap.set_root(caller, 0, a);
            heap.move_root(callee, 0, caller, 0);
            assert_eq!(heap.frames.last().unwrap().1[0], 0);
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(a).unwrap().rc, 1);
            heap.pop_frame(callee);
            heap.collect();
            assert!(alive(&heap, a));
            heap.move_root(caller, 0, caller, 0);
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(a).unwrap().rc, 1);
            let b = list(&mut heap, 0);
            heap.set_root(caller, 1, b);
            heap.move_root(caller, 0, caller, 1);
            heap.collect();
            assert!(alive(&heap, a) && !alive(&heap, b));
            heap.pop_frame(caller);
            heap.collect();
            assert!(!alive(&heap, a));
        }
    }

    #[test]
    fn move_rejects_observational_root_before_consuming_owner() {
        let mut heap = heap_arc();
        let owner = heap.push_owning_frame(1);
        let observer = heap.push_frame_with_slots(1);
        let a = list(&mut heap, 0);
        heap.set_root(owner, 0, a);
        let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            heap.move_root(owner, 0, observer, 0);
        }));
        assert!(failure.is_err());
        assert_eq!(heap.frames[0].1[0], a);
        assert_eq!(heap.frames[1].1[0], 0);
        assert_eq!(heap.arc.as_ref().unwrap().estado.meta(a).unwrap().rc, 1);
        heap.collect();
        heap.pop_frame(observer);
        heap.pop_frame(owner);
        heap.collect();
        assert!(!alive(&heap, a));
    }

    #[test]
    fn closure_construction_transfers_protection_to_strong_edge() {
        for pure in [true, false] {
            let mut heap = heap_arc();
            heap.arc.as_mut().unwrap().pure = pure;
            heap.stress = true;
            let frame = heap.push_owning_frame(1);
            let env = heap.new_context(&[(42, false)]);
            heap.set_root(frame, 0, env);
            let closure = heap.new_closure(123, (env, true), 0, 0);
            heap.set_root(frame, 0, closure);
            heap.collect();
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(env).unwrap().rc, 1);
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(closure).unwrap().rc, 1);
            assert_eq!(heap.closure(closure).unwrap().contexto.0, env);
            heap.pop_frame(frame);
            heap.collect();
            assert!(!alive(&heap, env) && !alive(&heap, closure));
        }
    }

    #[test]
    fn temporary_list_owners_survive_growth_and_decompaction() {
        for pure in [true, false] {
            let mut heap = heap_arc();
            heap.arc.as_mut().unwrap().pure = pure;
            heap.stress = true;
            let frame = heap.push_owning_frame(1);
            let l = heap.new_growable(1, 1, crate::listas::Element::Int);
            heap.set_root(frame, 0, l);
            heap.list_set(l, 0, Value::Int(i64::MAX));
            heap.list_push(l, Value::Double(2.5));
            heap.collect();
            assert_eq!(heap.list_shape(l), crate::listas::Element::General);
            assert!(matches!(heap.list_get(l, 0), Value::Ref(r) if matches!(heap.value(r), Value::Int(i64::MAX))));
            assert!(matches!(heap.list_get(l, 1), Value::Ref(r) if matches!(heap.value(r), Value::Double(x) if x == 2.5)));
            assert_eq!(heap.frames.len(), 1, "nenhum quadro temporário pode permanecer aberto");
            let data = heap.list_data(l);
            let caixas = heap.words(data)[1..3].to_vec();
            for &r in &caixas {
                assert_eq!(heap.arc.as_ref().unwrap().estado.meta(r).unwrap().rc, 1);
            }
            heap.pop_frame(frame);
            heap.collect();
            assert!(!alive(&heap, l) && !alive(&heap, data));
            assert!(caixas.iter().all(|&r| !alive(&heap, r)));
        }
    }

    #[test]
    fn typed_view_keeps_base_after_consuming_temporary_owners() {
        for pure in [true, false] {
            let mut heap = heap_arc();
            heap.arc.as_mut().unwrap().pure = pure;
            heap.stress = true;
            let frame = heap.push_owning_frame(1);
            let base = heap.new_typed(crate::tipadas::tipo::UINT8, 8);
            heap.set_root(frame, 0, base);
            let c = crate::tipadas::view_cid(crate::tipadas::tipo::UINT8, false);
            let first_one = heap.new_view(c, base, 2, 6);
            heap.set_root(frame, 0, first_one);
            let second_one = heap.new_view(c, first_one, 1, 3);
            heap.set_root(frame, 0, second_one);
            heap.collect();
            let vista = heap.tipada(second_one).unwrap();
            assert_eq!(vista.base, Some(base));
            assert_eq!(vista.offset, 3);
            assert_eq!(vista.len, 3);
            assert!(!alive(&heap, first_one));
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(base).unwrap().rc, 1);
            heap.pop_frame(frame);
            heap.collect();
            assert!(!alive(&heap, base) && !alive(&heap, second_one));
        }
    }

    #[test]
    fn persistent_owners_count_aliases_move_and_promotion() {
        for pure in [true, false] {
            let mut heap = heap_arc();
            heap.arc.as_mut().unwrap().pure = pure;
            let a = list(&mut heap, 0);
            heap.set_global_root(10, a);
            heap.set_global_root(20, a);
            heap.set_runtime_root(0, a);
            for _ in 0..2 {
                heap.coletar(true);
                assert_eq!(heap.arc.as_ref().unwrap().estado.meta(a).unwrap().rc, 3);
            }
            heap.move_global_root(10, 10);
            heap.set_global_root(20, a);
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(a).unwrap().rc, 3);
            heap.move_global_root(10, 20);
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(a).unwrap().rc, 2);
            heap.release_global_root(20);
            heap.release_global_root(20);
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(a).unwrap().rc, 1);
            heap.collect();
            assert!(alive(&heap, a));
            heap.set_runtime_root(0, 0);
            heap.collect();
            assert!(!alive(&heap, a));
        }
    }

    #[test]
    fn arc_activation_counts_globals_and_runtime_slots_open_under_tracing() {
        let mut heap = Heap::new(false);
        let a = list(&mut heap, 0);
        heap.set_global_root(10, a);
        heap.set_runtime_root(1, a);
        heap.enable_arc();
        heap.arc.as_mut().unwrap().check = true;
        assert_eq!(heap.arc.as_ref().unwrap().estado.meta(a).unwrap().rc, 2);
        let b = list(&mut heap, 0);
        heap.set_global_root(10, b);
        assert_eq!(heap.arc.as_ref().unwrap().estado.meta(a).unwrap().rc, 1);
        assert_eq!(heap.arc.as_ref().unwrap().estado.meta(b).unwrap().rc, 1);
        heap.set_runtime_root(1, 0);
        heap.collect();
        assert!(!alive(&heap, a) && alive(&heap, b));
        heap.set_global_root(10, smi::de(7).unwrap());
        heap.collect();
        assert!(!alive(&heap, b));
    }

    #[test]
    fn canonical_tables_count_slots_without_duplicate_registration() {
        for pure in [true, false] {
            let mut heap = heap_arc();
            heap.arc.as_mut().unwrap().pure = pure;
            heap.stress = true;
            let unidades = vec![65u16];
            let text = heap.allocate_str("A");
            heap.store_literal(unidades.clone(), text);
            heap.store_literal(unidades.clone(), text);
            let tearoff = heap.new_closure(123, (0, false), 0, 0);
            heap.register_tearoff(123, tearoff);
            heap.register_tearoff(123, tearoff);
            let enum_value_of = heap.enum_value(900, 0, "zero");
            assert_eq!(heap.enum_value(900, 0, "zero"), enum_value_of);
            for _ in 0..2 {
                heap.coletar(true);
                for h in [text, tearoff, enum_value_of] {
                    assert_eq!(heap.arc.as_ref().unwrap().estado.meta(h).unwrap().rc, 1);
                }
            }
            assert_eq!(heap.literal(&unidades), Some(text));
            assert_eq!(heap.registered_tearoff(123), Some(tearoff));
            let replacement = heap.new_closure(123, (0, false), 0, 0);
            heap.register_tearoff(123, replacement);
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(tearoff).unwrap().rc, 0);
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(replacement).unwrap().rc, 1);
            heap.collect();
            assert_eq!(heap.registered_tearoff(123), Some(replacement));
            assert!(!alive(&heap, tearoff), "o índice de identidade não é owner");
            assert!([text, replacement, enum_value_of].iter().all(|&h| alive(&heap, h)));
        }
    }

    #[test]
    fn message_owners_count_in_tracing_walk() {
        let mut heap = Heap::new(false);
        let text = heap.allocate_str("fila");
        let a = heap.retain_message_owner(text);
        let b = heap.retain_message_owner(text);
        let before = heap.stats().roots_scanned;
        heap.collect();
        assert_eq!(heap.stats().roots_scanned - before, 2);
        assert!(heap.is_live_object(text));
        heap.release_message_owner(a);
        let before = heap.stats().roots_scanned;
        heap.collect();
        assert_eq!(heap.stats().roots_scanned - before, 1);
        heap.release_message_owner(b);
        heap.collect();
        assert!(!heap.is_live_object(text));
    }

    #[test]
    fn arc_activation_counts_already_open_owning_frames() {
        let mut heap = Heap::new(false);
        let frame = heap.push_owning_frame(0);
        let a = list(&mut heap, 0);
        heap.root(frame, a);
        heap.root(frame, a);
        heap.enable_arc();
        heap.arc.as_mut().unwrap().check = true;
        assert_eq!(heap.arc.as_ref().unwrap().estado.meta(a).unwrap().rc, 2);
        heap.collect();
        heap.pop_frame(frame);
        heap.collect();
        assert!(!alive(&heap, a));
    }

    #[test]
    fn arc_promotes_counts_and_releases_by_rc() {
        let mut heap = heap_arc();
        let frame = heap.push_frame_with_slots(1);
        let a = list(&mut heap, 2);
        let b = list(&mut heap, 1);
        heap.write_ref(a, 1, b);
        heap.set_root(frame, 0, a);
        heap.coletar(true);
        assert!(alive(&heap, a) && alive(&heap, b));
        assert_eq!(heap.arc.as_ref().unwrap().estado.meta(b).unwrap().rc, 1);
        // a[0] = null num velho: b perde a única ocorrência e morre.
        heap.write_ref(a, 1, 0);
        heap.coletar(true);
        assert!(!alive(&heap, b));
        heap.pop_frame(frame);
        heap.coletar(true);
        assert!(!alive(&heap, a));
        heap.collect();
    }

    #[test]
    fn arc_roots_register_young_without_retaining_garbage_or_recounting_old() {
        let mut heap = heap_arc();
        let frame = heap.push_frame_with_slots(2);
        let old_gen = list(&mut heap, 0);
        heap.set_root(frame, 0, old_gen);
        heap.coletar(true);
        let generation = heap.arc.as_ref().unwrap().estado.meta(old_gen).unwrap().generation;
        let young = list(&mut heap, 0);
        let garbage = list(&mut heap, 0);
        heap.set_root(frame, 1, young);
        heap.coletar(true);
        assert!(alive(&heap, old_gen) && alive(&heap, young));
        assert!(!alive(&heap, garbage));
        assert_eq!(heap.arc.as_ref().unwrap().estado.meta(old_gen).unwrap().generation, generation);
        assert_eq!(heap.arc.as_ref().unwrap().estado.meta(young).unwrap().rc, 0);
        heap.pop_frame(frame);
        heap.coletar(true);
        assert!(!alive(&heap, old_gen) && !alive(&heap, young));
    }

    #[test]
    fn arc_cycle_dies_in_full_and_root_holds_it() {
        let mut heap = heap_arc();
        let frame = heap.push_frame_with_slots(1);
        let a = list(&mut heap, 1);
        let b = list(&mut heap, 1);
        heap.write_ref(a, 1, b);
        heap.write_ref(b, 1, a);
        heap.set_root(frame, 0, a);
        heap.collect();
        assert!(alive(&heap, a) && alive(&heap, b), "a raiz segura o ciclo");
        heap.pop_frame(frame);
        heap.coletar(true);
        assert!(alive(&heap, a), "a menor não roda a rodada de ciclos");
        heap.collect();
        assert!(!alive(&heap, a) && !alive(&heap, b));
    }

    #[test]
    fn arc_raw_write_into_old_reconciles_by_snapshot() {
        let mut heap = heap_arc();
        let frame = heap.push_frame_with_slots(1);
        let a = list(&mut heap, 2);
        let b = list(&mut heap, 1);
        let c = list(&mut heap, 1);
        heap.write_ref(a, 1, b);
        heap.write_ref(a, 2, c);
        heap.set_root(frame, 0, a);
        heap.coletar(true);
        // Troca crua: a = [c, c]; b perde, c ganha.
        let p = heap.words_mut(a);
        p[1] = c;
        heap.remember(a);
        heap.coletar(true);
        assert!(!alive(&heap, b));
        assert_eq!(heap.arc.as_ref().unwrap().estado.meta(c).unwrap().rc, 2);
        heap.pop_frame(frame);
        heap.collect();
    }

    // -- Grafos aleatórios com oráculo independente (§26.2) ------------------

    /// xorshift64*: a sequência de cada semente é reproduzível.
    struct Rng(u64);

    impl Rng {
        fn new(seed: u64) -> Rng {
            Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
        }
        fn next_ix(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            self.0 = x;
            x.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }
        fn until(&mut self, n: usize) -> usize {
            (self.next_ix() % n as u64) as usize
        }
    }

    /// O alvo de uma fraca ou a entrada de um efêmero no modelo. O alvo (ou a
    /// chave) que deixou de ser alcançável vira `Pending`: a coleta pode
    /// zerá-lo a qualquer momento, e a completa tem de zerá-lo.
    #[derive(Clone, Copy, PartialEq, Debug)]
    enum Weak<T> {
        Holds(T),
        Pending,
    }

    /// O grafo do lado do teste: listas com os elementos, os slots do quadro,
    /// as fracas e os efêmeros. O alcance é calculado só daqui, sem olhar o
    /// heap: o forte pelas raízes e o ponto fixo condicional dos efêmeros
    /// (o valor de um portador alcançado é alcançado quando a chave é).
    #[derive(Default)]
    struct Model {
        fields: crate::hash::HashMap<Ref, Vec<Ref>>,
        root_list: Vec<Ref>,
        weak_refs: crate::hash::HashMap<Ref, Weak<Ref>>,
        ephemerons: crate::hash::HashMap<Ref, Weak<(Ref, Ref)>>,
        /// Os registrados no ARC que ficaram inalcançáveis: a completa tem de
        /// matá-los.
        garbage: Vec<Ref>,
    }

    impl Model {
        fn reachable(&self) -> crate::hash::HashSet<Ref> {
            let mut live_set = crate::hash::HashSet::default();
            let mut stack: Vec<Ref> = self.root_list.iter().copied().filter(|&h| h != 0).collect();
            loop {
                while let Some(h) = stack.pop() {
                    if live_set.insert(h) {
                        stack.extend(self.fields[&h].iter().copied().filter(|&f| f != 0));
                    }
                }
                for (p, e) in &self.ephemerons {
                    if let Weak::Holds((k, v)) = *e
                        && live_set.contains(p)
                        && live_set.contains(&k)
                        && !live_set.contains(&v)
                    {
                        stack.push(v);
                    }
                }
                if stack.is_empty() {
                    return live_set;
                }
            }
        }

        fn usable(&self) -> Vec<Ref> {
            let mut v: Vec<Ref> = self.reachable().into_iter().collect();
            v.sort_unstable();
            v
        }

        fn draw(&self, r: &mut Rng) -> Option<Ref> {
            let v = self.usable();
            if v.is_empty() { None } else { Some(v[r.until(v.len())]) }
        }
    }

    /// Depois de uma coleta: o alcançável continua com os elementos do
    /// modelo (e, no ARC, registrado vivo); a fraca e o efêmero de alvo
    /// alcançável continuam, e os de alvo inalcançável viram `Pending` (a
    /// completa os exige zerados). Os inalcançáveis saem do modelo.
    fn check_model(heap: &Heap, m: &mut Model, full: bool, arc: bool, contexto: &str) {
        let live_set = m.reachable();
        for &h in &live_set {
            if arc {
                assert!(alive(heap, h), "{contexto}: alcançável {h:#x} sem RC vivo");
            }
            let p = heap.words(h);
            for (i, &f) in m.fields[&h].iter().enumerate() {
                assert_eq!(p[1 + i], f, "{contexto}: elemento {i} de {h:#x}");
            }
        }
        if full && arc {
            let current_time: Vec<Ref> = m.fields.keys().copied().filter(|h| !live_set.contains(h)).collect();
            m.garbage.extend(current_time);
            for &h in &m.garbage {
                assert!(
                    !alive(heap, h),
                    "{contexto}: inalcançável {h:#x} sobreviveu à completa ({:?}; efêmeros {:?}; fracas {:?})",
                    heap.arc.as_ref().unwrap().estado.meta(h),
                    heap.ephemerons.iter().filter(|(p, (k, v))| **p == h || *k == h || *v == h).collect::<Vec<_>>(),
                    heap.weak_refs.iter().filter(|(p, a)| **p == h || **a == h).collect::<Vec<_>>(),
                );
            }
            m.garbage.clear();
        }
        let dead_set: Vec<Ref> = m.fields.keys().copied().filter(|h| !live_set.contains(h)).collect();
        for h in dead_set {
            m.fields.remove(&h);
            if arc && alive(heap, h) {
                m.garbage.push(h);
            }
        }
        let model_fields = &m.fields;
        m.weak_refs.retain(|p, target| {
            if !live_set.contains(p) {
                return false;
            }
            let current = heap.weak_refs.get(p).copied().unwrap_or(0);
            match *target {
                Weak::Holds(a) if live_set.contains(&a) => assert_eq!(current, a, "{contexto}: fraca {p:#x} com alvo vivo"),
                Weak::Holds(a) => {
                    assert!(current == a || current == 0, "{contexto}: fraca {p:#x}");
                    *target = Weak::Pending;
                }
                Weak::Pending => {}
            }
            if full && *target == Weak::Pending {
                assert_eq!(
                    current,
                    0,
                    "{contexto}: fraca {p:#x} não zerada na completa (alvo {:?}; efêmeros {:?}; quem aponta {:?})",
                    heap.arc.as_ref().map(|a| a.estado.meta(current)),
                    heap.ephemerons
                        .iter()
                        .filter(|(q, (k, v))| **q == current || *k == current || *v == current)
                        .map(|(q, kv)| (*q, *kv, live_set.contains(q), live_set.contains(&kv.0), heap.arc.as_ref().map(|a| a.estado.alive(*q))))
                        .collect::<Vec<_>>(),
                    model_fields.iter().filter(|(_, c)| c.contains(&current)).map(|(h, _)| (*h, live_set.contains(h))).collect::<Vec<_>>(),
                );
            }
            true
        });
        m.ephemerons.retain(|p, e| {
            if !live_set.contains(p) {
                return false;
            }
            let current = heap.ephemerons.get(p).copied().unwrap_or((0, 0));
            match *e {
                Weak::Holds((k, v)) if live_set.contains(&k) => assert_eq!(current, (k, v), "{contexto}: efêmero {p:#x} com chave viva"),
                Weak::Holds((k, v)) => {
                    assert!(current == (k, v) || current == (0, 0), "{contexto}: efêmero {p:#x}");
                    *e = Weak::Pending;
                }
                Weak::Pending => {}
            }
            if full && *e == Weak::Pending {
                assert_eq!(current, (0, 0), "{contexto}: efêmero {p:#x} não zerado na completa");
            }
            true
        });
    }

    /// Uma semente: alocações, gravações contadas e cruas, raízes que entram
    /// e saem, fracas e efêmeros, coletas menores e completas, com a auditoria
    /// do ARC (`check`) em toda sincronização e o oráculo do modelo depois
    /// de cada coleta.
    fn random_graph(seed: u64, arc: bool, cycles_always: bool, steps: usize) {
        const SLOTS: usize = 6;
        let mut r = Rng::new(seed);
        let mut heap = if arc { heap_arc() } else { Heap::new(false) };
        if cycles_always {
            heap.arc.as_mut().unwrap().cycles = Some(true);
        }
        heap.verify = true;
        let frame = heap.push_frame_with_slots(SLOTS);
        let mut m = Model { root_list: vec![0; SLOTS], ..Model::default() };
        for step in 0..steps {
            let contexto = format!("semente {seed}, passo {step}, arc {arc}");
            match r.until(20) {
                0..=4 => {
                    let len = 1 + r.until(3);
                    let h = list(&mut heap, len);
                    let s = r.until(SLOTS);
                    heap.set_root(frame, s, h);
                    m.root_list[s] = h;
                    m.fields.insert(h, vec![0; len]);
                }
                5..=8 => {
                    if let Some(a) = m.draw(&mut r) {
                        let i = r.until(m.fields[&a].len());
                        let b = if r.until(5) == 0 { 0 } else { m.draw(&mut r).unwrap_or(0) };
                        if r.until(3) == 0 {
                            heap.words_mut(a)[1 + i] = b;
                            heap.remember(a);
                        } else {
                            heap.write_ref(a, 1 + i, b);
                        }
                        m.fields.get_mut(&a).unwrap()[i] = b;
                    }
                }
                9..=10 => {
                    let s = r.until(SLOTS);
                    heap.set_root(frame, s, 0);
                    m.root_list[s] = 0;
                }
                11 => {
                    if let Some(h) = m.draw(&mut r) {
                        let s = r.until(SLOTS);
                        heap.set_root(frame, s, h);
                        m.root_list[s] = h;
                    }
                }
                12 => {
                    if let (Some(p), Some(a)) = (m.draw(&mut r), m.draw(&mut r))
                        && !m.ephemerons.contains_key(&p)
                    {
                        heap.weak_refs.insert(p, a);
                        m.weak_refs.insert(p, Weak::Holds(a));
                    }
                }
                13..=14 => {
                    if let (Some(p), Some(k), Some(v)) = (m.draw(&mut r), m.draw(&mut r), m.draw(&mut r))
                        && !m.weak_refs.contains_key(&p)
                    {
                        heap.ephemerons.insert(p, (k, v));
                        m.ephemerons.insert(p, Weak::Holds((k, v)));
                    }
                }
                15..=17 => {
                    heap.coletar(true);
                    check_model(&heap, &mut m, false, arc, &contexto);
                }
                _ => {
                    heap.collect();
                    check_model(&heap, &mut m, true, arc, &contexto);
                }
            }
        }
        for s in 0..SLOTS {
            heap.set_root(frame, s, 0);
            m.root_list[s] = 0;
        }
        heap.collect();
        check_model(&heap, &mut m, true, arc, &format!("semente {seed}, fim, arc {arc}"));
        if arc {
            assert_eq!(heap.arc.as_ref().unwrap().estado.live_set().count(), 0, "semente {seed}: sobrou vivo sem raiz");
        }
        heap.pop_frame(frame);
    }

    /// Quantas sementes: `DF_ARC_SEMENTES` amplia a busca fora do CI.
    fn seeds(pattern: u64) -> u64 {
        std::env::var("DF_ARC_SEMENTES").ok().and_then(|v| v.parse().ok()).unwrap_or(pattern)
    }

    #[test]
    fn arc_random_graphs_against_model() {
        for seed in 0..seeds(150) {
            random_graph(seed, true, false, 400);
        }
    }

    /// Com a rodada de ciclos em toda sincronização (a *trial deletion*
    /// também nas menores, além do ponto fixo da completa).
    #[test]
    fn arc_random_graphs_with_cycles_always() {
        for seed in 0..seeds(150) {
            random_graph(seed.wrapping_add(1 << 32), true, true, 400);
        }
    }

    /// O mesmo oráculo contra o rastreamento: valida o modelo.
    #[test]
    fn arc_random_graphs_model_holds_under_tracing() {
        for seed in 0..seeds(60) {
            random_graph(seed, false, false, 400);
        }
    }
}

#[cfg(test)]
mod tests_observation_without_reset {
    use super::*;
    #[test]
    fn diagnostic_tells_free_block_with_stale_header() {
        for arc in [false, true] {
            let mut heap = Heap::new(false);
            heap.objects.clear_dead = false;
            if arc { heap.enable_arc(); }
            let first = heap.box_int(i64::MAX);
            heap.retain_code_owner(first);
            let neighbor = heap.box_int(i64::MAX - 1);
            heap.retain_code_owner(neighbor);
            heap.release_code_owner(first);
            heap.collect();
            assert_eq!(heap.observe_code_owner(first), 0, "ARC={arc}");
            assert_eq!(heap.observe_code_owner(neighbor), 3);
            heap.release_code_owner(neighbor);
            heap.collect();
        }
    }
}
