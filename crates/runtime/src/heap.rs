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
    /// `Smi`, cada um seria uma alocação. `allocations + caixas_evitadas` é a
    /// contagem de antes do R10, exata.
    pub caixas_evitadas: u64,
}

// `Texto`, `TextoMut`, `IterUnidades` e `empurrar_ponto` moram em `textos.rs` (P1,
// docs/NATIVO-ESPACO-UNIFICADO.md §4.3).
pub use crate::textos::{IterUnidades, Texto, TextoMut, empurrar_ponto};

/// Um campo de objeto: os bits e se são referência (o que o coletor segue).
pub type Campo = (i64, bool);

// O contrato de layout (docs/NATIVO-ESPACO-UNIFICADO.md §3.1) mora em
// `crate::layout`; o `heap` o reexporta para quem já o usava por aqui.
pub use crate::layout::{
    CABECA_DA_PAGINA, CAMPOS_EM_LINHA, Cabecalho, DESLOCAMENTO_DO_HANDLE, MAIOR_CLASSE, PAGINA, Ref, TLAB_N,
    capacidade, e_objeto, palavras_do_mapa, smi,
};

// O cabeçalho ([`Cabecalho`], `crate::layout`) de um objeto do usuário no
// espaço de objetos ([`EspacoDeObjetos`]) é seguido dos campos (8 bytes cada,
// os bits) e, com mais de 32 campos, das palavras que estendem o mapa de
// referências. Layout C fixo: contrato com o emissor (`llvm/mod.rs`).
//
// O handle do objeto é o endereço do cabeçalho com o bit 1 ligado (`bloco +
// 2`, [`DESLOCAMENTO_DO_HANDLE`]); os blocos são alinhados a 8 e o `Smi` é
// ímpar (R10), então `h & 7 == 2` ([`e_objeto`]) distingue o objeto sem
// consulta.
//
// Como na VM, o campo não carrega a marca de referência: o **mapa** (`mapa`,
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

/// Estados de um bloco ([`Cabecalho::estado`]; `crate::layout::estado`). O
/// coletor é geracional sem mover objetos (marcas "pegajosas"): um objeto nasce
/// [`JOVEM`] (a alocação em linha grava 1), e o que sobrevive a uma coleta vira
/// [`VELHO`]. A coleta menor só marca e varre os jovens; os velhos contam como
/// vivos, e as referências de velho para jovem são achadas pelos [`LEMBRADO`]s
/// — a barreira de escrita (`Heap::get_mut` e as gravações de campo do
/// runtime, e a do código gerado depois de gravar um `Ref` num campo,
/// `llvm/mod.rs`) marca o velho que recebe uma referência, como o *store
/// buffer* da VM. [`PERMANENTE`] é o objeto estático, fora do heap.
pub const LIVRE: u8 = crate::layout::estado::LIVRE;
pub const JOVEM: u8 = crate::layout::estado::JOVEM;
pub const MARCADO: u8 = 2;
pub const VELHO: u8 = crate::layout::estado::VELHO;
pub const LEMBRADO: u8 = crate::layout::estado::LEMBRADO;
pub const PERMANENTE: u8 = crate::layout::estado::PERMANENTE;
/// Bit de [`Cabecalho::flags`]: os campos moram num corpo de fora.
pub const FORA: u8 = crate::layout::flags::FORA;

// [`PAGINA`] é o tamanho da página do espaço de objetos (e o alinhamento dela:
// a página de um handle é `h & !(PAGINA - 1)`). [`CABECA_DA_PAGINA`] são os
// bytes do começo de cada página com o mapa de marcas: um bit por palavra de 8
// bytes da página (o bit do bloco é o da palavra onde ele começa). A marcação
// acende o bit do objeto alcançado; as varreduras leem só os bits, sem tocar os
// blocos, vivos ou mortos (a VM também marca fora do objeto no *old space*,
// `marking.cc`, pelo bit no cabeçalho; aqui o mapa fica junto, por página,
// como o do Immix).

// O espaço de objetos (páginas, classes de tamanho, regiões grandes, mapa de
// marcas, cartões, anexos) mora em `crate::espaco`. [`MAIOR_CLASSE`] e
// [`TLAB_N`] contam palavras do corpo (docs/NATIVO-ESPACO-UNIFICADO.md §2.6): a
// TLAB de `w` palavras é `Contexto::tlab[w]`, `w` em `1..=TLAB_N`.
pub use crate::espaco::{EspacoDeObjetos, tamanho_do_bloco};
use crate::espaco::{
    TLAB_BLOCOS, bytes_de_alocacao, campos_de, corpo, e_referencia, forma, marcado, marcar_bloco, marcar_referencia,
    novo_corpo_de_fora, refs_do_corpo, soltar_corpo_de_fora,
};
use crate::layout::{bytes_do_bloco, flags as flags_do_bloco};

/// A coleta menor vem a cada `LIMITE_JOVEM` bytes alocados (o tamanho do
/// *new space*; o semiespaço da VM começa menor e cresce até 8 MiB em 64
/// bits — aqui fixo, entre o custo por coleta e o pico de memória; 2 MiB
/// cabem melhor no cache que 4: medido em `objetos_escapam`, menos tempo e
/// menos memória)…
const LIMITE_JOVEM: usize = 2 * 1024 * 1024;
/// Quanto o heap cresce até a próxima coleta completa, em % do que
/// sobreviveu à última: 150 (o dobro, 200, custava ~15 MB a mais de pico em
/// `objetos_escapam` sem ganho de tempo, já que a varredura só lê o mapa
/// de marcas).
const CRESCIMENTO: usize = 150;
/// … ou a cada `CONTAGEM_JOVEM` alocações (valores pequenos do runtime).
const CONTAGEM_JOVEM: usize = 256 * 1024;
/// No `--gc-stress`, uma coleta completa a cada tantas (as demais, menores:
/// é nelas que uma barreira faltando apareceria) …
const MENORES_POR_COMPLETA_NO_ESTRESSE: u64 = 8;
/// … ou, com muito dado vivo, a cada `trabalho / TRABALHO_POR_MENOR_NO_ESTRESSE`
/// (o trabalho de marcação da última completa: objetos e palavras de
/// referência visitados): a completa custa o heap vivo inteiro, e uma a cada 8
/// alocações tornava o estresse quadrático (o programa 99, ~1 s sem estresse,
/// passava de 6 min; uma lista viva de 40 mil elementos custava 100 µs por
/// alocação). Assim o custo das completas por alocação fica em ~64 palavras
/// visitadas; a menor continua antes de toda alocação.
const TRABALHO_POR_MENOR_NO_ESTRESSE: usize = 64;

/// O objeto de ninguém: cabeçalho zerado e campos zerados. A leitura em
/// linha de um campo de algo que não é objeto do espaço lê daqui (o que
/// `dartforge_object_get` devolvia); nunca é gravado — está em memória só
/// de leitura, e gravar nela é erro do compilador que termina o processo.
#[repr(C)]
pub struct ObjetoVazio {
    pub cabecalho: Cabecalho,
    pub campos: [i64; CAMPOS_EM_LINHA],
}
pub static OBJETO_VAZIO: ObjetoVazio = ObjetoVazio {
    cabecalho: Cabecalho { estado: LIVRE, flags: 0, n: 0, class_id: 0, mapa: 0, metadado: 0 },
    campos: [0; CAMPOS_EM_LINHA],
};

/// A vista de um objeto do espaço para o runtime ([`Heap::objeto`]): a
/// classe e os campos, `(bits, é referência)`.
#[derive(Clone, Copy)]
pub struct Obj<'a> {
    corpo: *const Cabecalho,
    /// A classe do objeto.
    pub class_id: i64,
    _heap: std::marker::PhantomData<&'a Heap>,
}

#[allow(unsafe_code)]
impl Obj<'_> {
    /// Quantos campos.
    pub fn len(&self) -> usize {
        // SAFETY: corpo vivo enquanto o empréstimo do heap dura.
        usize::from(unsafe { (*self.corpo).n })
    }
    /// Sem campos?
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// O campo `i`: os bits e se é referência.
    ///
    /// # Panics
    /// Com `i` fora do objeto.
    pub fn campo(&self, i: usize) -> Campo {
        assert!(i < self.len(), "campo {i} fora do objeto de {} campos", self.len());
        // SAFETY: `i < n`; corpo vivo.
        unsafe { (*campos_de(self.corpo.cast_mut()).add(i), e_referencia(self.corpo, i)) }
    }
    /// O campo `i`, se existe.
    pub fn get(&self, i: usize) -> Option<Campo> {
        (i < self.len()).then(|| self.campo(i))
    }
    /// O primeiro campo, se existe.
    pub fn first(&self) -> Option<Campo> {
        self.get(0)
    }
    /// Os campos, em ordem.
    pub fn iter(&self) -> impl Iterator<Item = Campo> + '_ {
        (0..self.len()).map(|i| self.campo(i))
    }
    /// Os campos copiados.
    pub fn to_vec(&self) -> Vec<Campo> {
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
fn marcado_na_coleta(objetos: &EspacoDeObjetos, menor: bool, h: i64) -> bool {
    if e_objeto(h) {
        // SAFETY: bloco de uma página do espaço.
        #[allow(unsafe_code)]
        return match objetos.bloco_de(h) {
            Some(b) => unsafe {
                if menor {
                    let e = (*b).estado;
                    e == VELHO || e == LEMBRADO
                } else {
                    marcado(b)
                }
            },
            // O estático (seção da imagem) nunca morre.
            None => e_estatico(h),
        };
    }
    false
}

// `e_objeto` e o módulo `smi` (a etiqueta do `Ref`, R10) moram em
// `crate::layout` e são reexportados no começo deste módulo.

/// Um finalizador nativo e o dado dele (`Dart_NewFinalizableHandle`).
pub type Finalizador = (fn(usize), usize);

/// O quadro de raízes de uma função gerada (G1), no stack dela: o
/// anterior, o número de slots e os slots, que o código gerado grava com um
/// `store` comum (a pilha-sombra do LLVM, `ShadowStackGC`). O runtime só
/// encadeia e desencadeia o quadro ([`empilhar_quadro`]) e o percorre na
/// coleta; os quadros do próprio runtime continuam em `Heap::frames`.
#[repr(C)]
pub struct QuadroDeRaizes {
    anterior: *const QuadroDeRaizes,
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
pub struct Contexto {
    /// 1 com exceção pendente (deslocamento 0).
    pub pendente: std::cell::Cell<u8>,
    /// O quadro do topo da pilha-sombra (deslocamento 8).
    pub topo: std::cell::Cell<*const QuadroDeRaizes>,
    /// A área de globais de cada módulo neste isolado, pelo id do módulo
    /// (`@df.area_id`; 0 = sem id): o caminho rápido do prólogo das
    /// funções (`@df.obter_area`). Endereço dos elementos (deslocamento 16)
    /// e comprimento (24); uma entrada nula vai ao runtime.
    pub areas: std::cell::Cell<*const *mut i64>,
    pub n_areas: std::cell::Cell<usize>,
    /// O pedido de interrupção do isolado (deslocamento 32): a porta de
    /// controle o liga, de outra thread, quando chega uma mensagem de controle
    /// (`portas.rs`). O código gerado o lê, atômico, no começo de cada volta
    /// de laço (o ponto seguro da J01).
    pub interrupcao: std::sync::atomic::AtomicU8,
    /// O cabeçalho de [`OBJETO_VAZIO`] (deslocamento 40): a leitura em
    /// linha de um campo de algo que não é objeto do espaço usa este objeto
    /// no lugar do cabeçalho em `h - 2` (`llvm/mod.rs`), e lê zeros, como
    /// `dartforge_object_get` dava.
    pub vazios: *const Cabecalho,
    /// As classes com tabela de métodos registrada (deslocamento 48; um
    /// byte 0/1 por id, `seletores.rs`) e quantas (56): a alocação em linha
    /// de uma classe que registra a tabela na primeira alocação
    /// (`dartforge_object_new_t`) só pula o runtime depois do registro.
    pub registradas: std::cell::Cell<*const u8>,
    pub n_registradas: std::cell::Cell<usize>,
    /// A TLAB do isolado (deslocamento 64): para cada número de campos
    /// `n ≤ TLAB_N`, uma faixa `[cursor, fim)` de blocos livres contíguos do
    /// espaço de objetos já contados como alocação
    /// (`Heap::reabastecer_tlab`): o cursor em `64 + 16n`, o fim em
    /// `72 + 16n`. O código gerado aloca avançando o cursor um bloco (o
    /// *bump pointer* da VM, `llvm/mod.rs`); faixa esgotada vai ao runtime,
    /// que reabastece.
    pub tlab: [[std::cell::Cell<*mut u8>; 2]; TLAB_N + 1],
    /// Os mapas de bits de subtipo (`seletores.rs`, [`crate::nucleo`]): para
    /// cada classe alvo `C` (índice; deslocamento 336), o endereço do mapa
    /// em que o bit `cid` diz se `cid <: C` (nulo = ainda não calculado), e
    /// quantos alvos (344); a largura de todos os mapas em bits (352): um
    /// `cid` fora dela vai ao runtime (`dartforge_is_subclass`, que também
    /// monta o mapa na primeira consulta). Um registro de subclasse novo
    /// (a carga, uma recarga) zera os mapas.
    pub subtipos: std::cell::Cell<*const *const u64>,
    pub n_subtipos: std::cell::Cell<usize>,
    pub largura_subtipos: std::cell::Cell<usize>,
    /// Os handles (endereço + 2) das caixas estáticas de `true` (deslocamento
    /// 360) e `false` (368) deste runtime: `@df.caixa_bool` e
    /// `@df.desencaixa_bool` os leem daqui, sem referenciar os dados
    /// `dartforge_verdadeiro`/`_falso` de outra imagem (o executável e a DLL do
    /// SDK têm cada um a sua cópia; só a do runtime ativo é a canônica).
    pub verdadeiro: *const u8,
    pub falso: *const u8,
    /// O limite da pilha nativa da thread (deslocamento 376): o prólogo de
    /// cada função que chama outra compara o endereço do seu quadro com ele
    /// e, abaixo, lança `StackOverflowError` em vez de estourar a pilha do
    /// sistema (o `stack_overflow_check` da VM). 0 = ainda não calculado
    /// ([`crate::gc_raizes`], `dartforge_contexto`); 1 = sem limite conhecido.
    pub limite_da_pilha: std::cell::Cell<usize>,
}

/// O `is C` lido em linha pelo código gerado ([`Contexto::subtipos`]). Para
/// medir, `DARTFORGE_SEM_CLASSE_EM_LINHA=1` não publica os mapas: o código
/// gerado vai sempre ao runtime (a resposta é a mesma).
pub fn classe_em_linha() -> bool {
    static SIM: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *SIM.get_or_init(|| std::env::var_os("DARTFORGE_SEM_CLASSE_EM_LINHA").is_none())
}

const _: () = {
    use crate::layout::contexto as c;
    assert!(std::mem::offset_of!(Contexto, pendente) == c::PENDENTE);
    assert!(std::mem::offset_of!(Contexto, topo) == c::TOPO);
    assert!(std::mem::offset_of!(Contexto, areas) == c::AREAS);
    assert!(std::mem::offset_of!(Contexto, n_areas) == c::N_AREAS);
    assert!(std::mem::offset_of!(Contexto, interrupcao) == c::INTERRUPCAO);
    assert!(std::mem::offset_of!(Contexto, vazios) == c::VAZIOS);
    assert!(std::mem::offset_of!(Contexto, registradas) == c::REGISTRADAS);
    assert!(std::mem::offset_of!(Contexto, n_registradas) == c::N_REGISTRADAS);
    assert!(std::mem::offset_of!(Contexto, tlab) == c::TLAB);
    assert!(std::mem::offset_of!(Contexto, subtipos) == c::SUBTIPOS);
    assert!(std::mem::offset_of!(Contexto, n_subtipos) == c::N_SUBTIPOS);
    assert!(std::mem::offset_of!(Contexto, largura_subtipos) == c::LARGURA_SUBTIPOS);
    assert!(std::mem::offset_of!(Contexto, verdadeiro) == c::VERDADEIRO);
    assert!(std::mem::offset_of!(Contexto, falso) == c::FALSO);
    assert!(std::mem::offset_of!(Contexto, limite_da_pilha) == c::LIMITE_DA_PILHA);
};

thread_local! {
    pub static CONTEXTO: Contexto = const {
        Contexto {
            pendente: std::cell::Cell::new(0),
            topo: std::cell::Cell::new(std::ptr::null()),
            areas: std::cell::Cell::new(std::ptr::null()),
            n_areas: std::cell::Cell::new(0),
            interrupcao: std::sync::atomic::AtomicU8::new(0),
            vazios: &raw const OBJETO_VAZIO.cabecalho,
            registradas: std::cell::Cell::new(std::ptr::null()),
            n_registradas: std::cell::Cell::new(0),
            tlab: [const { [std::cell::Cell::new(std::ptr::null_mut()), std::cell::Cell::new(std::ptr::null_mut())] }; TLAB_N + 1],
            subtipos: std::cell::Cell::new(std::ptr::null()),
            n_subtipos: std::cell::Cell::new(0),
            largura_subtipos: std::cell::Cell::new(0),
            verdadeiro: (&raw const dartforge_verdadeiro).cast::<u8>().wrapping_add(DESLOCAMENTO_DO_HANDLE as usize),
            falso: (&raw const dartforge_falso).cast::<u8>().wrapping_add(DESLOCAMENTO_DO_HANDLE as usize),
            limite_da_pilha: std::cell::Cell::new(0),
        }
    };
}


/// Encadeia `q` no topo da pilha-sombra. `q.n` e os slots (zerados) já
/// foram escritos pelo código gerado.
///
/// # Safety
/// `q` aponta um quadro válido que vive até o [`desempilhar_quadro`]
/// correspondente.
#[allow(unsafe_code)]
pub unsafe fn empilhar_quadro(q: *mut QuadroDeRaizes) {
    CONTEXTO.with(|c| {
        // SAFETY: `q` é o quadro no stack da função que chama, vivo até o
        // `desempilhar_quadro` antes de cada retorno dela.
        unsafe { (*q).anterior = c.topo.get() };
        c.topo.set(q);
    });
}

/// Desencadeia `q`, que tem de ser o topo (os retornos fecham os quadros
/// em ordem, G3).
///
/// # Safety
/// `q` é o quadro válido passado ao último [`empilhar_quadro`].
#[allow(unsafe_code)]
pub unsafe fn desempilhar_quadro(q: *const QuadroDeRaizes) {
    CONTEXTO.with(|c| {
        assert!(std::ptr::eq(c.topo.get(), q), "bug do compilador: quadro de raízes fechado fora de ordem");
        // SAFETY: `q` é o topo, ainda no stack de quem chama.
        c.topo.set(unsafe { (*q).anterior });
    });
}

/// O campo `n` de um quadro de raízes: o número de slots nos 40 bits de
/// baixo; nos de cima, quantos dos últimos são de conferência (o build de
/// conferência do percurso por mapas, `llvm/mod.rs`).
const SLOTS_DO_QUADRO: u64 = (1 << 40) - 1;

/// O bit 62 do campo `n`: o quadro está num quadro nativo próprio (o
/// programa sem otimização, sem inlining; `com_quadros_ordenados` do
/// emissor). Só entre dois quadros assim a ordem dos endereços é garantida.
const QUADRO_ORDENADO: u64 = 1 << 62;

/// Visita as raízes de todos os quadros da pilha-sombra desta thread.
///
/// Os quadros são `alloca` do código gerado na pilha nativa da thread, que
/// cresce para baixo: cada quadro anterior está num endereço maior. Um
/// anterior abaixo do atual é um quadro morto, de uma função que já saiu —
/// o topo que um desenrolamento deixou de restaurar (o pouso de
/// `--excecoes=tabelas` sem o `store` do topo, a sabotagem `pouso_sem_topo`
/// do §14.7) e que um quadro novo encadeou. Ler os slots dele seria ler
/// lixo; o processo encerra com o código 3. A conferência só vale entre dois
/// quadros com [`QUADRO_ORDENADO`]: com inlining (o código otimizado, o SDK),
/// o quadro da função embutida e o da que a chamou são `alloca` do mesmo
/// quadro nativo, em ordem de endereço qualquer.
#[allow(unsafe_code)]
fn visitar_quadros(mut f: impl FnMut(i64)) {
    let mut q = CONTEXTO.with(|c| c.topo.get());
    while !q.is_null() {
        // SAFETY: cada quadro encadeado está no stack de uma função ainda
        // ativa desta thread, com `n` slots depois do cabeçalho (a ordem dos
        // endereços, conferida abaixo antes de seguir, o garante).
        unsafe {
            let n = ((*q).n as u64 & SLOTS_DO_QUADRO) as usize;
            let slots = std::ptr::addr_of!((*q).slots) as *const i64;
            for i in 0..n {
                f(*slots.add(i));
            }
            let anterior = (*q).anterior;
            if !anterior.is_null()
                && (anterior as usize) <= (q as usize)
                && (*q).n as u64 & QUADRO_ORDENADO != 0
                && (*anterior).n as u64 & QUADRO_ORDENADO != 0
            {
                eprintln!("dartforge: quadro de raízes morto na pilha-sombra");
                std::process::exit(3);
            }
            q = anterior;
        }
    }
}

// ---------------------------------------------------------------------------
// Raízes por mapas de pilha (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §14.8).
//
// Com `--raizes=mapas` os valores vivos através de uma chamada não vão para a
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
const PRESERVADOS: u32 = ((1 << 11) - 1) << 19;
#[cfg(all(not(target_arch = "aarch64"), windows))]
const PRESERVADOS: u32 = (1 << 3) | (1 << 4) | (1 << 5) | (1 << 6) | (0xF << 12);
#[cfg(all(not(target_arch = "aarch64"), not(windows)))]
const PRESERVADOS: u32 = (1 << 3) | (1 << 6) | (0xF << 12);

/// O tipo de uma raiz do mapa: slot sobre o SP, slot sobre o FP, ou
/// registrador (o valor é o número DWARF dele).
const RAIZ_NO_SP: u8 = 0;
const RAIZ_NO_FP: u8 = 1;
const RAIZ_EM_REGISTRADOR: u8 = 2;

/// Uma função com mapa de pilha numa imagem: o endereço do começo dela e
/// onde está a descrição, que só é decodificada na primeira consulta
/// (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §3.5 e §14.5, E3.2).
#[derive(Clone, Copy)]
struct FuncaoComMapa {
    inicio: u64,
    /// DFGM: o começo do fluxo da função; `.llvm_stackmaps`: o primeiro
    /// registro dela.
    dados: usize,
    /// `.llvm_stackmaps`: quantos registros e o vetor de constantes do blob
    /// (os `ConstIndex`); DFGM: `u32::MAX` e a versão do blob (1: só slots;
    /// 2: com a máscara dos registradores).
    n_llvm: u32,
    constantes: usize,
}

/// Os registros de uma função, decodificados: (deslocamento do endereço de
/// retorno em relação ao começo da função, início em `slots`, número), em
/// ordem do deslocamento; e as raízes, (tipo, deslocamento em bytes do slot
/// ou número DWARF do registrador): [`RAIZ_NO_SP`], [`RAIZ_NO_FP`],
/// [`RAIZ_EM_REGISTRADOR`].
#[derive(Default)]
struct RegistrosDaFuncao {
    registros: Vec<(u32, u32, u32)>,
    slots: Vec<(u8, i32)>,
}

/// Os mapas de uma imagem (o executável, a DLL do SDK).
struct MapaDePilha {
    /// O que identifica a imagem no registro: a base no Windows, o começo
    /// da seção no ELF e no Mach-O.
    chave: usize,
    /// Ordenadas pelo começo.
    funcoes: Vec<FuncaoComMapa>,
    decodificadas: Vec<std::sync::OnceLock<RegistrosDaFuncao>>,
}

// SAFETY: os endereços guardados são de seções só de leitura da imagem,
// mapeadas enquanto o processo vive; nada é escrito por eles.
#[allow(unsafe_code)]
unsafe impl Send for MapaDePilha {}
#[allow(unsafe_code)]
unsafe impl Sync for MapaDePilha {}

impl MapaDePilha {
    fn novo(chave: usize) -> Self {
        MapaDePilha { chave, funcoes: Vec::new(), decodificadas: Vec::new() }
    }

    /// Ordena o índice e prepara as decodificações preguiçosas. Duas
    /// entradas com o mesmo começo (o ICF do ligador juntou duas funções
    /// idênticas, de mapas idênticos) ficam numa só.
    fn fechar(&mut self) {
        self.funcoes.sort_by_key(|f| f.inicio);
        self.funcoes.dedup_by_key(|f| f.inicio);
        self.decodificadas = (0..self.funcoes.len()).map(|_| std::sync::OnceLock::new()).collect();
    }

    /// A função com o maior começo que não passa de `retorno`.
    fn funcao_de(&self, retorno: u64) -> Option<usize> {
        self.funcoes.partition_point(|f| f.inicio <= retorno).checked_sub(1)
    }

    #[allow(unsafe_code)]
    fn registros(&self, i: usize) -> &RegistrosDaFuncao {
        // SAFETY: `funcoes[i]` aponta para a descrição de uma função numa
        // seção mapeada (conferida no registro da imagem).
        self.decodificadas[i].get_or_init(|| unsafe { decodificar_funcao(self.funcoes[i]) })
    }

    /// Os slots da chamada da função `i` que retorna em `retorno`.
    ///
    /// No Windows x64, em parte das chamadas, o gerador de código põe um
    /// `nop` depois do `call` (para o desenrolador do sistema) e o registro
    /// aponta depois dele (§14.5): o deslocamento seguinte também vale, só
    /// quando o byte no endereço de retorno é esse `nop`.
    #[allow(unsafe_code)]
    fn slots_de(&self, i: usize, retorno: u64) -> Option<&[(u8, i32)]> {
        let r = self.registros(i);
        let deslocamento = u32::try_from(retorno.checked_sub(self.funcoes[i].inicio)?).ok()?;
        let achar = |d: u32| {
            r.registros.binary_search_by_key(&d, |g| g.0).ok().map(|k| {
                let (_, ini, n) = r.registros[k];
                &r.slots[ini as usize..(ini + n) as usize]
            })
        };
        achar(deslocamento).or_else(|| {
            // SAFETY: `retorno` é um endereço de retorno da pilha desta
            // thread, um byte de código mapeado.
            let nop = cfg!(all(windows, target_arch = "x86_64")) && !sabotagem("sem_nop") && unsafe { *(retorno as *const u8) } == 0x90;
            if nop { achar(deslocamento + 1) } else { None }
        })
    }
}

static MAPAS_DE_PILHA: std::sync::RwLock<Vec<MapaDePilha>> = std::sync::RwLock::new(Vec::new());
/// Alguma imagem registrou mapas: a coleta percorre a pilha nativa.
static HA_MAPAS_DE_PILHA: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Os números do percurso por mapas, do processo inteiro (§7.5, a prova de
/// que coletou): quadros nativos percorridos, raízes lidas de mapas e
/// raízes conferidas contra a pilha-sombra (`DARTFORGE_GC_PERCURSO`).
static QUADROS_PERCORRIDOS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static RAIZES_DE_MAPA: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static RAIZES_CONFERIDAS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// (quadros percorridos, raízes de mapa visitadas, raízes conferidas).
pub fn numeros_do_percurso() -> (u64, u64, u64) {
    use std::sync::atomic::Ordering::Relaxed;
    (QUADROS_PERCORRIDOS.load(Relaxed), RAIZES_DE_MAPA.load(Relaxed), RAIZES_CONFERIDAS.load(Relaxed))
}

/// Alguma imagem do processo registrou mapas de pilha.
pub fn ha_mapas_de_pilha() -> bool {
    HA_MAPAS_DE_PILHA.load(std::sync::atomic::Ordering::Acquire)
}

/// Uma sabotagem de teste ligada (`DARTFORGE_SABOTAGEM=a,b,…`,
/// docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.3 e §14.7): cada teste
/// dirigido tem de falhar com a dele. As do runtime: `sem_nop` (o leitor
/// sem a regra do `nop`), `folga` (D8: a folga da pilha de 4 KiB) e
/// `sem_porta` (D9: as portas Rust → Dart não são registradas) e
/// `tipo_sem_raiz` (D12: o objeto `Type` canônico fica só no cache).
pub fn sabotagem(nome: &str) -> bool {
    static LIGADAS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    LIGADAS
        .get_or_init(|| std::env::var("DARTFORGE_SABOTAGEM").unwrap_or_default().split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
        .iter()
        .any(|s| s == nome)
}

/// `DARTFORGE_GC_PERCURSO=conferir` (§3.6, E2.5): cada coleta confere que
/// o percurso por mapas visitou as raízes que o build de conferência
/// (`DARTFORGE_RAIZES_CONFERIR=1` na compilação) gravou na pilha-sombra, e
/// uma anomalia do percurso aborta em vez de só encerrá-lo.
fn percurso_conferido() -> bool {
    static SIM: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *SIM.get_or_init(|| std::env::var("DARTFORGE_GC_PERCURSO").is_ok_and(|v| v == "conferir"))
}

/// Encerra o processo: um mapa que o runtime não sabe ler é uma raiz
/// perdida, e continuar seria liberar objeto vivo.
fn mapa_recusado(motivo: &str) -> ! {
    eprintln!("dartforge: mapa de pilha recusado: {motivo}");
    std::process::abort()
}

/// Registra os mapas de pilha da imagem PE carregada em `base` (uma vez por
/// imagem). O módulo que tem funções com raízes no mapa chama isto na
/// partida, com o `__ImageBase` da imagem em que foi ligado.
#[allow(unsafe_code)]
pub fn registrar_mapa_de_pilha(base: usize) {
    if base == 0 {
        return;
    }
    let mut mapas = MAPAS_DE_PILHA.write().unwrap_or_else(|e| e.into_inner());
    if mapas.iter().any(|m| m.chave == base) {
        return;
    }
    // SAFETY: `base` é o começo de uma imagem PE carregada neste processo.
    let mapa = unsafe { ler_mapa_da_imagem(base) };
    if mapa.funcoes.is_empty() {
        mapa_recusado("a imagem foi compilada com raízes por mapas e não tem a seção `.dfgcm` nem a `.llvm_st` (o ligador renomeia a seção quando há informação de depuração)");
    }
    HA_MAPAS_DE_PILHA.store(true, std::sync::atomic::Ordering::Release);
    mapas.push(mapa);
}

/// Registra o mapa compacto de uma imagem ELF ou Mach-O: a seção `dfgcm`
/// (`__DATA_CONST,__dfgcm` no Mach-O) entre `inicio` e `fim`, que o
/// ligador delimita (`__start_dfgcm`/`__stop_dfgcm`; `section$start$…`).
#[allow(unsafe_code)]
pub fn registrar_secao_de_mapa(inicio: usize, fim: usize) {
    if inicio == 0 || fim <= inicio {
        return;
    }
    let mut mapas = MAPAS_DE_PILHA.write().unwrap_or_else(|e| e.into_inner());
    if mapas.iter().any(|m| m.chave == inicio) {
        return;
    }
    let mut mapa = MapaDePilha::novo(inicio);
    // SAFETY: `[inicio, fim)` é a seção do mapa, mapeada e legível.
    unsafe { ler_dfgm(inicio as *const u8, fim - inicio, 0, &mut mapa) };
    mapa.fechar();
    if mapa.funcoes.is_empty() {
        mapa_recusado("a seção do mapa de pilha compacto está vazia numa imagem compilada com raízes por mapas");
    }
    HA_MAPAS_DE_PILHA.store(true, std::sync::atomic::Ordering::Release);
    mapas.push(mapa);
}

/// Registra o mapa de pilha no formato do LLVM (`.llvm_stackmaps` versão 3)
/// de uma imagem Mach-O: a seção `__LLVM_STACKMAPS,__llvm_stackmaps` entre
/// `inicio` e `fim` (`section$start$…`/`section$end$…`), com os endereços
/// das funções já ajustados pelo carregador. Seção vazia não é erro aqui: o
/// módulo que registra pode não ter função com mapa, e o `.no_dead_strip`
/// que guarda a seção vai em todo módulo que tem (`llvm/mod.rs`).
#[allow(unsafe_code)]
pub fn registrar_secao_llvm(inicio: usize, fim: usize) {
    if inicio == 0 || fim <= inicio {
        return;
    }
    let mut mapas = MAPAS_DE_PILHA.write().unwrap_or_else(|e| e.into_inner());
    if mapas.iter().any(|m| m.chave == inicio) {
        return;
    }
    let mut mapa = MapaDePilha::novo(inicio);
    // SAFETY: `[inicio, fim)` é a seção do mapa, mapeada e legível.
    unsafe { ler_stackmaps(inicio as *const u8, fim - inicio, &mut mapa) };
    mapa.fechar();
    if mapa.funcoes.is_empty() {
        return;
    }
    HA_MAPAS_DE_PILHA.store(true, std::sync::atomic::Ordering::Release);
    mapas.push(mapa);
}

/// Tira do índice o mapa registrado com a chave `chave` (o começo da seção):
/// o JIT o chama quando solta a memória de um objeto (uma geração aposentada
/// da recarga, docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §3.8). A
/// publicação de uma geração acontece sem quadro Dart na pilha, então nenhum
/// quadro em curso usa o mapa que sai.
pub fn desregistrar_mapa(chave: usize) {
    let mut mapas = MAPAS_DE_PILHA.write().unwrap_or_else(|e| e.into_inner());
    mapas.retain(|m| m.chave != chave);
}

/// Lê os mapas de pilha da imagem PE em `base`: a seção `.dfgcm` (o mapa
/// compacto, `emit_native/src/gcmap.rs`) e a `.llvm_st` (o `.llvm_stackmaps`
/// dos objetos que não passaram pelo conversor — os da LTO —, com o nome
/// cortado em 8 bytes).
///
/// # Safety
/// `base` é o começo de uma imagem PE mapeada.
#[allow(unsafe_code)]
unsafe fn ler_mapa_da_imagem(base: usize) -> MapaDePilha {
    let mut mapa = MapaDePilha::novo(base);
    // SAFETY: os cabeçalhos de uma imagem PE mapeada (DOS, NT, seções).
    unsafe {
        let b = base as *const u8;
        let nt = b.add(b.add(0x3c).cast::<u32>().read_unaligned() as usize);
        let n_secoes = nt.add(6).cast::<u16>().read_unaligned() as usize;
        let opcional = nt.add(20).cast::<u16>().read_unaligned() as usize;
        let secoes = nt.add(24 + opcional);
        for i in 0..n_secoes {
            let s = secoes.add(40 * i);
            let nome = std::slice::from_raw_parts(s, 8);
            let tamanho = s.add(8).cast::<u32>().read_unaligned() as usize;
            let rva = s.add(12).cast::<u32>().read_unaligned() as usize;
            if nome == b".llvm_st" {
                ler_stackmaps(b.add(rva), tamanho, &mut mapa);
            } else if nome == b".dfgcm\0\0" {
                ler_dfgm(b.add(rva), tamanho, base, &mut mapa);
            }
        }
    }
    mapa.fechar();
    mapa
}

/// O tamanho de um registro do `.llvm_stackmaps` v3 que começa em `r`
/// (cabeçalho, locais, enchimento, *live-outs*, enchimento).
///
/// # Safety
/// `r` é o começo de um registro dentro de uma seção mapeada.
#[allow(unsafe_code)]
unsafe fn fim_do_registro_llvm(r: *const u8) -> *const u8 {
    let alinhar = |q: *const u8| ((q as usize + 7) & !7) as *const u8;
    // SAFETY: o formato do registro (§14.3).
    unsafe {
        let n_locais = r.add(14).cast::<u16>().read_unaligned() as usize;
        let q = alinhar(r.add(16 + 12 * n_locais));
        let n_saidas = q.add(2).cast::<u16>().read_unaligned() as usize;
        alinhar(q.add(4 + 4 * n_saidas))
    }
}

/// Indexa os blobs `.llvm_stackmaps` versão 3 de `[p, p + tamanho)` (um
/// por objeto ligado, com zeros de enchimento entre eles): por função, o
/// começo e o primeiro registro. Os registros só são lidos na consulta.
///
/// # Safety
/// `[p, p + tamanho)` é a seção, mapeada e legível.
#[allow(unsafe_code)]
unsafe fn ler_stackmaps(mut p: *const u8, tamanho: usize, mapa: &mut MapaDePilha) {
    // SAFETY: todas as leituras ficam dentro da seção, pelo formato.
    unsafe {
        let fim = p.add(tamanho);
        while (p as usize) + 16 <= fim as usize {
            if *p == 0 {
                p = p.add(1);
                continue;
            }
            if *p != 3 {
                mapa_recusado("versão do `.llvm_stackmaps` diferente de 3");
            }
            let n_funcoes = p.add(4).cast::<u32>().read_unaligned() as usize;
            let n_constantes = p.add(8).cast::<u32>().read_unaligned() as usize;
            let funcoes = p.add(16);
            let constantes = funcoes.add(24 * n_funcoes);
            let mut r = constantes.add(8 * n_constantes);
            for i in 0..n_funcoes {
                let endereco = funcoes.add(24 * i).cast::<u64>().read_unaligned();
                let n_registros = funcoes.add(24 * i + 16).cast::<u64>().read_unaligned();
                if n_registros > u64::from(u32::MAX - 1) {
                    mapa_recusado("função do `.llvm_stackmaps` com registros demais");
                }
                mapa.funcoes.push(FuncaoComMapa { inicio: endereco, dados: r as usize, n_llvm: n_registros as u32, constantes: constantes as usize });
                for _ in 0..n_registros {
                    r = fim_do_registro_llvm(r);
                }
                if r as usize > fim as usize {
                    mapa_recusado("`.llvm_stackmaps` truncado");
                }
            }
            p = r;
        }
    }
}

/// Indexa os blobs DFGM v1 e v2 de `[p, p + tamanho)` (o formato está em
/// `emit_native/src/gcmap.rs`; entre as contribuições dos objetos há zeros
/// de enchimento, em múltiplos de 4). O endereço de cada função vem do
/// índice conforme as bandeiras do blob: 0, deslocamento sobre `base` (o
/// `ADDR32NB` do COFF); 1, deslocamento de 32 bits sobre o próprio campo
/// (ELF); 2, endereço absoluto de 64 bits (Mach-O).
///
/// # Safety
/// `[p, p + tamanho)` é a seção, mapeada e legível.
#[allow(unsafe_code)]
unsafe fn ler_dfgm(mut p: *const u8, tamanho: usize, base: usize, mapa: &mut MapaDePilha) {
    // SAFETY: todas as leituras ficam dentro da seção, pelo formato.
    unsafe {
        let fim = p.add(tamanho);
        while (p as usize) + 16 <= fim as usize {
            if std::slice::from_raw_parts(p, 4) != b"DFGM" {
                p = p.add(4);
                continue;
            }
            let versao = *p.add(4);
            if versao != 1 && versao != 2 {
                mapa_recusado("versão do mapa compacto diferente de 1 e de 2");
            }
            let bandeiras = p.add(6).cast::<u16>().read_unaligned();
            let total = p.add(8).cast::<u32>().read_unaligned() as usize;
            let n_funcoes = p.add(12).cast::<u32>().read_unaligned() as usize;
            let entrada = if bandeiras & 2 != 0 { 16 } else { 8 };
            if total < 16 + entrada * n_funcoes || (p as usize) + total > fim as usize || (bandeiras & 3 == 0 && base == 0) {
                mapa_recusado("mapa compacto com o cabeçalho errado");
            }
            let indice = p.add(16);
            let fluxo = indice.add(entrada * n_funcoes);
            for i in 0..n_funcoes {
                let e = indice.add(entrada * i);
                let (inicio_da_funcao, desloc_no_fluxo) = if bandeiras & 2 != 0 {
                    (e.cast::<u64>().read_unaligned(), e.add(8).cast::<u32>().read_unaligned())
                } else if bandeiras & 1 != 0 {
                    let rel = i64::from(e.cast::<i32>().read_unaligned());
                    ((e as i64).wrapping_add(rel) as u64, e.add(4).cast::<u32>().read_unaligned())
                } else {
                    (base as u64 + u64::from(e.cast::<u32>().read_unaligned()), e.add(4).cast::<u32>().read_unaligned())
                };
                mapa.funcoes.push(FuncaoComMapa {
                    inicio: inicio_da_funcao,
                    dados: fluxo.add(desloc_no_fluxo as usize) as usize,
                    n_llvm: u32::MAX,
                    constantes: usize::from(versao),
                });
            }
            p = p.add(total);
        }
    }
}

/// Decodifica os registros de uma função (a primeira consulta a ela).
///
/// # Safety
/// `f` vem de [`ler_stackmaps`] ou [`ler_dfgm`] sobre uma seção mapeada.
#[allow(unsafe_code)]
unsafe fn decodificar_funcao(f: FuncaoComMapa) -> RegistrosDaFuncao {
    let mut d = RegistrosDaFuncao::default();
    // SAFETY: as leituras seguem o formato a partir de `f.dados`.
    unsafe {
        if f.n_llvm == u32::MAX {
            let mut q = f.dados as *const u8;
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
            let n_registros = varint(&mut q);
            let _quadro = varint(&mut q);
            let mut retorno = 0u64;
            // (início em `slots`, número) do conjunto do registro anterior.
            let mut anterior = (0u32, 0u32);
            for _ in 0..n_registros {
                retorno += varint(&mut q);
                let c = varint(&mut q);
                if c & 1 == 0 {
                    let primeiro = d.slots.len() as u32;
                    let mut slot = 0i64;
                    for _ in 0..(c >> 1) {
                        let s = varint(&mut q);
                        let u = s >> 1;
                        slot += ((u >> 1) as i64) ^ -((u & 1) as i64);
                        d.slots.push((if s & 1 == 1 { RAIZ_NO_FP } else { RAIZ_NO_SP }, (slot * 8) as i32));
                    }
                    // v2: a máscara dos registradores com raiz.
                    if f.constantes >= 2 {
                        let mascara = varint(&mut q);
                        if mascara & !u64::from(PRESERVADOS) != 0 {
                            mapa_recusado("registrador não preservado com raiz no mapa compacto");
                        }
                        for r in 0..32 {
                            if mascara & (1 << r) != 0 {
                                d.slots.push((RAIZ_EM_REGISTRADOR, r));
                            }
                        }
                    }
                    anterior = (primeiro, d.slots.len() as u32 - primeiro);
                }
                if retorno > u64::from(u32::MAX) {
                    mapa_recusado("deslocamento de retorno fora de 32 bits no mapa compacto");
                }
                d.registros.push((retorno as u32, anterior.0, anterior.1));
            }
        } else {
            let mut r = f.dados as *const u8;
            for _ in 0..f.n_llvm {
                let deslocamento = r.add(8).cast::<u32>().read_unaligned();
                let n_locais = r.add(14).cast::<u16>().read_unaligned() as usize;
                let locais = r.add(16);
                if n_locais < 3 {
                    mapa_recusado("registro de statepoint sem os três locais iniciais");
                }
                let n_deopt = locais.add(2 * 12 + 8).cast::<i32>().read_unaligned().max(0) as usize;
                let inicio = d.slots.len();
                if 3 + n_deopt > n_locais {
                    mapa_recusado("registro de statepoint com mais locais de `deopt` que locais");
                }
                // As raízes: cada local do `"deopt"` (a forma do §14.12) e a
                // base de cada par (base, derivado) do `"gc-live"`.
                for j in (3..3 + n_deopt).chain((3 + n_deopt..n_locais.saturating_sub(1)).step_by(2)) {
                    let l = locais.add(12 * j);
                    let tipo = *l;
                    let largura = l.add(2).cast::<u16>().read_unaligned();
                    let registrador = l.add(4).cast::<u16>().read_unaligned();
                    let valor = l.add(8).cast::<i32>().read_unaligned();
                    // Constante: null não é raiz, nem o `undef` que o
                    // otimizador deixou num operando (0xFEFEFEFE); outra
                    // constante par não nula seria um valor bruto tratado
                    // como referência (§3.5).
                    if tipo == 4 || tipo == 5 {
                        let c = if tipo == 4 {
                            i64::from(valor)
                        } else {
                            (f.constantes as *const u8).add(8 * valor.max(0) as usize).cast::<i64>().read_unaligned()
                        };
                        if c != 0 && c & 1 == 0 && c != 0xFEFE_FEFE {
                            mapa_recusado("constante par não nula como raiz no mapa de pilha");
                        }
                        continue;
                    }
                    let slot = if tipo == 1 && largura == 8 && registrador < 32 && PRESERVADOS & (1 << registrador) != 0 {
                        (RAIZ_EM_REGISTRADOR, i32::from(registrador))
                    } else if tipo == 3 && largura == 8 && (registrador == DWARF_SP || registrador == DWARF_FP) {
                        (if registrador == DWARF_FP { RAIZ_NO_FP } else { RAIZ_NO_SP }, valor)
                    } else {
                        mapa_recusado("local que não é `Indirect [SP|FP + d]` nem `Register` preservado de 8 bytes")
                    };
                    if !d.slots[inicio..].contains(&slot) {
                        d.slots.push(slot);
                    }
                }
                d.registros.push((deslocamento, inicio as u32, (d.slots.len() - inicio) as u32));
                r = fim_do_registro_llvm(r);
            }
            d.registros.sort_unstable_by_key(|g| g.0);
        }
    }
    if d.registros.windows(2).any(|par| par[0].0 == par[1].0) {
        mapa_recusado("dois registros do mapa de pilha no mesmo endereço de retorno");
    }
    d
}

/// Visita as raízes do quadro nativo parado no endereço de retorno
/// `retorno`, com o SP do quadro no ponto da chamada em `sp` e o FP em `fp`;
/// `registrador` dá o valor de um registrador preservado (pelo número
/// DWARF) naquele quadro, como o desenrolador o restaurou.
/// `inicio_exato` é o começo da função como o desenrolador o conhece: se
/// ela tem mapa e o endereço não tem registro, é defeito do emissor ou do
/// conversor, e o processo aborta. Devolve se o quadro tinha mapa.
#[allow(unsafe_code)]
fn visitar_quadro(
    mapas: &[MapaDePilha],
    inicio_exato: Option<u64>,
    retorno: u64,
    sp: u64,
    fp: u64,
    registrador: &dyn Fn(u16) -> u64,
    f: &mut dyn FnMut(i64),
) -> bool {
    for m in mapas {
        let Some(i) = m.funcao_de(retorno) else { continue };
        match m.slots_de(i, retorno) {
            Some(slots) => {
                for &(tipo, d) in slots {
                    if tipo == RAIZ_EM_REGISTRADOR {
                        f(registrador(d as u16) as i64);
                        continue;
                    }
                    let b = if tipo == RAIZ_NO_FP { fp } else { sp };
                    // SAFETY: o slot é do quadro de uma função ainda ativa,
                    // parada na chamada que o registro descreve.
                    f(unsafe { *((b as i64).wrapping_add(i64::from(d)) as *const i64) });
                }
                RAIZES_DE_MAPA.fetch_add(slots.len() as u64, std::sync::atomic::Ordering::Relaxed);
                return true;
            }
            None => {
                if inicio_exato == Some(m.funcoes[i].inicio) {
                    mapa_recusado("função com mapa sem registro exato para o endereço de retorno");
                }
            }
        }
    }
    false
}

/// Uma anomalia do percurso (SP que não cresce, fora da pilha): encerra o
/// percurso; com `DARTFORGE_GC_PERCURSO=conferir`, aborta.
fn anomalia_do_percurso(motivo: &str) {
    if percurso_conferido() {
        mapa_recusado(&format!("anomalia no percurso da pilha: {motivo}"));
    }
}

/// Visita as raízes dos quadros nativos desta thread que têm mapa de pilha:
/// desenrola a pilha a partir daqui (`RtlVirtualUnwind`, sem chamar
/// tratadores) e, em cada quadro, lê os slots do registro do endereço de
/// retorno. Sem mapa registrado, não faz nada.
#[cfg(all(windows, target_arch = "x86_64"))]
#[allow(unsafe_code)]
#[inline(never)]
fn visitar_quadros_por_mapas(mut f: impl FnMut(i64)) {
    if !HA_MAPAS_DE_PILHA.load(std::sync::atomic::Ordering::Acquire) {
        return;
    }
    /// O `CONTEXT` do x86-64: 1232 bytes, alinhado a 16. São lidos o `Rip`
    /// (0xF8) e os inteiros de `Rax` (0x78) a `R15` (0xF0).
    #[repr(C, align(16))]
    struct ContextoDaCpu([u8; 1232]);
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn RtlCaptureContext(contexto: *mut ContextoDaCpu);
        fn RtlLookupFunctionEntry(pc: u64, base: *mut u64, historico: *mut u8) -> *const u8;
        fn RtlVirtualUnwind(
            tipo: u32,
            base: u64,
            pc: u64,
            funcao: *const u8,
            contexto: *mut ContextoDaCpu,
            dados: *mut *mut u8,
            quadro: *mut u64,
            ponteiros: *mut u8,
        ) -> *mut u8;
        fn GetCurrentThreadStackLimits(baixo: *mut usize, alto: *mut usize);
    }
    let mapas = MAPAS_DE_PILHA.read().unwrap_or_else(|e| e.into_inner());
    let mut contexto = ContextoDaCpu([0; 1232]);
    let campo = |c: &ContextoDaCpu, d: usize| u64::from_le_bytes(c.0[d..d + 8].try_into().expect("oito bytes"));
    /// O deslocamento no `CONTEXT` de cada registrador DWARF 0–15 (rax,
    /// rdx, rcx, rbx, rsi, rdi, rbp, rsp, r8–r15).
    const NO_CONTEXTO: [usize; 16] = [0x78, 0x88, 0x80, 0x90, 0xA8, 0xB0, 0xA0, 0x98, 0xB8, 0xC0, 0xC8, 0xD0, 0xD8, 0xE0, 0xE8, 0xF0];
    let mut quadros = 0u64;
    // SAFETY: o contexto é o desta thread, capturado aqui; cada passo
    // desenrola um quadro com a tabela de desenrolamento da função dele, e
    // para quando o SP deixa de crescer ou sai da pilha da thread. Os slots
    // lidos são do quadro de uma função ainda ativa, parada na chamada que o
    // registro descreve.
    unsafe {
        RtlCaptureContext(&mut contexto);
        let (mut baixo, mut alto) = (0usize, 0usize);
        GetCurrentThreadStackLimits(&mut baixo, &mut alto);
        let mut pc = campo(&contexto, 0xF8);
        let mut base = 0u64;
        let mut funcao = RtlLookupFunctionEntry(pc, &mut base, std::ptr::null_mut());
        while !funcao.is_null() {
            let sp_antes = campo(&contexto, 0x98);
            let mut dados: *mut u8 = std::ptr::null_mut();
            let mut quadro = 0u64;
            RtlVirtualUnwind(0, base, pc, funcao, &mut contexto, &mut dados, &mut quadro, std::ptr::null_mut());
            // Agora o contexto é o de quem chamou: `Rip` é o endereço de
            // retorno nele e `Rsp`, o SP dele no ponto da chamada.
            let (retorno, sp, fp) = (campo(&contexto, 0xF8), campo(&contexto, 0x98), campo(&contexto, 0xA0));
            if retorno == 0 {
                break;
            }
            if sp <= sp_antes || (sp as usize) < baixo || (sp as usize) >= alto {
                anomalia_do_percurso("o SP não cresce ou saiu da pilha da thread");
                break;
            }
            quadros += 1;
            // A função de quem chamou: o começo exato (o `BeginAddress` da
            // entrada do `.pdata`) e a entrada, para o passo seguinte.
            let mut base_seguinte = 0u64;
            let seguinte = RtlLookupFunctionEntry(retorno, &mut base_seguinte, std::ptr::null_mut());
            let inicio = (!seguinte.is_null()).then(|| base_seguinte + u64::from(seguinte.cast::<u32>().read_unaligned()));
            // Os preservados no contexto são os de quem chamou, no ponto da
            // chamada: o desenrolamento dos quadros de dentro os restaurou.
            let registrador = |r: u16| NO_CONTEXTO.get(usize::from(r)).map_or(0, |&d| campo(&contexto, d));
            visitar_quadro(&mapas, inicio, retorno, sp, fp, &registrador, &mut f);
            pc = retorno;
            base = base_seguinte;
            funcao = seguinte;
        }
    }
    QUADROS_PERCORRIDOS.fetch_add(quadros, std::sync::atomic::Ordering::Relaxed);
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
fn visitar_quadros_por_mapas(mut f: impl FnMut(i64)) {
    if !HA_MAPAS_DE_PILHA.load(std::sync::atomic::Ordering::Acquire) {
        return;
    }
    unsafe extern "C" {
        fn _Unwind_Backtrace(passo: unsafe extern "C" fn(*mut u8, *mut u8) -> i32, argumento: *mut u8) -> i32;
        fn _Unwind_GetIP(contexto: *mut u8) -> usize;
        fn _Unwind_GetCFA(contexto: *mut u8) -> usize;
        fn _Unwind_GetGR(contexto: *mut u8, registrador: i32) -> usize;
        fn _Unwind_GetRegionStart(contexto: *mut u8) -> usize;
    }
    /// `_URC_NO_REASON` (continua) e `_URC_NORMAL_STOP` (para).
    const CONTINUA: i32 = 0;
    const PARA: i32 = 4;
    struct Estado<'a> {
        mapas: &'a [MapaDePilha],
        f: &'a mut dyn FnMut(i64),
        /// O CFA do quadro anterior do percurso (o chamado).
        cfa_anterior: Option<usize>,
        quadros: u64,
    }
    unsafe extern "C" fn passo(contexto: *mut u8, argumento: *mut u8) -> i32 {
        // SAFETY: `argumento` é o `Estado` abaixo, vivo durante o percurso;
        // `contexto` é o do desenrolador, válido durante a chamada.
        unsafe {
            let e = &mut *argumento.cast::<Estado<'_>>();
            let ip = _Unwind_GetIP(contexto) as u64;
            if ip == 0 {
                return PARA;
            }
            let cfa = _Unwind_GetCFA(contexto);
            if let Some(sp) = e.cfa_anterior {
                if cfa < sp {
                    anomalia_do_percurso("o CFA não cresce");
                    return PARA;
                }
                e.quadros += 1;
                let inicio = _Unwind_GetRegionStart(contexto) as u64;
                let fp = _Unwind_GetGR(contexto, i32::from(DWARF_FP)) as u64;
                // SAFETY: o contexto do desenrolador, válido durante o passo.
                let registrador = |r: u16| unsafe { _Unwind_GetGR(contexto, i32::from(r)) as u64 };
                visitar_quadro(e.mapas, (inicio != 0).then_some(inicio), ip, sp as u64, fp, &registrador, e.f);
            }
            e.cfa_anterior = Some(cfa);
            CONTINUA
        }
    }
    let mapas = MAPAS_DE_PILHA.read().unwrap_or_else(|e| e.into_inner());
    let mut estado = Estado { mapas: &mapas, f: &mut f, cfa_anterior: None, quadros: 0 };
    // SAFETY: o desenrolador percorre a pilha desta thread; `passo` só lê.
    unsafe {
        _Unwind_Backtrace(passo, std::ptr::addr_of_mut!(estado).cast::<u8>());
    }
    QUADROS_PERCORRIDOS.fetch_add(estado.quadros, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(not(any(all(windows, target_arch = "x86_64"), all(unix, any(target_arch = "x86_64", target_arch = "aarch64")))))]
fn visitar_quadros_por_mapas(_f: impl FnMut(i64)) {}

/// Os endereços de retorno da pilha desta thread, de dentro para fora, com o
/// começo da função de cada um (0 quando o desenrolador não o dá) e o topo
/// da pilha do quadro em que o endereço está (o ponteiro de pilha dele na
/// chamada; 0 quando não se sabe), até `maximo` quadros: o rastro simbólico
/// (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13.14) os guarda no `throw` e
/// só os simboliza quando o texto é pedido; o topo casa o quadro com a pilha
/// do rastro (a cadeia de quem espera).
/// O mesmo percurso de [`visitar_quadros_por_mapas`], sem ler raízes: não
/// depende dos mapas, e vale nos dois modos de raízes.
#[cfg(all(windows, target_arch = "x86_64"))]
#[allow(unsafe_code)]
#[inline(never)]
pub fn enderecos_de_retorno(maximo: usize) -> Vec<(u64, u64, u64)> {
    /// O `CONTEXT` do x86-64 (veja `visitar_quadros_por_mapas`).
    #[repr(C, align(16))]
    struct ContextoDaCpu([u8; 1232]);
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn RtlCaptureContext(contexto: *mut ContextoDaCpu);
        fn RtlLookupFunctionEntry(pc: u64, base: *mut u64, historico: *mut u8) -> *const u8;
        fn RtlVirtualUnwind(
            tipo: u32,
            base: u64,
            pc: u64,
            funcao: *const u8,
            contexto: *mut ContextoDaCpu,
            dados: *mut *mut u8,
            quadro: *mut u64,
            ponteiros: *mut u8,
        ) -> *mut u8;
        fn GetCurrentThreadStackLimits(baixo: *mut usize, alto: *mut usize);
    }
    let mut saida = Vec::new();
    let mut contexto = ContextoDaCpu([0; 1232]);
    let campo = |c: &ContextoDaCpu, d: usize| u64::from_le_bytes(c.0[d..d + 8].try_into().expect("oito bytes"));
    // SAFETY: como em `visitar_quadros_por_mapas`: o contexto é o desta
    // thread, cada passo desenrola um quadro com a tabela da função dele, e o
    // percurso para quando o SP deixa de crescer ou sai da pilha.
    unsafe {
        RtlCaptureContext(&mut contexto);
        let (mut baixo, mut alto) = (0usize, 0usize);
        GetCurrentThreadStackLimits(&mut baixo, &mut alto);
        let mut pc = campo(&contexto, 0xF8);
        let mut base = 0u64;
        let mut funcao = RtlLookupFunctionEntry(pc, &mut base, std::ptr::null_mut());
        while !funcao.is_null() && saida.len() < maximo {
            let sp_antes = campo(&contexto, 0x98);
            let mut dados: *mut u8 = std::ptr::null_mut();
            let mut quadro = 0u64;
            RtlVirtualUnwind(0, base, pc, funcao, &mut contexto, &mut dados, &mut quadro, std::ptr::null_mut());
            let (retorno, sp) = (campo(&contexto, 0xF8), campo(&contexto, 0x98));
            if retorno == 0 || sp <= sp_antes || (sp as usize) < baixo || (sp as usize) >= alto {
                break;
            }
            let mut base_seguinte = 0u64;
            let seguinte = RtlLookupFunctionEntry(retorno, &mut base_seguinte, std::ptr::null_mut());
            let inicio = if seguinte.is_null() { 0 } else { base_seguinte + u64::from(seguinte.cast::<u32>().read_unaligned()) };
            saida.push((retorno, inicio, sp));
            pc = retorno;
            base = base_seguinte;
            funcao = seguinte;
        }
    }
    saida
}

/// O mesmo nos alvos do desenrolador Itanium, por `_Unwind_Backtrace`: o
/// IP de cada quadro depois do primeiro é o endereço de retorno, e o começo
/// da função é o da região do FDE.
#[cfg(all(unix, any(target_arch = "x86_64", target_arch = "aarch64")))]
#[allow(unsafe_code)]
#[inline(never)]
pub fn enderecos_de_retorno(maximo: usize) -> Vec<(u64, u64, u64)> {
    unsafe extern "C" {
        fn _Unwind_Backtrace(passo: unsafe extern "C" fn(*mut u8, *mut u8) -> i32, argumento: *mut u8) -> i32;
        fn _Unwind_GetIP(contexto: *mut u8) -> usize;
        fn _Unwind_GetRegionStart(contexto: *mut u8) -> usize;
        fn _Unwind_GetCFA(contexto: *mut u8) -> usize;
    }
    /// `_URC_NO_REASON` (continua) e `_URC_NORMAL_STOP` (para).
    const CONTINUA: i32 = 0;
    const PARA: i32 = 4;
    struct Estado {
        saida: Vec<(u64, u64, u64)>,
        maximo: usize,
        /// O CFA do quadro anterior (mais fundo): o topo da pilha deste.
        cfa_anterior: u64,
    }
    unsafe extern "C" fn passo(contexto: *mut u8, argumento: *mut u8) -> i32 {
        // SAFETY: `argumento` é o `Estado` abaixo, vivo durante o percurso;
        // `contexto` é o do desenrolador, válido durante a chamada.
        unsafe {
            let e = &mut *argumento.cast::<Estado>();
            let ip = _Unwind_GetIP(contexto) as u64;
            if ip == 0 || e.saida.len() >= e.maximo {
                return PARA;
            }
            e.saida.push((ip, _Unwind_GetRegionStart(contexto) as u64, e.cfa_anterior));
            e.cfa_anterior = _Unwind_GetCFA(contexto) as u64;
            CONTINUA
        }
    }
    let mut estado = Estado { saida: Vec::new(), maximo, cfa_anterior: 0 };
    // SAFETY: o desenrolador percorre a pilha desta thread; `passo` só lê.
    unsafe {
        _Unwind_Backtrace(passo, std::ptr::addr_of_mut!(estado).cast::<u8>());
    }
    estado.saida
}

#[cfg(not(any(all(windows, target_arch = "x86_64"), all(unix, any(target_arch = "x86_64", target_arch = "aarch64")))))]
pub fn enderecos_de_retorno(_maximo: usize) -> Vec<(u64, u64, u64)> {
    Vec::new()
}

/// O build de conferência (§15.2, E2.5): os valores dos slots de
/// conferência dos quadros da pilha-sombra desta thread (os últimos `k`
/// slots de um quadro cujo campo `n` traz `k` nos bits 40 em diante).
#[allow(unsafe_code)]
fn visitar_slots_de_conferencia(mut f: impl FnMut(i64)) {
    let mut q = CONTEXTO.with(|c| c.topo.get());
    while !q.is_null() {
        // SAFETY: cada quadro encadeado está no stack de uma função ainda
        // ativa desta thread (como em `visitar_quadros`).
        unsafe {
            let n = (*q).n as u64;
            let total = (n & SLOTS_DO_QUADRO) as usize;
            let k = ((n & !QUADRO_ORDENADO) >> 40) as usize;
            let slots = std::ptr::addr_of!((*q).slots) as *const i64;
            for i in total.saturating_sub(k)..total {
                f(*slots.add(i));
            }
            q = (*q).anterior;
        }
    }
}

/// `DARTFORGE_GC_PERCURSO=conferir`: toda raiz que o build de conferência
/// gravou na pilha-sombra tem de estar entre as que o percurso por mapas
/// visitou (`dos_mapas`). Uma que falte é raiz que o mapa perdeu.
#[allow(unsafe_code)]
fn conferir_percurso(dos_mapas: &[i64]) {
    let vistos: crate::hash::HashSet<i64> = dos_mapas.iter().copied().collect();
    let mut conferidas = 0u64;
    visitar_slots_de_conferencia(|h| {
        // Só os objetos do heap: um `Smi` (ou o nulo) no slot não é raiz, e
        // o objeto estático da imagem (`PERMANENTE`) nunca é coletado — o
        // compilador o tira do `"deopt"` (`tirar_estaticos_do_deopt`, §14.12).
        if !crate::layout::e_objeto(h) {
            return;
        }
        // SAFETY: `h` é o handle de um objeto que o código gerado gravou no
        // slot de conferência, vivo neste ponto de coleta.
        if unsafe { (*((h - DESLOCAMENTO_DO_HANDLE) as *const Cabecalho)).estado } == PERMANENTE {
            return;
        }
        conferidas += 1;
        if !vistos.contains(&h) {
            mapa_recusado(&format!("conferência do percurso: a raiz {h:#x} está viva na pilha-sombra de conferência e o percurso por mapas não a visitou"));
        }
    });
    RAIZES_CONFERIDAS.fetch_add(conferidas, std::sync::atomic::Ordering::Relaxed);
}

/// O heap de um isolado: o espaço de objetos (todo valor do runtime é um bloco
/// dele, docs/NATIVO-ESPACO-UNIFICADO.md), as raízes, as tabelas laterais por
/// handle e a coleta.
/// A coleta agendada por semente (`DARTFORGE_GC_AGENDA=<semente>,<taxa>`,
/// docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.5): uma coleta em uma de cada
/// `taxa` alocações, sorteadas por um gerador fixo a partir da semente. A
/// mesma semente e o mesmo programa coletam nos mesmos pontos: acha o defeito
/// que o `--gc-stress` total esconde por mudar o tempo, e o reproduz. Como
/// no estresse, sem TLAB (toda alocação passa pelo runtime).
#[derive(Debug)]
struct Agenda {
    estado: std::cell::Cell<u64>,
    taxa: u64,
    /// O sorteio já saiu e a coleta ainda não veio: a próxima consulta
    /// responde o mesmo (o caminho lento consulta de novo).
    pendente: std::cell::Cell<bool>,
}

impl Agenda {
    fn do_ambiente() -> Option<Agenda> {
        let v = std::env::var("DARTFORGE_GC_AGENDA").ok()?;
        let (semente, taxa) = v.split_once(',')?;
        let semente: u64 = semente.trim().parse().ok()?;
        let taxa: u64 = taxa.trim().parse().ok().filter(|&t| t > 0)?;
        let estado = semente ^ 0x9E37_79B9_7F4A_7C15;
        Some(Agenda { estado: std::cell::Cell::new(if estado == 0 { 1 } else { estado }), taxa, pendente: std::cell::Cell::new(false) })
    }

    /// Esta alocação coleta? (O xorshift64*.)
    fn sorteia(&self) -> bool {
        if self.pendente.get() {
            return true;
        }
        let mut x = self.estado.get();
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.estado.set(x);
        let sim = x.wrapping_mul(0x2545_F491_4F6C_DD1D) % self.taxa == 0;
        self.pendente.set(sim);
        sim
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
    agenda: Option<Agenda>,
    stats: HeapStats,
    /// Posições percorridas pela última marcação.
    trabalho_da_marcacao: usize,
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
    limite_bytes: usize,
    enum_values: crate::hash::HashMap<(i64, i64), i64>,
    /// Tear-offs canônicos de funções top-level, por ID de código.
    ///
    /// O oráculo Dart 3.6.2 exige `identical(f, f)` verdadeiro para dois
    /// tear-offs da mesma função top-level; cada `code_id` tem um único handle,
    /// mantido vivo como raiz permanente, como os singletons de enum.
    tearoffs: crate::hash::HashMap<i64, i64>,
    /// Literais do compilador, canônicos por unidades UTF-16. Permanecem
    /// enraizados pelo isolate; strings criadas em execução não entram aqui.
    literais: crate::hash::HashMap<Vec<u16>, i64>,
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
    constantes: crate::hash::HashMap<i64, usize>,
    /// Tear-off canônico → o código dele (o inverso de `tearoffs`).
    codigo_do_tearoff: crate::hash::HashMap<i64, i64>,
    /// `DARTFORGE_GC_OFF=1`: nunca coleta. Instrumento de diagnóstico
    /// (docs/NATIVO-PLANO.md §6): um programa que morre com "handle já
    /// coletado" e passa com a coleta desligada tem raiz faltando; um que
    /// morre igual nos dois modos tem escalar usado como handle.
    gc_desligado: bool,
    /// Valor corrente de cada global `Ref` do programa (variável de topo ou
    /// campo estático), por id: raízes permanentes (N6/G6 do contrato).
    globais: crate::hash::HashMap<i64, i64>,
    /// Raízes que moram no runtime e não num frame (G6): a exceção pendente
    /// e o rastro corrente. 0 = nenhuma.
    raizes_do_runtime: [i64; 2],
    /// Ocorrências compartilhadas mantidas por mensagens neste isolado.
    owners_mensagens: crate::hash::HashMap<u64, Ref>,
    proximo_owner_mensagem: u64,
    /// Campos `late` já escritos, por handle e índice físico. A marca fica
    /// fora do valor: zero e null são atribuições válidas do programa.
    campos_late_inicializados: crate::hash::HashSet<(i64, i64)>,
    /// As marcas de `campos_late_inicializados` gravadas desde a última
    /// coleta: só elas podem ser de um jovem, e a coleta menor confere só
    /// estas (os velhos não morrem nela).
    late_novos: Vec<(i64, i64)>,
    /// A época de layout (`EPOCA_DE_LAYOUT`) dos objetos deste heap: a de
    /// quando ele nasceu, e a de cada migração aplicada depois (J03).
    pub epoca_de_layout: u64,
    /// Onde mora todo valor do runtime ([`EspacoDeObjetos`]).
    objetos: EspacoDeObjetos,
    /// O heap do isolado da thread (não um de teste): reabastece a TLAB do
    /// [`Contexto`] dela.
    publica: bool,
    /// Bytes alocados desde a última coleta (o gatilho da coleta menor).
    bytes_jovens: usize,
    /// A coleta em curso é menor (só os jovens).
    coleta_menor: bool,
    /// `--gc-stress`: coletas desde a última completa e o trabalho de marcação
    /// dela (o espaçamento das completas, [`Heap::coletar_automatico`]).
    menores_desde_completa: u64,
    trabalho_na_completa: usize,
    /// Bytes fora do heap que contam para os gatilhos
    /// ([`Heap::contar_externos`]); os dos anexos o espaço conta à parte.
    externos: usize,
    /// `DARTFORGE_GC_VERIFICAR=1`: toda coleta menor confere, por uma
    /// travessia completa, que nenhum jovem alcançável ficou sem marca (uma
    /// barreira de escrita faltando).
    /// Quanto o heap pode crescer até a próxima coleta completa, em % do que
    /// sobreviveu à última ([`CRESCIMENTO`]; para medir,
    /// `DARTFORGE_GC_CRESCIMENTO`).
    crescimento: usize,
    /// Os bytes entre coletas menores ([`LIMITE_JOVEM`]; para medir,
    /// `DARTFORGE_GC_JOVEM_KB`).
    limite_jovem: usize,
    verificar: bool,
    /// Confere todo handle contra o mapa de páginas antes de ler o bloco (as
    /// mensagens N4 de handle inválido): no `--gc-stress`, com a verificação,
    /// nos testes e no runtime de depuração, ou com
    /// `DARTFORGE_VALIDAR_HANDLES=1`. Fora disso, um handle de objeto é o
    /// bloco (a invariante do código gerado): só o estado `LIVRE` (objeto já
    /// coletado) é conferido, na leitura do cabeçalho que se faz de todo modo.
    validar_handles: bool,
    /// `DARTFORGE_GC_RASTRO=1`: uma linha por coleta no stderr (o tipo, os
    /// marcados, a estimativa e o gatilho).
    rastrear: bool,
    /// `DARTFORGE_GC_MEMORIA=1`: depois de cada coleta completa (e no fim do
    /// programa), a quebra da memória do espaço por classe de tamanho, das
    /// tabelas do runtime e do processo ([`Heap::relatorio_de_memoria`]).
    /// Com `2`, depois de toda coleta.
    memoria: u8,
    /// Pares nativos finalizáveis (o `Dart_NewFinalizableHandle` da VM):
    /// objeto → (finalizador, par). Quando o objeto morre, a coleta tira a
    /// entrada e chama `finalizador(par)` — que só libera recursos do
    /// sistema (fecha um arquivo, solta uma contagem de referências) e nunca
    /// toca o heap.
    pub finalizaveis: crate::hash::HashMap<i64, Finalizador>,
    /// As referências fracas (`WeakReference`, o `WeakReference_*` da VM):
    /// objeto portador → alvo. O alvo NÃO é seguido pela marcação; se não
    /// sobreviver por outro caminho, a coleta o troca por 0 (null).
    pub fracas: crate::hash::HashMap<i64, i64>,
    /// Os efêmeros (`_WeakProperty`, a base do `Expando`): portador →
    /// (chave, valor). O valor só é alcançado se a chave for (ponto fixo na
    /// marcação); chave morta zera os dois.
    pub efemeros: crate::hash::HashMap<i64, (i64, i64)>,
    /// Os anexos de `Finalizer`/`NativeFinalizer` (o `FinalizerEntry` da
    /// VM): o valor e a chave de `detach` são fracos; o dono e a ação,
    /// fortes. Valor morto: a ação de um `Finalizer` vai para
    /// [`Heap::finalizacoes_prontas`] (o laço de eventos a chama); a de um
    /// `NativeFinalizer` roda logo depois da coleta.
    pub anexos: Vec<AnexoDeFinalizador>,
    /// As ações de `Finalizer` cujo valor morreu, à espera do laço de
    /// eventos (raízes até lá).
    pub finalizacoes_prontas: std::collections::VecDeque<i64>,
    /// O ARC (`--memoria=arc`, docs/ARC-IMPLEMENTACAO.md): ligado pelo
    /// programa na entrada (`dartforge_memoria_arc_v1`); `None` é o coletor
    /// por rastreamento de sempre.
    arc: Option<Box<ArcDoHeap>>,
}

/// Um anexo de finalizador (ver [`Heap::anexos`]).
#[derive(Debug, Clone, Copy)]
pub struct AnexoDeFinalizador {
    /// O `Finalizer`/`NativeFinalizer` (identidade do `detach`).
    pub dono: i64,
    pub valor: i64,
    /// A chave de `detach` (0 = nenhuma).
    pub desanexo: i64,
    pub acao: AcaoDeFinalizador,
}

/// O que um finalizador faz quando o valor morre.
#[derive(Debug, Clone, Copy)]
pub enum AcaoDeFinalizador {
    /// Uma closure Dart sem argumentos (`callback(token)` já aplicada).
    Dart(i64),
    /// `funcao(token)`, uma função C (`NativeFinalizerFunction`).
    Nativa(usize, usize),
}

#[allow(unsafe_code)]
impl Heap {
    /// Roda as ações nativas de todos os anexos ainda vivos (o isolado
    /// terminou: a VM garante os `NativeFinalizer` no encerramento) e
    /// descarta os de `Finalizer`.
    pub fn encerrar_finalizadores(&mut self) {
        let anexos = std::mem::take(&mut self.anexos);
        while self.concluir_finalizacao_pronta().is_some() {}
        for a in anexos {
            if let AcaoDeFinalizador::Nativa(f, token) = a.acao {
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
            agenda: Agenda::do_ambiente(),
            stats: HeapStats::default(),
            trabalho_da_marcacao: 0,
            pending: Vec::new(),
            byte_threshold: 2 * LIMITE_JOVEM,
            crescimento: std::env::var("DARTFORGE_GC_CRESCIMENTO").ok().and_then(|v| v.parse::<usize>().ok()).map_or(CRESCIMENTO, |c| c.max(110)),
            limite_jovem: std::env::var("DARTFORGE_GC_JOVEM_KB")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .map_or(LIMITE_JOVEM, |kb| (kb * 1024).max(64 * 1024)),
            limite_bytes: Self::limite_do_ambiente(),
            enum_values: crate::hash::HashMap::default(),
            tearoffs: crate::hash::HashMap::default(),
            literais: crate::hash::HashMap::default(),
            permanentes: crate::hash::HashSet::default(),
            constantes: crate::hash::HashMap::default(),
            codigo_do_tearoff: crate::hash::HashMap::default(),
            gc_desligado: std::env::var("DARTFORGE_GC_OFF").as_deref() == Ok("1"),
            globais: crate::hash::HashMap::default(),
            raizes_do_runtime: [0, 0],
            owners_mensagens: crate::hash::HashMap::default(),
            proximo_owner_mensagem: 0,
            campos_late_inicializados: crate::hash::HashSet::default(),
            late_novos: Vec::new(),
            epoca_de_layout: EPOCA_DE_LAYOUT.load(std::sync::atomic::Ordering::Acquire),
            // Os testes do runtime conferem que o handle de um morto é
            // recusado: lá, como no `--gc-stress`, os mortos são zerados já.
            externos: 0,
            objetos: EspacoDeObjetos::new(stress || cfg!(test) || std::env::var("DARTFORGE_GC_VERIFICAR").as_deref() == Ok("1")),
            publica: false,
            bytes_jovens: 0,
            arc: None,
            coleta_menor: false,
            menores_desde_completa: 0,
            trabalho_na_completa: 0,
            verificar: std::env::var("DARTFORGE_GC_VERIFICAR").as_deref() == Ok("1"),
            validar_handles: stress
                || cfg!(any(test, debug_assertions))
                // O veneno só pega o uso depois de liberar na validação.
                || std::env::var("DARTFORGE_GC_VENENO").is_ok_and(|v| !v.is_empty() && v != "0")
                || std::env::var("DARTFORGE_GC_VERIFICAR").as_deref() == Ok("1")
                || std::env::var("DARTFORGE_VALIDAR_HANDLES").as_deref() == Ok("1"),
            rastrear: std::env::var("DARTFORGE_GC_RASTRO").as_deref() == Ok("1"),
            memoria: std::env::var("DARTFORGE_GC_MEMORIA").ok().and_then(|v| v.parse::<u8>().ok()).unwrap_or(0),
            finalizaveis: crate::hash::HashMap::default(),
            fracas: crate::hash::HashMap::default(),
            efemeros: crate::hash::HashMap::default(),
            anexos: Vec::new(),
            finalizacoes_prontas: std::collections::VecDeque::new(),
        }
    }
    /// Slot proprietário do runtime: 0 = exceção pendente, 1 = rastro corrente.
    ///
    /// # Panics
    /// `qual` não é 0 nem 1, ou um handle morto é detectado pela validação.
    ///
    /// ```
    /// use dartforge_runtime::heap::Heap;
    /// let mut heap = Heap::new(false);
    /// heap.set_raiz_do_runtime(0, 0);
    /// ```
    pub fn set_raiz_do_runtime(&mut self, qual: usize, handle: i64) {
        if smi::e_handle(handle) {
            self.conferir_vivo(handle);
        }
        let antigo = self.raizes_do_runtime[qual];
        self.arc_trocar_raiz_proprietaria(handle, antigo);
        self.raizes_do_runtime[qual] = handle;
    }
    /// A string internada com as `unidades` (a tabela `literais`, raiz), se já
    /// existe: o literal do JIT e do runtime (`textos.rs`, P1).
    pub fn literal(&self, unidades: &[u16]) -> Option<Ref> {
        self.literais.get(unidades).copied()
    }
    /// Interna `h` (uma string com as `unidades`) como literal canônico: raiz
    /// (`literais`) e permanente (identidade entre portas do mesmo isolado).
    /// A entrada conta um owner; repetir o mesmo registro não retém novamente.
    ///
    /// ```
    /// use dartforge_runtime::heap::Heap;
    /// let mut heap = Heap::new(false);
    /// let texto = heap.alocar_str("A");
    /// heap.guardar_literal(vec![65], texto);
    /// assert_eq!(heap.literal(&[65]), Some(texto));
    /// ```
    pub fn guardar_literal(&mut self, unidades: Vec<u16>, h: Ref) {
        if smi::e_handle(h) { self.conferir_vivo(h); }
        let antigo = self.literais.get(&unidades).copied().unwrap_or(0);
        self.arc_trocar_raiz_proprietaria(h, antigo);
        self.literais.insert(unidades, h);
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
        if smi::e_handle(handle) { self.conferir_vivo(handle); }
        let antigo = self.globais.get(&id).copied().unwrap_or(0);
        self.arc_trocar_raiz_proprietaria(handle, antigo);
        if !smi::e_handle(handle) {
            // null ou `Smi`: nada no heap a manter vivo.
            self.globais.remove(&id);
        } else {
            self.globais.insert(id, handle);
        }
    }

    /// Move a raiz de um global de um endereço de slot para outro (a área de
    /// globais de um módulo recarregado, `gc_raizes.rs`).
    /// Solta o owner anterior do destino. Origem ausente ou movimento para
    /// o mesmo endereço não altera os owners.
    ///
    /// ```
    /// use dartforge_runtime::heap::Heap;
    /// let mut heap = Heap::new(false);
    /// heap.mover_raiz_global(10, 20);
    /// ```
    pub fn mover_raiz_global(&mut self, de: i64, para: i64) {
        if de == para { return; }
        if let Some(h) = self.globais.remove(&de) {
            let antigo = self.globais.get(&para).copied().unwrap_or(0);
            self.arc_trocar_raiz_proprietaria(0, antigo);
            self.globais.insert(para, h);
        }
    }

    /// Solta a raiz de um global que deixou de existir.
    ///
    /// ```
    /// use dartforge_runtime::heap::Heap;
    /// let mut heap = Heap::new(false);
    /// heap.soltar_raiz_global(10);
    /// ```
    pub fn soltar_raiz_global(&mut self, id: i64) {
        if let Some(h) = self.globais.remove(&id) {
            self.arc_trocar_raiz_proprietaria(0, h);
        }
    }

    /// Marca `handle` como permanente (ver `permanentes`).
    pub fn marcar_permanente(&mut self, handle: i64) {
        if smi::e_handle(handle) {
            self.permanentes.insert(handle);
        }
    }

    /// Marca `handle` como a constante canônica que `getter` produz.
    pub fn marcar_constante(&mut self, handle: i64, getter: usize) {
        if smi::e_handle(handle) {
            self.permanentes.insert(handle);
            if getter != 0 {
                self.constantes.insert(handle, getter);
            }
        }
    }

    /// O getter da constante canônica `handle`, se for uma.
    pub fn getter_da_constante(&self, handle: i64) -> Option<usize> {
        self.constantes.get(&handle).copied()
    }

    /// O código do tear-off canônico `handle`, se for um.
    pub fn codigo_do_tearoff(&self, handle: i64) -> Option<i64> {
        self.codigo_do_tearoff.get(&handle).copied()
    }

    /// Se `handle` é permanente e imutável (ver `permanentes`): uma
    /// mensagem no mesmo isolado o passa sem copiar.
    pub fn e_permanente(&self, handle: i64) -> bool {
        self.permanentes.contains(&handle) || e_estatico(handle)
    }
    /// Le o teto do heap do ambiente uma vez, na criacao.
    fn limite_do_ambiente() -> usize {
        const PADRAO: usize = 256 * 1024 * 1024;
        match std::env::var("DARTFORGE_HEAP_MAX_MB") {
            Ok(v) => match v.trim().parse::<usize>() {
                Ok(0) => usize::MAX,
                Ok(mb) => mb.saturating_mul(1024 * 1024),
                Err(_) => PADRAO,
            },
            Err(_) => PADRAO,
        }
    }

    /// Bytes que o heap ocupa: os blocos em uso (com as regiões grandes e os
    /// bytes de fora, anexos e externos), mais `adicional`.
    fn bytes_totais(&self, adicional: usize) -> usize {
        self.stats.estimated_bytes.saturating_add(adicional)
    }

    /// Para o processo quando o heap passa do teto.
    fn verificar_teto(&self, adicional: usize) {
        if self.limite_bytes == usize::MAX {
            return;
        }
        let total = self.bytes_totais(adicional);
        if total > self.limite_bytes {
            Self::abortar_por_memoria(total, self.limite_bytes);
        }
    }

    #[cold]
    #[inline(never)]
    fn abortar_por_memoria(total: usize, limite: usize) -> ! {
        // Sem `panic!`: o runtime e chamado por `extern "C"` a partir do codigo
        // gerado, e um panico atravessando essa fronteira aborta com um despejo
        // de pilha ilegivel. Uma linha e o codigo 255 (o mesmo da VM para
        // excecao nao capturada) sao o que o harness precisa.
        let mib = 1024 * 1024;
        eprintln!(
            "Out of memory: heap do DartForge chegou a {} MiB, acima do teto de {} MiB (ajuste com DARTFORGE_HEAP_MAX_MB, 0 desliga).",
            total / mib,
            limite / mib
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
    /// let valor = heap.enum_value(900, 0, "zero");
    /// assert_eq!(heap.enum_value(900, 0, "zero"), valor);
    /// ```
    pub fn enum_value(&mut self, class_id: i64, index: i64, name: &str) -> i64 {
        assert!(class_id >= 0 && index >= 0, "identidade enum inválida");
        if let Some(&handle) = self.enum_values.get(&(class_id, index)) {
            return handle;
        }
        let frame = self.push_frame_proprietario(1);
        let text = self.alocar_str(name);
        self.set_root(frame, 0, text);
        let object = self.novo_objeto(class_id, &[(index, false), (text, true)]);
        self.arc_trocar_raiz_proprietaria(object, 0);
        self.enum_values.insert((class_id, index), object);
        self.permanentes.insert(object);
        self.pop_frame(frame);
        object
    }
    /// O tear-off canônico já registrado da função de topo `codigo` (a tabela
    /// `tearoffs`, raiz), se existe (`caixas.rs`, P2).
    pub fn tearoff_registrado(&self, codigo: i64) -> Option<Ref> {
        self.tearoffs.get(&codigo).copied()
    }
    /// Registra `h` como o tear-off canônico de `codigo`: raiz (`tearoffs`), o
    /// inverso (`codigo_do_tearoff`) e permanente.
    /// A tabela conta um owner, incluindo substituição sem duplicar aliases.
    ///
    /// ```
    /// use dartforge_runtime::heap::Heap;
    /// let mut heap = Heap::new(false);
    /// let closure = heap.nova_closure(123, (0, false), 0, 0);
    /// heap.registrar_tearoff(123, closure);
    /// assert_eq!(heap.tearoff_registrado(123), Some(closure));
    /// ```
    pub fn registrar_tearoff(&mut self, codigo: i64, h: Ref) {
        if smi::e_handle(h) { self.conferir_vivo(h); }
        let antigo = self.tearoffs.get(&codigo).copied().unwrap_or(0);
        self.arc_trocar_raiz_proprietaria(h, antigo);
        self.tearoffs.insert(codigo, h);
        self.codigo_do_tearoff.insert(h, codigo);
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
    /// let frame = heap.push_frame_proprietario(1);
    /// heap.set_root(frame, 0, 0);
    /// heap.pop_frame(frame);
    /// ```
    pub fn push_frame_proprietario(&mut self, slots: usize) -> i64 {
        let id = self.push_frame_with_slots(slots);
        self.frames.last_mut().unwrap().2 = true;
        id
    }

    fn raizes_proprietarias(&self) -> Vec<Ref> {
        self.frames.iter().filter(|(_, _, dono)| *dono)
            .flat_map(|(_, slots, _)| slots.iter().copied())
            .chain(self.globais.values().copied())
            .chain(self.literais.values().copied())
            .chain(self.tearoffs.values().copied())
            .chain(self.enum_values.values().copied())
            .chain(self.owners_mensagens.values().copied())
            .chain(self.finalizacoes_prontas.iter().copied())
            .chain(self.raizes_do_runtime.iter().copied()).collect()
    }

    /// Publica uma ocorrência proprietária na fila, sem coleta entre retenção e escrita.
    #[cfg(test)]
    pub(crate) fn adicionar_finalizacao_pronta(&mut self, acao: Ref) {
        self.conferir_vivo(acao);
        self.arc_trocar_raiz_proprietaria(acao, 0);
        self.finalizacoes_prontas.push_back(acao);
    }

    /// Consome o owner da primeira finalização; fila vazia é um no-op.
    pub(crate) fn concluir_finalizacao_pronta(&mut self) -> Option<Ref> {
        let acao = self.finalizacoes_prontas.pop_front()?;
        self.arc_trocar_raiz_proprietaria(0, acao);
        Some(acao)
    }

    /// Retém uma ocorrência de mensagem no domínio deste heap.
    pub(crate) fn reter_owner_mensagem(&mut self, valor: Ref) -> u64 {
        self.conferir_vivo(valor);
        let id = self.proximo_owner_mensagem.checked_add(1).expect("owners de mensagem esgotados");
        self.arc_trocar_raiz_proprietaria(valor, 0);
        self.owners_mensagens.insert(id, valor);
        self.proximo_owner_mensagem = id;
        id
    }

    /// Consome uma ocorrência de mensagem neste heap, sem coleta.
    pub(crate) fn soltar_owner_mensagem(&mut self, id: u64) {
        let valor = self.owners_mensagens.remove(&id).expect("owner de mensagem inexistente");
        self.arc_trocar_raiz_proprietaria(0, valor);
    }

    /// Retém antes de soltar, sem coleta nem chamada Dart entre as operações.
    fn arc_trocar_raiz_proprietaria(&mut self, novo: Ref, antigo: Ref) {
        if novo == antigo { return; }
        let Some(mut arc) = self.arc.take() else { return };
        self.arc_reter_registrando(&mut arc, novo);
        arc.estado.soltar(antigo).unwrap_or_else(|e| falha_do_arc(e));
        self.arc = Some(arc);
    }

    /// Copia a raiz para o slot; zero remove a referência anterior.
    /// Num quadro proprietário, retém a nova ocorrência antes de soltar a antiga.
    /// Um `Smi` ocupa o slot como qualquer `Ref`, mas o coletor não o segue.
    pub fn set_root(&mut self, frame: i64, slot: usize, handle: i64) {
        if smi::e_handle(handle) {
            self.conferir_vivo(handle);
        }
        let indice = self.frames.iter().rposition(|(id, _, _)| *id == frame)
            .expect("frame inexistente");
        let antigo = *self.frames[indice].1.get(slot).expect("slot de raiz inválido");
        if self.frames[indice].2 { self.arc_trocar_raiz_proprietaria(handle, antigo); }
        let previous = &mut self.frames[indice].1[slot];
        self.stats.live_roots -= usize::from(*previous != 0);
        self.stats.live_roots += usize::from(handle != 0);
        *previous = handle;
        self.stats.peak_roots = self.stats.peak_roots.max(self.stats.live_roots);
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
    /// let chamador = heap.push_frame_proprietario(1);
    /// let chamado = heap.push_frame_proprietario(1);
    /// heap.mover_raiz(chamado, 0, chamador, 0);
    /// heap.pop_frame(chamado);
    /// heap.pop_frame(chamador);
    /// ```
    pub fn mover_raiz(&mut self, origem: i64, slot_origem: usize, destino: i64, slot_destino: usize) {
        let de = self.frames.iter().rposition(|(id, _, _)| *id == origem)
            .expect("frame de origem inexistente");
        let para = self.frames.iter().rposition(|(id, _, _)| *id == destino)
            .expect("frame de destino inexistente");
        assert!(self.frames[de].2 && self.frames[para].2, "movimento exige quadros proprietários");
        let valor = *self.frames[de].1.get(slot_origem).expect("slot de origem inválido");
        let antigo = *self.frames[para].1.get(slot_destino).expect("slot de destino inválido");
        if de == para && slot_origem == slot_destino { return; }
        self.arc_trocar_raiz_proprietaria(0, antigo);
        self.frames[de].1[slot_origem] = 0;
        self.frames[para].1[slot_destino] = valor;
        self.stats.live_roots -= usize::from(antigo != 0);
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
    /// let texto = heap.alocar_str("evento");
    /// heap.set_global_root(-1, texto);
    /// let frame = heap.push_frame_proprietario(1);
    /// heap.mover_global_para_slot(-1, frame, 0);
    /// heap.pop_frame(frame);
    /// ```
    pub fn mover_global_para_slot(&mut self, global: i64, frame: i64, slot: usize) {
        let indice = self.frames.iter().rposition(|(id, _, _)| *id == frame)
            .expect("frame inexistente");
        assert!(self.frames[indice].2, "movimento exige quadro proprietário");
        let antigo = *self.frames[indice].1.get(slot).expect("slot de raiz inválido");
        let valor = *self.globais.get(&global).expect("owner global inexistente");
        self.arc_trocar_raiz_proprietaria(0, antigo);
        self.globais.remove(&global);
        self.frames[indice].1[slot] = valor;
        self.stats.live_roots -= usize::from(antigo != 0);
        self.stats.live_roots += usize::from(valor != 0);
        self.stats.peak_roots = self.stats.peak_roots.max(self.stats.live_roots);
    }

    /// Protege handle até o retorno da função; null não ocupa uma raiz.
    pub fn root(&mut self, frame: i64, handle: i64) {
        if !smi::e_handle(handle) {
            return;
        }
        self.conferir_vivo(handle);
        let indice = self.frames.iter().rposition(|(id, _, _)| *id == frame)
            .expect("frame inexistente");
        if self.frames[indice].2 { self.arc_trocar_raiz_proprietaria(handle, 0); }
        self.frames[indice].1.push(handle);
        self.stats.live_roots += 1;
        self.stats.root_slots += 1;
        self.stats.peak_roots = self.stats.peak_roots.max(self.stats.live_roots);
        self.stats.peak_root_slots = self.stats.peak_root_slots.max(self.stats.root_slots);
    }
    /// Fecha exatamente o frame do topo, sem coletar entre retorno e raiz do chamador.
    pub fn pop_frame(&mut self, frame: i64) {
        assert_eq!(self.frames.last().map(|(id, _, _)| *id), Some(frame));
        let (_, roots, proprietario) = self.frames.pop().unwrap();
        if proprietario {
            for &h in &roots { self.arc_trocar_raiz_proprietaria(0, h); }
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
        //   (`recalcular_gatilhos`) a partir dos sobreviventes e da folga até
        //   o teto;
        // * o TETO (`limite_bytes`): só a alocação que passaria dele força
        //   uma última coleta; se ainda passa, falta memória.
        // O portão antigo (`!self.frames.is_empty()`) existia porque o código
        // gerado não registrava raízes; com o frame de cada função (G1),
        // coletar sem frame aberto é só coletar com as raízes permanentes.
        if !self.gc_desligado && self.precisa_coletar(bytes) {
            self.coletar_automatico(bytes);
        }
        // Depois da coleta: se ainda passa do teto, nao ha o que recuperar.
        self.verificar_teto(bytes);
        self.contar_alocacao(1, bytes);
    }
    /// Um objeto do usuário novo com os campos `campos` (os que o runtime
    /// monta à mão: erros, valores de enum, a classe `Type`…). Coleta antes
    /// como [`Heap::allocate`]: as referências em `campos` precisam de raiz
    /// de quem chama.
    pub fn novo_objeto(&mut self, class_id: i64, campos: &[Campo]) -> i64 {
        let h = self.alocar_objeto(class_id, campos.len());
        for (i, &(bits, e_ref)) in campos.iter().enumerate() {
            self.definir_campo(h, i, bits, e_ref);
        }
        h
    }
    /// Conta `k` alocações de `bytes` no total para os gatilhos e os
    /// contadores.
    #[inline]
    fn contar_alocacao(&mut self, k: usize, bytes: usize) {
        self.stats.estimated_bytes = self.stats.estimated_bytes.checked_add(bytes).expect("heap excede usize");
        self.stats.peak_estimated_bytes = self.stats.peak_estimated_bytes.max(self.stats.estimated_bytes);
        self.allocations += k;
        self.stats.allocations += k as u64;
        self.bytes_jovens += bytes;
    }
    /// O campo `late` `chave` (handle, índice) já foi escrito?
    #[inline]
    pub fn late_inicializado(&self, chave: (i64, i64)) -> bool {
        !self.campos_late_inicializados.is_empty() && self.campos_late_inicializados.contains(&chave)
    }
    /// Marca o campo `late` `chave` como escrito.
    pub fn marcar_late(&mut self, chave: (i64, i64)) {
        if self.campos_late_inicializados.insert(chave) {
            self.late_novos.push(chave);
        }
    }
    /// Desfaz a marca do campo `late` `chave`.
    pub fn desmarcar_late(&mut self, chave: (i64, i64)) {
        self.campos_late_inicializados.remove(&chave);
    }
    /// O heap do isolado da thread: reabastece a TLAB do [`Contexto`] (a
    /// alocação em linha do código gerado).
    pub fn do_isolado(stress: bool) -> Self {
        let mut h = Self::new(stress);
        h.publica = true;
        h
    }
    /// Aloca um objeto do usuário de `n` campos zerados (o caminho do
    /// `dartforge_object_new`), no espaço de objetos.
    ///
    /// O caminho rápido confere os gatilhos de [`Heap::allocate`] numa
    /// comparação cada; se algum dispara (coleta, teto, `--gc-stress`), vai
    /// ao caminho geral.
    #[inline]
    pub fn alocar_objeto(&mut self, class_id: i64, n: usize) -> i64 {
        let cid = i32::try_from(class_id).expect("id de classe além de 32 bits");
        self.alocar_bloco(cid, crate::layout::palavras_de_instancia(n), flags_do_bloco::INSTANCIA, n)
    }
    /// Um bloco de corpo de `palavras` palavras zeradas, classe `cid`, `flags` e
    /// `n` no cabeçalho (em `INSTANCIA`, o número de campos; nos demais, as
    /// palavras): o caminho de [`Heap::alocar`], [`Heap::alocar_objeto`] e de
    /// `dartforge_alocar`.
    #[inline]
    pub(crate) fn alocar_bloco(&mut self, cid: i32, palavras: usize, flags: u8, n: usize) -> Ref {
        let bytes = bytes_de_alocacao(palavras);
        if self.precisa_coletar(bytes) {
            return self.alocar_lento(cid, palavras, flags, n);
        }
        self.contar_alocacao(1, bytes);
        self.objetos.alocar(cid, palavras, flags, n)
    }
    /// Uma alocação de `bytes` agora passaria de algum gatilho de coleta?
    /// A menor vem a cada [`LIMITE_JOVEM`] bytes (ou [`CONTAGEM_JOVEM`]
    /// alocações) desde a última coleta; a completa, quando o total passa do
    /// gatilho que a última completa calculou (`byte_threshold`) ou do teto.
    #[inline]
    fn precisa_coletar(&self, bytes: usize) -> bool {
        self.stress
            || self.agenda.as_ref().is_some_and(Agenda::sorteia)
            || self.allocations >= CONTAGEM_JOVEM
            || self.bytes_jovens + bytes > self.limite_jovem
            || self.stats.estimated_bytes.saturating_add(bytes) > self.byte_threshold
            || (self.limite_bytes != usize::MAX && self.bytes_totais(bytes) > self.limite_bytes)
    }
    /// A coleta que os gatilhos pedem antes de alocar `bytes`: a completa se
    /// o total passou do gatilho dela (ou do teto, ou a cada
    /// [`MENORES_POR_COMPLETA_NO_ESTRESSE`] no `--gc-stress`); a menor senão.
    fn coletar_automatico(&mut self, bytes: usize) {
        let completa = self.stats.estimated_bytes.saturating_add(bytes) > self.byte_threshold
            || (self.limite_bytes != usize::MAX && self.bytes_totais(bytes) > self.limite_bytes)
            || (self.stress
                && self.menores_desde_completa + 1
                    >= MENORES_POR_COMPLETA_NO_ESTRESSE.max((self.trabalho_na_completa / TRABALHO_POR_MENOR_NO_ESTRESSE) as u64));
        self.coletar(!completa);
    }
    #[cold]
    #[inline(never)]
    fn alocar_lento(&mut self, cid: i32, palavras: usize, flags: u8, n: usize) -> Ref {
        self.antes_de_alocar(bytes_de_alocacao(palavras));
        self.objetos.alocar(cid, palavras, flags, n)
    }
    /// Reabastece a TLAB de `n` campos do [`Contexto`] (se esgotada) com
    /// uma faixa contígua de até [`TLAB_BLOCOS`] blocos livres, contados
    /// como alocados agora — sem coletar: só o que cabe antes do próximo
    /// gatilho (o que passa dele fica para o runtime, que coleta). Sem TLAB
    /// no `--gc-stress` (toda alocação passa pelo runtime, que coleta antes)
    /// e nos heaps de teste.
    pub fn reabastecer_tlab(&mut self, palavras: usize) {
        let n = palavras;
        if !self.publica || self.stress || self.agenda.is_some() || n == 0 || n > TLAB_N {
            return;
        }
        let tamanho = bytes_do_bloco(n);
        if CONTEXTO.with(|c| (c.tlab[n][1].get() as usize).saturating_sub(c.tlab[n][0].get() as usize) >= tamanho) {
            return;
        }
        let mut k = TLAB_BLOCOS
            .min(CONTAGEM_JOVEM.saturating_sub(self.allocations))
            .min(self.limite_jovem.saturating_sub(self.bytes_jovens) / tamanho)
            .min(self.byte_threshold.saturating_sub(self.stats.estimated_bytes) / tamanho);
        if self.limite_bytes != usize::MAX {
            k = k.min(self.limite_bytes.saturating_sub(self.bytes_totais(0)) / tamanho);
        }
        if k == 0 {
            return;
        }
        let (inicio, k) = self.objetos.tirar_faixa(n, k);
        self.objetos.vivos += k;
        self.contar_alocacao(k, k * tamanho);
        CONTEXTO.with(|c| {
            c.tlab[n][0].set(inicio);
            c.tlab[n][1].set(inicio.wrapping_add(k * tamanho));
        });
    }
    /// Devolve as TLABs do [`Contexto`] (o que o código gerado não usou) antes
    /// de uma coleta: o resto de cada faixa volta à frente da lista livre
    /// e sai da contagem de alocações.
    fn devolver_tlabs(&mut self) {
        if !self.publica {
            return;
        }
        for n in 1..=TLAB_N {
            let (cursor, fim) = CONTEXTO.with(|c| (c.tlab[n][0].replace(std::ptr::null_mut()), c.tlab[n][1].replace(std::ptr::null_mut())));
            let tamanho = bytes_do_bloco(n);
            let k = (fim as usize).saturating_sub(cursor as usize) / tamanho;
            if k > 0 {
                self.objetos.devolver_faixa(n, cursor, fim);
                let bytes = k * tamanho;
                self.objetos.vivos -= k;
                self.allocations = self.allocations.saturating_sub(k);
                self.stats.allocations = self.stats.allocations.saturating_sub(k as u64);
                self.stats.estimated_bytes = self.stats.estimated_bytes.saturating_sub(bytes);
                self.bytes_jovens = self.bytes_jovens.saturating_sub(bytes);
            }
        }
    }
    /// O bloco vivo do objeto `h` (`h & 3 == 2`), ou `panic` como
    /// [`Heap::indice_vivo`] (N4).
    ///
    /// Um objeto estático (seção da imagem, `PERMANENTE`) também é devolvido:
    /// ele se lê como qualquer bloco; quem grava confere
    /// ([`Heap::conferir_gravavel`]).
    ///
    /// As falhas têm causas diferentes e a mensagem é a chave de agrupamento do
    /// harness (docs/NATIVO-PLANO.md §6.4, N4): texto fixo na primeira linha e
    /// o detalhe na segunda — null desreferenciado, `Smi` ou escalar negativo
    /// usado como handle, handle fora do espaço (escalar positivo) e objeto já
    /// coletado (raiz faltando).
    #[inline(always)]
    fn bloco_vivo(&self, handle: i64) -> *mut Cabecalho {
        if !self.validar_handles && e_objeto(handle) {
            let b = (handle - DESLOCAMENTO_DO_HANDLE) as *mut Cabecalho;
            // SAFETY: sem a validação, um handle de objeto é um bloco do espaço
            // ou um estático (`validar_handles`).
            #[allow(unsafe_code)]
            if unsafe { (*b).estado } != LIVRE {
                return b;
            }
        }
        self.bloco_vivo_conferido(handle)
    }
    /// [`Heap::bloco_vivo`] pelo mapa de páginas, com as mensagens N4 (o
    /// caminho de `validar_handles` e o das falhas).
    #[cold]
    #[inline(never)]
    fn bloco_vivo_conferido(&self, handle: i64) -> *mut Cabecalho {
        if !e_objeto(handle) {
            Self::handle_invalido(handle);
        }
        match self.objetos.bloco_de(handle) {
            // SAFETY: bloco de uma página do espaço.
            #[allow(unsafe_code)]
            Some(b) if unsafe { (*b).estado } == crate::espaco::ESTADO_DE_VENENO => {
                panic!("bug do compilador: handle já coletado (raiz faltando; bloco envenenado, DARTFORGE_GC_VENENO)\nhandle {handle}")
            }
            // SAFETY: bloco de uma página do espaço.
            #[allow(unsafe_code)]
            Some(b) if unsafe { (*b).estado } != 0 => b,
            Some(_) => panic!("bug do compilador: handle já coletado (raiz faltando)\nhandle {handle}"),
            None if e_estatico(handle) => (handle - DESLOCAMENTO_DO_HANDLE) as *mut Cabecalho,
            None if self.objetos.na_pagina(handle) => {
                panic!("bug do compilador: handle já coletado (raiz faltando)\nhandle {handle}")
            }
            None => panic!("bug do compilador: handle além da tabela (escalar usado como handle)\nhandle {handle}"),
        }
    }
    /// O bloco de `h` se é objeto do espaço (vivo ou `LIVRE`; nunca um
    /// estático): o mapa de páginas com `validar_handles`, senão o próprio
    /// endereço.
    #[inline]
    fn bloco_do_espaco(&self, h: i64) -> Option<*mut Cabecalho> {
        if self.validar_handles {
            return self.objetos.bloco_de(h);
        }
        if !e_objeto(h) {
            return None;
        }
        let b = (h - DESLOCAMENTO_DO_HANDLE) as *mut Cabecalho;
        // SAFETY: `validar_handles` (um handle de objeto é um bloco ou um estático).
        #[allow(unsafe_code)]
        (unsafe { (*b).estado } != PERMANENTE).then_some(b)
    }
    /// O filho `v` gravado num velho pede a barreira (§2.7)? Só um objeto jovem:
    /// null, `Smi`, velho e estático não pedem.
    #[inline]
    fn filho_jovem(&self, v: Ref) -> bool {
        if !smi::e_handle(v) || v < 0 {
            return false;
        }
        // SAFETY: bloco do espaço.
        #[allow(unsafe_code)]
        self.bloco_do_espaco(v).is_some_and(|b| unsafe { (*b).estado } == JOVEM)
    }
    /// O pânico N4 de um handle que não é de objeto.
    #[cold]
    #[inline(never)]
    fn handle_invalido(handle: i64) -> ! {
        if handle == 0 {
            panic!("bug do compilador: handle null (0) desreferenciado");
        }
        if smi::e_smi(handle) {
            panic!(
                "bug do compilador: Smi usado como handle (int em posição Ref lido como objeto)\nSmi {}",
                smi::valor(handle)
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
    fn conferir_gravavel(b: *const Cabecalho) {
        // SAFETY: bloco legível (de `bloco_vivo`).
        assert!(unsafe { (*b).estado } != PERMANENTE, "bug do compilador: gravação num objeto estático");
    }
    /// O `hashCode` de identidade do objeto `h` do espaço: tirado do
    /// endereço, que não muda enquanto o objeto vive (o coletor não move) e
    /// é único entre os vivos — 30 bits espalhados, nunca 0 (a VM sorteia e
    /// guarda no cabeçalho; aqui o cabeçalho não tem lugar e não precisa).
    /// `None` se `h` não é objeto vivo do espaço.
    pub fn hash_de_identidade(&self, h: i64) -> Option<i64> {
        let b = match self.bloco_do_espaco(h) {
            Some(b) => b,
            None if self.e_estatico(h) => (h - DESLOCAMENTO_DO_HANDLE) as *mut Cabecalho,
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
    pub fn objeto(&self, h: i64) -> Option<Obj<'_>> {
        let b = self.bloco_do_espaco(h)?;
        // SAFETY: bloco de uma página do espaço, conferido vivo.
        #[allow(unsafe_code)]
        unsafe {
            // O mapa só é mapa de referências em `INSTANCIA` (§2.3).
            if (*b).estado == LIVRE || forma(b) != flags_do_bloco::INSTANCIA {
                return None;
            }
            Some(Obj { corpo: corpo(b), class_id: i64::from((*b).class_id), _heap: std::marker::PhantomData })
        }
    }
    /// A classe do objeto `h` do espaço (`None` se não é objeto vivo).
    #[inline]
    pub fn classe_do_objeto(&self, h: i64) -> Option<i64> {
        self.objeto(h).map(|o| o.class_id)
    }
    /// Grava o campo `i` do objeto `h` (com a marca de referência e a
    /// barreira de escrita).
    ///
    /// # Panics
    /// Se `h` não é objeto vivo ou `i` está fora dele.
    pub fn definir_campo(&mut self, h: i64, i: usize, bits: i64, e_ref: bool) {
        if e_ref && smi::e_handle(bits) {
            self.conferir_vivo(bits);
        }
        let b = self.bloco_vivo(h);
        Self::conferir_gravavel(b);
        // SAFETY: bloco vivo.
        #[allow(unsafe_code)]
        let instancia = unsafe { forma(b) } == flags_do_bloco::INSTANCIA;
        assert!(instancia, "bug do runtime: definir_campo num objeto que não é INSTANCIA");
        self.objetos.lembrar(b);
        // SAFETY: bloco vivo; `i` conferido contra o número de campos.
        #[allow(unsafe_code)]
        let antigo = unsafe {
            let c = corpo(b);
            assert!(i < usize::from((*c).n), "campo {i} fora do objeto de {} campos", (*c).n);
            let antigo = if e_referencia(c, i) { *campos_de(c).add(i) } else { 0 };
            *campos_de(c).add(i) = bits;
            marcar_referencia(c, i, e_ref);
            antigo
        };
        if self.arc.is_some() {
            self.arc_trocou(b, h, antigo, if e_ref { bits } else { 0 });
        }
    }
    /// Pânico (N4) se `h` não é handle vivo.
    fn conferir_vivo(&self, h: i64) {
        self.bloco_vivo(h);
    }
    /// A barreira de escrita do código gerado (`dartforge_lembrar`): o
    /// objeto `h`, velho, recebeu um `Ref` num campo.
    pub fn lembrar_objeto(&mut self, h: i64) {
        if let Some(b) = self.bloco_do_espaco(h) {
            self.objetos.lembrar(b);
        }
    }
    /// O metadado de um handle vivo (0 = nenhum).
    pub fn metadado(&self, handle: i64) -> i64 {
        // SAFETY: bloco vivo (ou estático).
        #[allow(unsafe_code)]
        i64::from(unsafe { (*self.bloco_vivo(handle)).metadado })
    }
    /// Grava o metadado de um handle vivo.
    pub fn set_metadado(&mut self, handle: i64, valor: i64) {
        let b = self.bloco_vivo(handle);
        Self::conferir_gravavel(b);
        // SAFETY: bloco vivo.
        #[allow(unsafe_code)]
        unsafe {
            (*b).metadado = u32::try_from(valor).expect("metadado além de 32 bits")
        };
    }
    /// Migra os objetos das classes de `plano` para o layout novo (J03):
    /// cada posição nova recebe o valor da antiga indicada (`-1`: zero, que
    /// é `null` para o campo anulável e "não inicializado" para o `late`), e
    /// as marcas de `late` inicializado seguem o campo.
    pub fn migrar_instancias(&mut self, plano: &crate::hash::HashMap<i64, Vec<i64>>) {
        let mut blocos = Vec::new();
        self.objetos.para_cada_vivo(|b| blocos.push(b));
        for b in blocos {
            // SAFETY: bloco vivo do espaço.
            #[allow(unsafe_code)]
            let class_id = i64::from(unsafe { (*b).class_id });
            let Some(origem) = plano.get(&class_id) else { continue };
            let h = b as i64 + DESLOCAMENTO_DO_HANDLE;
            let antigos = self.objeto(h).expect("objeto vivo").to_vec();
            let valores: Vec<Campo> = origem
                .iter()
                .map(|&o| usize::try_from(o).ok().and_then(|o| antigos.get(o).copied()).unwrap_or((0, false)))
                .collect();
            self.trocar_campos(b, &valores);
        }
        let marcas: Vec<(i64, i64)> = self.campos_late_inicializados.iter().copied().filter(|&(_, i)| i >= 0).collect();
        for (h, i) in marcas {
            let Some(class_id) = self.classe_do_objeto(h) else { continue };
            let Some(origem) = plano.get(&class_id) else { continue };
            let nova = origem.iter().position(|&o| o == i);
            self.campos_late_inicializados.remove(&(h, i));
            if let Some(j) = nova {
                self.marcar_late((h, j as i64));
            }
        }
    }

    /// Troca os campos do objeto do bloco `b` por `valores`: no próprio bloco
    /// se o número de campos é o dele, senão num corpo de fora (o objeto não
    /// muda de endereço; o código gerado segue o corpo, [`FORA`]).
    fn trocar_campos(&mut self, b: *mut Cabecalho, valores: &[Campo]) {
        self.objetos.lembrar(b);
        // No ARC, a migração (a recarga do JIT) e o campo acrescentado são
        // uma escrita crua: a foto conta os campos de antes e os de depois.
        if self.arc.is_some() {
            self.arc_foto(b, b as i64 + DESLOCAMENTO_DO_HANDLE);
        }
        // SAFETY: bloco vivo do espaço; o corpo de fora é novo e do tamanho
        // de `valores`.
        #[allow(unsafe_code)]
        unsafe {
            let atual = corpo(b);
            let c = if usize::from((*atual).n) == valores.len() {
                atual
            } else {
                let c = novo_corpo_de_fora(valores.len());
                (*c).class_id = (*b).class_id;
                if (*b).flags & FORA != 0 {
                    soltar_corpo_de_fora(atual);
                } else {
                    self.objetos.com_fora.push(b);
                }
                *campos_de(b).cast::<*mut Cabecalho>() = c;
                (*b).flags |= FORA;
                c
            };
            for (i, &(bits, e_ref)) in valores.iter().enumerate() {
                *campos_de(c).add(i) = bits;
                marcar_referencia(c, i, e_ref);
            }
        }
    }

    /// Garante ao objeto `handle` pelo menos `n` campos (os novos,
    /// zerados): num corpo de fora ([`Heap::trocar_campos`]).
    pub fn garantir_campos(&mut self, handle: i64, n: usize) {
        let Some(o) = self.objeto(handle) else { return };
        if o.len() >= n {
            return;
        }
        let mut novos = o.to_vec();
        novos.resize(n, (0, false));
        let b = self.bloco_vivo(handle);
        self.trocar_campos(b, &novos);
    }
    /// O endereço dos campos do objeto `handle` (palavras de 8 bytes; o
    /// caminho de `dartforge_object_campos`); `None` se não é objeto vivo.
    /// Quem pede o ponteiro pode gravar por ele: conta como barreira de
    /// escrita, mas o mapa de referências fica com quem grava.
    #[inline]
    pub fn campos_de_objeto(&mut self, handle: i64) -> Option<*mut i64> {
        let b = self.bloco_do_espaco(handle)?;
        // SAFETY: bloco do espaço.
        #[allow(unsafe_code)]
        unsafe {
            if (*b).estado == LIVRE {
                return None;
            }
            self.objetos.lembrar(b);
            if self.arc.is_some() {
                self.arc_foto(b, handle);
            }
            Some(campos_de(corpo(b)))
        }
    }
    /// Atualiza campo com tag explícita; valores escalares jamais são raízes.
    pub fn set(&mut self, handle: i64, index: i64, bits: i64, is_ref: bool) {
        self.definir_campo(handle, usize::try_from(index).expect("índice inválido"), bits, is_ref);
    }
    /// Marca raízes e arestas tipadas iterativamente e libera inclusive ciclos inalcançáveis.
    /// Marca tudo o que é alcançável a partir de `pending`; devolve quantos
    /// objetos marcou.
    fn marcar_pendentes(&mut self) -> usize {
        // Uma cópia da marcação para cada tipo de coleta: o teste do estado
        // de cada objeto (jovem na menor, não marcado na completa) sai sem
        // desvio pelo tipo.
        if self.coleta_menor { self.marcar::<true>() } else { self.marcar::<false>() }
    }

    fn marcar<const MENOR: bool>(&mut self) -> usize {
        let menor = MENOR;
        let mut live = 0_usize;
        // A pilha e os contadores em variáveis locais: gravar em `self` a
        // cada objeto (o `Vec` e o trabalho) impedia o compilador de os
        // manter em registradores (as gravações nos objetos podem, para
        // ele, apontar para `self`).
        let mut pilha = std::mem::take(&mut self.pending);
        let mut trabalho = 0usize;
        let mut objetos_marcados = 0usize;
        // As arestas vêm de raízes e de campos `is_ref`: a validação
        // completa do handle (a página e o início do bloco) fica para o
        // `--gc-stress`, o modo que caça raiz faltando (N4).
        let validar = self.stress;
        // Esta coleta percorre o objeto no estado `e`? Na menor, só o jovem
        // (um velho conta como vivo, e as referências dele a jovens estão
        // nos lembrados); na completa, o que ainda não foi marcado.
        // Na completa, o marcado é o do bit no mapa de marcas da página (a
        // varredura lê só os bits).
        // SAFETY (dos usos): `b` é bloco de uma página do espaço.
        #[allow(unsafe_code)]
        // O estático (`PERMANENTE`) é conferido antes do mapa de marcas: o
        // mapa dele cairia fora de uma página do heap (§2.8).
        let percorre = |b: *const Cabecalho| unsafe {
            if menor { (*b).estado == JOVEM } else { (*b).estado != PERMANENTE && !marcado(b) }
        };
        // Uma aresta: o primeiro filho objeto ainda por marcar vai para
        // `proximo` (seguido sem a pilha); os demais, para a pilha.
        #[allow(unsafe_code)]
        let visitar_ref = |bits: i64, proximo: &mut i64, pilha: &mut Vec<i64>| {
            if !smi::e_handle(bits) {
                return;
            }
            if e_objeto(bits) {
                if !validar && !percorre((bits - DESLOCAMENTO_DO_HANDLE) as *const Cabecalho) {
                    return;
                }
                if *proximo == 0 {
                    *proximo = bits;
                    return;
                }
            }
            pilha.push(bits);
        };
        while let Some(handle) = pilha.pop() {
            // null e `Smi` (R10) não são arestas: o coletor nunca segue um
            // `Smi`, que não aponta para o heap.
            if !smi::e_handle(handle) {
                continue;
            }
            if e_objeto(handle) {
                let mut atual = handle;
                // Segue direto o primeiro filho ainda por marcar (a lista
                // ligada, o ramo de uma árvore) sem passar pela pilha.
                while atual != 0 {
                    if validar && self.objetos.bloco_de(atual).is_none() && e_estatico(atual) {
                        break;
                    }
                    let b = if validar {
                        self.bloco_vivo(atual)
                    } else {
                        debug_assert!(self.objetos.bloco_de(atual).is_some(), "aresta inválida {atual}");
                        (atual - DESLOCAMENTO_DO_HANDLE) as *mut Cabecalho
                    };
                    let mut proximo = 0;
                    // SAFETY: bloco vivo do espaço.
                    #[allow(unsafe_code)]
                    unsafe {
                        if !percorre(b) {
                            break;
                        }
                        // As duas promovem já na marcação (o jovem alcançado
                        // fica velho, o lembrado volta a velho na completa:
                        // a varredura só lê os bits) e acendem o bit.
                        if (*b).estado != VELHO {
                            (*b).estado = VELHO;
                        }
                        marcar_bloco(b);
                        live += 1;
                        objetos_marcados += 1;
                        // O corpo pelo formato (§2.8): `BRUTO` não tem
                        // referências; `REFS`, as palavras `1..=palavra 0`;
                        // `INSTANCIA`, os campos que o mapa diz referência. O
                        // que esta coleta não percorre (velho na menor, já
                        // marcado) nem entra na pilha; o primeiro que falta
                        // marcar é seguido direto.
                        let tipo = forma(b);
                        if tipo != flags_do_bloco::INSTANCIA {
                            if tipo == flags_do_bloco::REFS {
                                let w = self.objetos.palavras_do_corpo(b);
                                let n = refs_do_corpo(b, w);
                                trabalho += n;
                                let p = campos_de(b);
                                for i in 1..=n {
                                    visitar_ref(*p.add(i), &mut proximo, &mut pilha);
                                }
                                if !menor && (*b).flags & flags_do_bloco::CARTOES != 0 {
                                    crate::espaco::zerar_cartoes(b, n);
                                }
                            }
                            atual = proximo;
                            continue;
                        }
                        let c = corpo(b);
                        let n = usize::from((*c).n);
                        trabalho += n;
                        let campos = campos_de(c);
                        let visitar = visitar_ref;
                        let mut m = (*c).mapa;
                        while m != 0 {
                            let i = m.trailing_zeros() as usize;
                            visitar(*campos.add(i), &mut proximo, &mut pilha);
                            m &= m - 1;
                        }
                        if n > 32 {
                            let ext = campos.add(capacidade(n)).cast::<u64>();
                            for w in 0..palavras_do_mapa(n) {
                                let mut m = *ext.add(w);
                                while m != 0 {
                                    let i = 32 + w * 64 + m.trailing_zeros() as usize;
                                    visitar(*campos.add(i), &mut proximo, &mut pilha);
                                    m &= m - 1;
                                }
                            }
                        }
                    }
                    atual = proximo;
                }
                continue;
            }
            // Raiz ou aresta que não é handle de objeto: as mensagens N4.
            Self::handle_invalido(handle);
        }
        self.pending = pilha;
        self.trabalho_da_marcacao += trabalho;
        self.objetos.marcados += objetos_marcados;
        live
    }

    /// As posições de referência forte do objeto `h`, pela forma do corpo, as
    /// mesmas que a marcação percorre (§2.8): `REFS`, as palavras
    /// `1..=palavra 0`; `INSTANCIA`, os campos do mapa (e do mapa estendido);
    /// `BRUTO`, nenhuma. Um estático ou um bloco livre não tem arestas aqui (o
    /// estático é imortal no ARC, docs/ARC-CICLOS-ESPECIFICACAO.md §19.1).
    fn posicoes_de_ref(&self, h: i64, f: &mut dyn FnMut(*mut i64)) {
        let Some(b) = self.bloco_do_espaco(h) else { return };
        // SAFETY: bloco do espaço; só os vivos são percorridos.
        #[allow(unsafe_code)]
        unsafe {
            if (*b).estado == LIVRE {
                return;
            }
            let tipo = forma(b);
            if tipo == flags_do_bloco::REFS {
                let w = self.objetos.palavras_do_corpo(b);
                let n = refs_do_corpo(b, w);
                let p = campos_de(b);
                for i in 1..=n {
                    f(p.add(i));
                }
                return;
            }
            if tipo != flags_do_bloco::INSTANCIA {
                return;
            }
            let c = corpo(b);
            let n = usize::from((*c).n);
            let campos = campos_de(c);
            let mut m = (*c).mapa;
            while m != 0 {
                f(campos.add(m.trailing_zeros() as usize));
                m &= m - 1;
            }
            if n > 32 {
                let ext = campos.add(capacidade(n)).cast::<u64>();
                for w in 0..palavras_do_mapa(n) {
                    let mut m = *ext.add(w);
                    while m != 0 {
                        f(campos.add(32 + w * 64 + m.trailing_zeros() as usize));
                        m &= m - 1;
                    }
                }
            }
        }
    }

    /// A coleta completa (o `dartforge_gc_collect` e os testes): marca a
    /// partir das raízes e varre tudo; o que sobrevive fica velho.
    pub fn collect(&mut self) {
        self.coletar(false);
    }

    /// Empilha em `destino` as raízes: as permanentes do runtime, os
    /// globais, os anexos de finalizador e os quadros (os do runtime e a
    /// pilha-sombra do código gerado).
    fn raizes(&self, destino: &mut Vec<i64>) {
        destino.extend(self.owners_mensagens.values().copied());
        destino.extend(self.enum_values.values().copied());
        destino.extend(self.tearoffs.values().copied());
        destino.extend(self.literais.values().copied());
        destino.extend(self.globais.values().copied());
        destino.extend(self.raizes_do_runtime.iter().copied().filter(|&h| h != 0));
        destino.extend(self.finalizacoes_prontas.iter().copied());
        for a in &self.anexos {
            destino.push(a.dono);
            if let AcaoDeFinalizador::Dart(acao) = a.acao {
                destino.push(acao);
            }
        }
        destino.extend(self.frames.iter().flat_map(|(_, roots, _)| roots.iter().copied()));
        visitar_quadros(|h| destino.push(h));
        let dos_mapas = destino.len();
        visitar_quadros_por_mapas(|h| destino.push(h));
        if percurso_conferido() {
            conferir_percurso(&destino[dos_mapas..]);
        }
    }

    /// `DARTFORGE_GC_VERIFICAR=1`, depois da marcação de uma coleta menor:
    /// uma travessia completa a partir das raízes, sem gerações, não pode
    /// achar jovem sem marca — seria uma referência de velho para jovem que
    /// a barreira de escrita não lembrou.
    ///
    /// # Panics
    /// Com a primeira referência assim.
    fn verificar_coleta_menor(&self) {
        let mut pilha = Vec::new();
        self.raizes(&mut pilha);
        let mut visto = crate::hash::HashSet::default();
        let mut origem: crate::hash::HashMap<i64, i64> = crate::hash::HashMap::default();
        while let Some(h) = pilha.pop() {
            if !smi::e_handle(h) || h < 0 || !visto.insert(h) {
                continue;
            }
            let antes = pilha.len();
            if e_objeto(h) {
                if self.objetos.bloco_de(h).is_none() && e_estatico(h) {
                    continue;
                }
                let b = self.bloco_vivo(h);
                // SAFETY: bloco vivo do espaço.
                #[allow(unsafe_code)]
                let estado = unsafe { (*b).estado };
                if estado == JOVEM {
                    // O pai: cid, estado, flags e a posição do filho no corpo.
                    let pai = origem.get(&h).copied();
                    let desc = |x: i64| {
                        let c = self.cabecalho(x);
                        let pos = self.palavras(x).iter().position(|&w| w == h);
                        format!("cid {} estado {} flags {:#x} n {} posição {pos:?}", c.class_id, c.estado, c.flags, c.n)
                    };
                    panic!(
                        "bug do coletor: jovem alcançável sem marca na coleta menor (barreira de escrita faltando)\nobjeto {h} (cid {}), alcançado de {pai:?} ({})",
                        self.cabecalho(h).class_id,
                        pai.map(desc).unwrap_or_default()
                    );
                }
                // SAFETY: bloco vivo do espaço.
                #[allow(unsafe_code)]
                unsafe {
                    self.objetos.empilhar_corpo(b, &mut pilha)
                };
            } else {
                Self::handle_invalido(h);
            }
            for &f in &pilha[antes..] {
                origem.entry(f).or_insert(h);
            }
        }
    }

    /// Uma coleta: `menor` marca e varre só os jovens (os velhos contam como
    /// vivos, e os lembrados pela barreira de escrita entram como raízes);
    /// a completa, tudo. Em ambas o que sobrevive fica velho.
    fn coletar(&mut self, menor: bool) {
        // O ARC puro não rastreia: a drenagem decide a vida pelo RC e pelas
        // raízes diretas (docs/ARC-IMPLEMENTACAO.md §6).
        if self.arc.as_ref().is_some_and(|a| a.puro) {
            self.drenar_arc(!menor);
            return;
        }
        conferir_coleta_permitida();
        // No ARC a marcação é sempre a menor (os jovens); a vida dos velhos é
        // o RC, e a completa pedida é a rodada de ciclos e a reclamação
        // (`sincronizar_arc`).
        let completa_arc = self.arc.is_some() && !menor;
        let menor = menor || self.arc.is_some();
        if let Some(a) = &self.agenda {
            a.pendente.set(false);
        }
        // O que a TLAB não usou volta a ser livre.
        self.devolver_tlabs();
        self.coleta_menor = menor;
        self.stats.collections += 1;
        self.stats.roots_scanned += self.enum_values.len() as u64;
        self.stats.roots_scanned += self.tearoffs.len() as u64;
        self.stats.roots_scanned += self.literais.len() as u64;
        self.stats.roots_scanned += self.owners_mensagens.len() as u64;
        self.stats.roots_scanned += self
            .frames
            .iter()
            .map(|(_, roots, _)| roots.len() as u64)
            .sum::<u64>();
        if !menor {
            self.objetos.limpar_marcas();
        }
        self.trabalho_da_marcacao = 0;
        let mut pendentes = std::mem::take(&mut self.pending);
        pendentes.clear();
        self.raizes(&mut pendentes);
        if menor {
            // Os velhos lembrados: as referências deles podem ser jovens.
            for &b in &self.objetos.lembrados {
                // SAFETY: bloco lembrado, vivo.
                #[allow(unsafe_code)]
                let trabalho = unsafe { self.objetos.empilhar_lembrado(b, &mut pendentes) };
                self.trabalho_da_marcacao += trabalho;
            }
        }
        self.pending = pendentes;
        let mut live = self.marcar_pendentes();
        // Os efêmeros: o valor de um portador vivo é alcançado quando a
        // chave é — o que pode tornar vivas outras chaves, até o ponto fixo.
        if !self.efemeros.is_empty() {
            loop {
                let objetos = &self.objetos;
                let marcado = |h: i64| marcado_na_coleta(objetos, menor, h);
                let antes = self.pending.len();
                for (&portador, &(chave, valor)) in &self.efemeros {
                    if marcado(portador) && (marcado(chave) || !smi::e_handle(chave)) && smi::e_handle(valor) && !marcado(valor) {
                        self.pending.push(valor);
                    }
                }
                if self.pending.len() == antes {
                    break;
                }
                live += self.marcar_pendentes();
            }
        }
        // Tabelas laterais: só ficam os handles que sobreviveram (G6).
        // A verificação percorre o heap vivo inteiro: no `--gc-stress` (uma
        // menor por alocação), espaçada como as completas, para o custo por
        // alocação não crescer com o heap.
        if menor
            && self.verificar
            && (!self.stress
                || self.menores_desde_completa % (self.trabalho_na_completa / TRABALHO_POR_MENOR_NO_ESTRESSE).max(1) as u64 == 0)
        {
            self.verificar_coleta_menor();
        }
        let promocao = if self.arc.is_some() { Some(self.dados_da_promocao()) } else { None };
        let objetos = &self.objetos;
        let vivo = |h: &i64| marcado_na_coleta(objetos, menor, *h);
        self.fracas.retain(|portador, alvo| {
            if smi::e_handle(*alvo) && !vivo(alvo) {
                *alvo = 0;
            }
            vivo(portador)
        });
        self.efemeros.retain(|portador, (chave, valor)| {
            if smi::e_handle(*chave) && !vivo(chave) {
                *chave = 0;
                *valor = 0;
            }
            vivo(portador)
        });
        // Na menor, só as marcas `late` gravadas desde a última coleta podem
        // ser de um jovem (os velhos não morrem nela): conferir só essas, e
        // não a tabela inteira (uma por objeto vivo com campo `late`).
        if menor {
            for chave in self.late_novos.drain(..) {
                if !vivo(&chave.0) {
                    self.campos_late_inicializados.remove(&chave);
                }
            }
        } else {
            self.late_novos.clear();
            self.campos_late_inicializados.retain(|(h, _)| vivo(h));
        }
        let mut prontas = Vec::new();
        let mut nativas = Vec::new();
        self.anexos.retain_mut(|a| {
            if smi::e_handle(a.desanexo) && !vivo(&a.desanexo) {
                a.desanexo = 0;
            }
            if vivo(&a.valor) {
                return true;
            }
            match a.acao {
                AcaoDeFinalizador::Dart(acao) => prontas.push(acao),
                AcaoDeFinalizador::Nativa(f, token) => nativas.push((f, token)),
            }
            false
        });
        if let Some(arc) = &mut self.arc {
            for &acao in &prontas {
                arc.estado.reter(acao).unwrap_or_else(|e| falha_do_arc(e));
            }
        }
        self.finalizacoes_prontas.extend(prontas);
        // As tabelas de identidade entre portas: purgadas pelo bit de marca
        // como as demais (quem precisa da constante viva a guarda num global,
        // o getter já grava lá; §2.8).
        self.permanentes.retain(|h| vivo(h));
        self.constantes.retain(|h, _| vivo(h));
        self.codigo_do_tearoff.retain(|h, _| vivo(h));
        let mut finalizar = Vec::new();
        self.finalizaveis.retain(|h, &mut par| {
            let fica = vivo(h);
            if !fica {
                finalizar.push(par);
            }
            fica
        });
        // A estimativa: a menor desconta o que soltou (blocos, regiões grandes,
        // corpos de fora e anexos dos mortos); a completa a refaz dos vivos, com
        // os bytes externos.
        if menor {
            let (mortos, bytes_soltos) = self.objetos.varrer_jovens();
            self.stats.reclaimed += mortos as u64;
            self.stats.estimated_bytes = self.stats.estimated_bytes.saturating_sub(bytes_soltos);
        } else {
            let (mortos, _, bytes_de_objetos) = self.objetos.varrer();
            self.stats.reclaimed += mortos as u64;
            self.stats.estimated_bytes = bytes_de_objetos.saturating_add(self.externos);
        }
        for (finalizador, par) in finalizar {
            finalizador(par);
        }
        for (f, token) in nativas {
            #[allow(unsafe_code)]
            // SAFETY: a `NativeFinalizerFunction` do anexo, `void f(void*)`;
            // como na VM, roda durante a coleta e não pode tocar o heap.
            let f: extern "C" fn(usize) = unsafe { std::mem::transmute(f) };
            f(token);
        }
        self.allocations = 0;
        self.bytes_jovens = 0;
        self.coleta_menor = false;
        if let Some((promovidos, lembrados)) = promocao {
            self.sincronizar_arc(completa_arc, promovidos, lembrados);
        } else if menor {
            self.menores_desde_completa += 1;
        } else {
            self.menores_desde_completa = 0;
            self.trabalho_na_completa = self.trabalho_da_marcacao + live;
            self.recalcular_gatilhos(live);
        }
        if self.memoria > 1 || (self.memoria == 1 && !menor) {
            eprintln!("[memória] depois da coleta {}", if menor { "menor" } else { "completa" });
            eprint!("{}", self.relatorio_de_memoria());
        }
        if self.rastrear {
            eprintln!(
                "[gc] {} marcados={live} trabalho={} estimados={} gatilho={} paginas={} vivos={}",
                if menor { "menor" } else { "completa" },
                self.trabalho_da_marcacao,
                self.stats.estimated_bytes,
                self.byte_threshold,
                self.objetos.n_paginas(),
                self.objetos.vivos
            );
        }
    }

    /// A quebra da memória (`DARTFORGE_GC_MEMORIA=1`): o espaço de objetos, as
    /// tabelas laterais do runtime (entradas e bytes de capacidade) e o
    /// processo (residente, pico, heap do sistema confirmado).
    pub fn relatorio_de_memoria(&self) -> String {
        use std::fmt::Write;
        fn tabela<K, V>(m: &crate::hash::HashMap<K, V>) -> (usize, usize) {
            (m.len(), m.capacity() * (std::mem::size_of::<(K, V)>() + 1) / 1024)
        }
        fn conjunto<K>(m: &crate::hash::HashSet<K>) -> (usize, usize) {
            (m.len(), m.capacity() * (std::mem::size_of::<K>() + 1) / 1024)
        }
        let mut s = self.objetos.relatorio_de_memoria();
        let literais: usize = self.literais.keys().map(|k| k.capacity() * 2).sum::<usize>() / 1024;
        let _ = writeln!(
            s,
            "[memória] tabelas (entradas, KiB): globais {:?}, literais {:?} (+{literais} KiB de chaves), constantes {:?}, permanentes {:?}, enum {:?}, tearoffs {:?}, código de tearoff {:?}, late {:?}, fracas {:?}, efêmeros {:?}, finalizáveis {:?}; estimados {} KiB, gatilho {} KiB",
            tabela(&self.globais),
            tabela(&self.literais),
            tabela(&self.constantes),
            conjunto(&self.permanentes),
            tabela(&self.enum_values),
            tabela(&self.tearoffs),
            tabela(&self.codigo_do_tearoff),
            conjunto(&self.campos_late_inicializados),
            tabela(&self.fracas),
            tabela(&self.efemeros),
            tabela(&self.finalizaveis),
            self.stats.estimated_bytes / 1024,
            self.byte_threshold / 1024
        );
        let (rss, pico, heap_do_sistema) = crate::espaco::memoria_do_processo();
        let _ = writeln!(
            s,
            "[memória] processo: residente {} KiB, pico {} KiB, heap do sistema confirmado {} KiB",
            rss / 1024,
            pico / 1024,
            heap_do_sistema / 1024
        );
        s
    }

    /// Os gatilhos da próxima coleta, a partir do que sobreviveu a esta.
    ///
    /// Sem teto, o heap cresce [`CRESCIMENTO`]% (o geométrico). Com teto, a
    /// próxima coleta vem quando se gastar metade da folga que resta — a
    /// histerese: uma coleta que achou quase tudo vivo não se repete na
    /// alocação seguinte (antes, passar da metade do teto coletava em TODA
    /// alocação, e um programa com muito dado vivo parava de andar). O passo
    /// mínimo garante progresso mesmo com a folga no fim; o teto em si é
    /// conferido à parte (`allocate`).
    fn recalcular_gatilhos(&mut self, vivos: usize) {
        const PASSO_MINIMO: usize = 256 * 1024;
        // [`CRESCIMENTO`]% do que sobreviveu, e não menos que três quartos do gatilho
        // anterior (a histerese: uma estrutura grande que morre e volta — a
        // lista refeita a cada rodada — não faz o gatilho despencar e voltar
        // a subir com uma coleta completa a cada dobra).
        // E pelo menos dois semiespaços jovens acima do que sobreviveu: com
        // pouco dado vivo, o dobro dele vinha antes do gatilho da coleta
        // menor, e toda coleta era completa (a árvore longa de
        // `objetos_escapam/arvores` remarcada 250 vezes).
        let crescimento = (self
            .stats
            .estimated_bytes
            .saturating_mul(self.crescimento))
            / 100;
        let crescimento = crescimento
            .max(self.stats.estimated_bytes.saturating_add(2 * self.limite_jovem))
            .max(self.byte_threshold - self.byte_threshold / 4);
        self.byte_threshold = if self.limite_bytes == usize::MAX {
            crescimento
        } else {
            let folga = self.limite_bytes.saturating_sub(self.bytes_totais(0));
            crescimento.min(self.stats.estimated_bytes.saturating_add((folga / 2).max(PASSO_MINIMO)))
        };
        // Não há mais gatilho por contagem para a coleta completa: ele
        // existia porque toda coleta varria a tabela de slots inteira, e com
        // poucos vivos coletava a cada 256 alocações (um `sort` de 100 mil
        // `int` com alguns `_Mint` percorria a lista toda a cada vez). A
        // coleta menor varre só os jovens e vem por bytes ou por
        // `CONTAGEM_JOVEM` alocações; a completa, pelos bytes.
        let _ = vivos;
    }

    /// Obtém contadores sem percorrer os objetos ou suas raízes.
    pub fn stats(&self) -> HeapStats {
        HeapStats {
            live_objects: self.objetos.vivos,
            reserved_slots: 0,
            permanent_roots: self.enum_values.len()
                + self.tearoffs.len()
                + self.literais.len(),
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
pub enum Valor {
    Ref(Ref),
    Int(i64),
    Double(f64),
    Bool(bool),
}

/// As caixas estáticas de `false` e `true` (cid 5, `BRUTO`, `PERMANENTE`): o
/// cabeçalho e o valor, na forma de um bloco; o handle é o endereço + 2. O
/// código gerado as referencia como `@dartforge_falso`/`@dartforge_verdadeiro`
/// (`llvm/externs.rs`). São estáticos do runtime (§2.11): nunca marcados,
/// varridos nem gravados; [`e_estatico`] os reconhece pelo endereço.
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

/// As faixas `[início, fim)` da seção de objetos estáticos (`.dfimg`) de cada
/// imagem carregada (o executável, a DLL do SDK): registradas por
/// `dartforge_registrar_imagem`, que o `@df.preparar_isolado` de cada imagem
/// chama. Um handle de objeto fora das páginas do heap só é aceito como
/// estático dentro de uma delas (a validação do `--gc-stress` e da verificação,
/// e a leitura do runtime); os estáticos são do processo, compartilhados por
/// todos os isolados.
static IMAGENS: std::sync::RwLock<Vec<(usize, usize)>> = std::sync::RwLock::new(Vec::new());

/// Registra a seção de estáticos `[inicio, fim)` de uma imagem (idempotente).
pub fn registrar_imagem(inicio: usize, fim: usize) {
    if inicio >= fim {
        return;
    }
    let mut v = IMAGENS.write().unwrap_or_else(std::sync::PoisonError::into_inner);
    if !v.contains(&(inicio, fim)) {
        v.push((inicio, fim));
    }
}

/// As faixas das imagens registradas, na ordem do registro (o índice dos
/// literais estáticos, `textos.rs`).
pub fn imagens() -> Vec<(usize, usize)> {
    IMAGENS.read().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
}

/// `h` é um objeto estático (`PERMANENTE`): uma das caixas de `bool` do runtime
/// ou um bloco da seção de estáticos de uma imagem registrada? Nunca lê memória
/// fora dessas faixas.
#[allow(unsafe_code)]
pub fn e_estatico(h: Ref) -> bool {
    if !e_objeto(h) {
        return false;
    }
    let b = (h - DESLOCAMENTO_DO_HANDLE) as usize;
    let do_runtime = b == std::ptr::addr_of!(dartforge_falso) as usize || b == std::ptr::addr_of!(dartforge_verdadeiro) as usize;
    let na_imagem = do_runtime || {
        let v = IMAGENS.read().unwrap_or_else(std::sync::PoisonError::into_inner);
        v.iter().any(|&(i, f)| i <= b && b + std::mem::size_of::<Cabecalho>() <= f)
    };
    // SAFETY: `b` está numa faixa registrada (memória legível da imagem) ou é
    // uma das caixas do runtime.
    na_imagem && unsafe { (*(b as *const Cabecalho)).estado } == PERMANENTE
}


impl Heap {
    /// Um bloco zerado de `palavras` palavras de corpo, classe `cid` e `flags`
    /// (formato, forma compacta, cartões…), estado `JOVEM`; coleta antes se um
    /// gatilho pede (as referências que o chamador segura precisam de raiz). O `n`
    /// do cabeçalho é `palavras` (saturado): para `INSTANCIA`, use
    /// [`Heap::alocar_instancia`]. Quem aloca uma `_List` geral grande acende
    /// `CARTOES` (`layout::tem_cartoes`) e reserva as palavras deles
    /// (`layout::palavras_de_lista`).
    pub fn alocar(&mut self, cid: i32, palavras: usize, flags: u8) -> Ref {
        debug_assert!(
            flags & flags_do_bloco::FORMA != flags_do_bloco::INSTANCIA || palavras <= 32,
            "INSTANCIA com mapa estendido por `alocar`: use `alocar_instancia`"
        );
        self.alocar_bloco(cid, palavras, flags, palavras)
    }
    /// Um objeto `INSTANCIA` de `campos` campos zerados (o `alocar_objeto` de hoje).
    pub fn alocar_instancia(&mut self, cid: i32, campos: usize) -> Ref {
        self.alocar_objeto(i64::from(cid), campos)
    }
    /// O cabeçalho do bloco `h` (também de um estático). Pânico N4 para null,
    /// `Smi`, morto ou inválido.
    #[inline]
    #[allow(unsafe_code)]
    pub fn cabecalho(&self, h: Ref) -> &Cabecalho {
        let b = self.bloco_vivo(h);
        // SAFETY: bloco vivo de uma página do espaço (ou estático); o
        // empréstimo do heap o mantém (nenhuma coleta com `&self`).
        unsafe { &*b }
    }
    /// A classe de `h`: 1 para null, 2 para `Smi`, o cid do cabeçalho para um
    /// objeto. Pânico N4 para morto ou inválido.
    pub fn classe(&self, h: Ref) -> i32 {
        if h == 0 {
            crate::layout::cid::NULL
        } else if smi::e_smi(h) {
            crate::layout::cid::SMI
        } else if e_objeto(h) {
            self.cabecalho(h).class_id
        } else {
            Self::handle_invalido(h)
        }
    }
    /// `h` é um objeto vivo do espaço, ou um estático (sem pânico)?
    #[allow(unsafe_code)]
    pub fn e_objeto_vivo(&self, h: Ref) -> bool {
        if !self.validar_handles {
            // SAFETY: `validar_handles` (um handle de objeto é um bloco ou um estático).
            return e_objeto(h) && unsafe { (*((h - DESLOCAMENTO_DO_HANDLE) as *const Cabecalho)).estado } != LIVRE;
        }
        match self.objetos.bloco_de(h) {
            // SAFETY: bloco de uma página do espaço.
            Some(b) => (unsafe { (*b).estado }) != LIVRE,
            None => e_estatico(h),
        }
    }
    /// `h` é um objeto estático (seção da imagem, `PERMANENTE`)?
    #[allow(unsafe_code)]
    pub fn e_estatico(&self, h: Ref) -> bool {
        if !self.validar_handles {
            // SAFETY: `validar_handles` (um handle de objeto é um bloco ou um estático).
            return e_objeto(h) && unsafe { (*((h - DESLOCAMENTO_DO_HANDLE) as *const Cabecalho)).estado } == PERMANENTE;
        }
        self.objetos.bloco_de(h).is_none() && e_estatico(h)
    }
    /// As palavras do corpo do bloco `b` (legível): o corpo `INSTANCIA` (o de
    /// fora, se houver) e o número de palavras dele.
    #[inline]
    #[allow(unsafe_code)]
    fn corpo_e_palavras(&self, b: *mut Cabecalho) -> (*mut Cabecalho, usize) {
        // SAFETY: bloco legível (de `bloco_vivo`).
        unsafe {
            if forma(b) == flags_do_bloco::INSTANCIA {
                let c = corpo(b);
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
                assert!(crate::layout::cid::e_texto(c), "bug do compilador: estático grande que não é string");
                let len = usize::try_from(*campos_de(b)).unwrap_or(0);
                return (b, crate::layout::palavras_de_texto(len, c == crate::layout::cid::TWO_BYTE_STRING));
            }
            (b, self.objetos.palavras_do_corpo(b))
        }
    }
    /// O corpo inteiro de `h`, em palavras (a palavra 0 é `b+16`).
    #[inline]
    #[allow(unsafe_code)]
    pub fn palavras(&self, h: Ref) -> &[i64] {
        let (c, w) = self.corpo_e_palavras(self.bloco_vivo(h));
        // SAFETY: o corpo do bloco vivo tem `w` palavras; o empréstimo do heap
        // o mantém.
        unsafe { std::slice::from_raw_parts(campos_de(c), w) }
    }
    /// O corpo inteiro de `h`, gravável **sem barreira**: só `BRUTO`, ou antes de
    /// publicar o objeto (quem grava um `Ref` num objeto publicado chama
    /// [`Heap::lembrar`]/[`Heap::lembrar_elemento`]).
    #[inline]
    #[allow(unsafe_code)]
    pub fn palavras_mut(&mut self, h: Ref) -> &mut [i64] {
        let b = self.bloco_vivo(h);
        Self::conferir_gravavel(b);
        if self.arc.is_some() {
            self.arc_foto(b, h);
        }
        let (c, w) = self.corpo_e_palavras(b);
        // SAFETY: o corpo do bloco vivo tem `w` palavras; o empréstimo mutável
        // do heap é exclusivo.
        unsafe { std::slice::from_raw_parts_mut(campos_de(c), w) }
    }
    /// O corpo inteiro de `h`, em bytes.
    #[allow(unsafe_code)]
    pub fn bytes(&self, h: Ref) -> &[u8] {
        let p = self.palavras(h);
        // SAFETY: as mesmas palavras, vistas como bytes.
        unsafe { std::slice::from_raw_parts(p.as_ptr().cast(), p.len() * 8) }
    }
    /// O corpo inteiro de `h`, em bytes graváveis (as regras de [`Heap::palavras_mut`]).
    #[allow(unsafe_code)]
    pub fn bytes_mut(&mut self, h: Ref) -> &mut [u8] {
        let p = self.palavras_mut(h);
        // SAFETY: as mesmas palavras, vistas como bytes.
        unsafe { std::slice::from_raw_parts_mut(p.as_mut_ptr().cast(), p.len() * 8) }
    }
    /// Troca os bits `mascara` de `flags` por `valor`: só `FORMA`/`ELEMENTO` de uma
    /// `_List`/`_ImmutableList` (§2.16).
    #[allow(unsafe_code)]
    pub fn definir_flags(&mut self, h: Ref, mascara: u8, valor: u8) {
        assert!(
            mascara & !(flags_do_bloco::FORMA | flags_do_bloco::ELEMENTO) == 0,
            "bug do runtime: definir_flags só troca FORMA e ELEMENTO"
        );
        let b = self.bloco_vivo(h);
        Self::conferir_gravavel(b);
        // A forma decide que palavras são referências: o ARC fotografa as
        // contadas antes da troca.
        if self.arc.is_some() {
            self.arc_foto(b, h);
        }
        // SAFETY: bloco vivo e gravável.
        unsafe {
            assert!(crate::layout::cid::e_lista_fixa((*b).class_id), "bug do runtime: definir_flags fora de _List/_ImmutableList");
            (*b).flags = ((*b).flags & !mascara) | (valor & mascara);
        }
    }
    /// A barreira de elemento de §2.7 depois de gravar `v` no elemento `i` (índice
    /// a partir de 0, a palavra `i + 1`) do bloco `REFS` `b`.
    #[allow(unsafe_code)]
    fn barreira_de_elemento(&mut self, b: *mut Cabecalho, i: usize, v: Ref) {
        if !self.filho_jovem(v) {
            return;
        }
        // SAFETY: bloco vivo do espaço.
        unsafe {
            let e = (*b).estado;
            if e != VELHO && e != LEMBRADO {
                return;
            }
            if (*b).flags & flags_do_bloco::CARTOES != 0 {
                let len = refs_do_corpo(b, self.objetos.palavras_do_corpo(b));
                if i < len {
                    crate::espaco::sujar_cartao(b, len, i);
                }
            }
        }
        self.objetos.lembrar(b);
    }
    /// O bloco `REFS` gravável de `h`.
    #[allow(unsafe_code)]
    fn bloco_refs(&self, h: Ref) -> *mut Cabecalho {
        let b = self.bloco_vivo(h);
        Self::conferir_gravavel(b);
        // SAFETY: bloco vivo.
        assert!(unsafe { forma(b) } == flags_do_bloco::REFS, "bug do runtime: gravar_ref num objeto que não é REFS");
        b
    }
    /// Grava o `Ref` `v` na palavra `palavra ≥ 1` do corpo `REFS` de `h`, com
    /// barreira (e cartão).
    #[allow(unsafe_code)]
    pub fn gravar_ref(&mut self, h: Ref, palavra: usize, v: Ref) {
        if smi::e_handle(v) && self.stress {
            self.conferir_vivo(v);
        }
        let b = self.bloco_refs(h);
        let (c, w) = self.corpo_e_palavras(b);
        assert!(palavra >= 1 && palavra < w, "palavra {palavra} fora do corpo REFS de {w} palavras");
        // SAFETY: `palavra < w`.
        let antigo = unsafe { std::mem::replace(&mut *campos_de(c).add(palavra), v) };
        if self.arc.is_some() {
            self.arc_trocou(b, h, antigo, v);
        }
        self.barreira_de_elemento(b, palavra - 1, v);
    }
    /// Copia `v` para as palavras `palavra..` do corpo `REFS` de `h`, com uma
    /// barreira só: num velho com cartões, suja os cartões da faixa inteira (a
    /// descompactação no lugar e o preenchimento de `listas.rs` contam com isso)
    /// e o lembra se algum valor é jovem.
    #[allow(unsafe_code)]
    pub fn gravar_refs(&mut self, h: Ref, palavra: usize, v: &[Ref]) {
        let b = self.bloco_refs(h);
        let (c, w) = self.corpo_e_palavras(b);
        assert!(palavra >= 1 && palavra + v.len() <= w, "palavras {palavra}..+{} fora do corpo REFS de {w} palavras", v.len());
        // O ARC conta cada troca: as palavras de antes, uma a uma.
        let antigos: Vec<Ref> = if self.arc.is_some() {
            // SAFETY: a faixa está dentro do corpo.
            unsafe { std::slice::from_raw_parts(campos_de(c).add(palavra), v.len()).to_vec() }
        } else {
            Vec::new()
        };
        // SAFETY: a faixa está dentro do corpo; `v` não aponta para dentro do
        // heap mutável (é uma fatia do chamador).
        unsafe { std::ptr::copy(v.as_ptr(), campos_de(c).add(palavra), v.len()) };
        if v.is_empty() {
            return;
        }
        for (&antigo, &novo) in antigos.iter().zip(v) {
            self.arc_trocou(b, h, antigo, novo);
        }
        // SAFETY: bloco vivo do espaço.
        unsafe {
            let e = (*b).estado;
            if e != VELHO && e != LEMBRADO {
                return;
            }
            if (*b).flags & flags_do_bloco::CARTOES != 0 {
                let len = refs_do_corpo(b, self.objetos.palavras_do_corpo(b));
                let (de, ate) = ((palavra - 1).min(len), (palavra - 1 + v.len()).min(len));
                let mut i = de;
                while i < ate {
                    crate::espaco::sujar_cartao(b, len, i);
                    i = (i / crate::layout::ELEMENTOS_POR_CARTAO + 1) * crate::layout::ELEMENTOS_POR_CARTAO;
                }
            }
        }
        if v.iter().any(|&x| self.filho_jovem(x)) {
            self.objetos.lembrar(b);
        }
    }
    /// A barreira explícita (quem gravou por [`Heap::palavras_mut`] num objeto
    /// publicado): lembra o velho `h`; numa lista com cartões, suja todos.
    #[allow(unsafe_code)]
    pub fn lembrar(&mut self, h: Ref) {
        let Some(b) = self.bloco_do_espaco(h) else { return };
        // SAFETY: bloco do espaço.
        unsafe {
            let e = (*b).estado;
            if (e == VELHO || e == LEMBRADO) && forma(b) == flags_do_bloco::REFS && (*b).flags & flags_do_bloco::CARTOES != 0 {
                let len = refs_do_corpo(b, self.objetos.palavras_do_corpo(b));
                crate::espaco::sujar_todos_os_cartoes(b, len);
            }
        }
        self.objetos.lembrar(b);
    }
    /// A barreira de elemento explícita: lembra o velho `h` e suja o cartão do
    /// elemento `i` (índice a partir de 0).
    #[allow(unsafe_code)]
    pub fn lembrar_elemento(&mut self, h: Ref, i: usize) {
        let Some(b) = self.bloco_do_espaco(h) else { return };
        // SAFETY: bloco do espaço.
        unsafe {
            let e = (*b).estado;
            if e != VELHO && e != LEMBRADO {
                return;
            }
            if forma(b) == flags_do_bloco::REFS && (*b).flags & flags_do_bloco::CARTOES != 0 {
                let len = refs_do_corpo(b, self.objetos.palavras_do_corpo(b));
                if i < len {
                    crate::espaco::sujar_cartao(b, len, i);
                }
            }
        }
        self.objetos.lembrar(b);
    }
    /// Anexa ao bloco `h` (`ANEXO`) o ponteiro nativo `ptr`, que ocupa `bytes`
    /// bytes fora do heap: gravado em `b+16`, `soltar(ptr)` quando o dono morre
    /// (ou quando o heap acaba). Os bytes contam para os gatilhos.
    ///
    /// # Safety
    /// `soltar(ptr)` é a única liberação de `ptr`, e é válida; o corpo de `h` tem
    /// pelo menos uma palavra e é `BRUTO`.
    #[allow(unsafe_code)]
    pub unsafe fn anexar(&mut self, h: Ref, soltar: unsafe fn(*mut u8), ptr: *mut u8, bytes: usize) {
        let b = self.bloco_vivo(h);
        Self::conferir_gravavel(b);
        // SAFETY: o contrato da função; bloco vivo com a palavra 0.
        unsafe {
            // O bloco pode ter nascido com `ANEXO` (`alocar(.., BRUTO | ANEXO)`, a
            // forma de §2.4): o duplicado é um ponteiro já gravado.
            assert!(
                (*b).flags & flags_do_bloco::ANEXO == 0 || *campos_de(b) == 0,
                "bug do runtime: anexo duplicado"
            );
            (*b).flags |= flags_do_bloco::ANEXO;
            *campos_de(b) = ptr as i64;
        }
        self.objetos.anexar(b, soltar, ptr, bytes);
        self.stats.estimated_bytes = self.stats.estimated_bytes.saturating_add(bytes);
        self.stats.peak_estimated_bytes = self.stats.peak_estimated_bytes.max(self.stats.estimated_bytes);
        self.bytes_jovens += bytes;
    }
    /// Os bytes do anexo de `h` passam a ser `bytes` (o acumulador do
    /// `StringBuffer` cresceu): a diferença entra nos gatilhos de coleta, como
    /// [`Heap::contar_externos`], e sai quando o dono morre.
    pub fn ajustar_anexo(&mut self, h: Ref, bytes: usize) {
        let b = self.bloco_vivo(h);
        let delta = self.objetos.ajustar_anexo(b, bytes);
        if delta >= 0 {
            self.stats.estimated_bytes = self.stats.estimated_bytes.saturating_add(delta.unsigned_abs());
            self.stats.peak_estimated_bytes = self.stats.peak_estimated_bytes.max(self.stats.estimated_bytes);
            self.bytes_jovens += delta.unsigned_abs();
        } else {
            self.stats.estimated_bytes = self.stats.estimated_bytes.saturating_sub(delta.unsigned_abs());
        }
    }
    /// O ponteiro nativo anexado a `h` (nulo sem anexo).
    #[allow(unsafe_code)]
    pub fn anexo(&self, h: Ref) -> *mut u8 {
        let b = self.bloco_vivo(h);
        // SAFETY: bloco vivo.
        unsafe { if (*b).flags & flags_do_bloco::ANEXO != 0 { *campos_de(b) as *mut u8 } else { std::ptr::null_mut() } }
    }
    /// Soma `delta` aos bytes de fora do heap (contam para os gatilhos até quem os
    /// contou descontá-los).
    pub fn contar_externos(&mut self, delta: isize) {
        if delta >= 0 {
            let d = delta.unsigned_abs();
            self.externos = self.externos.saturating_add(d);
            self.stats.estimated_bytes = self.stats.estimated_bytes.saturating_add(d);
            self.stats.peak_estimated_bytes = self.stats.peak_estimated_bytes.max(self.stats.estimated_bytes);
            self.bytes_jovens += d;
        } else {
            let d = delta.unsigned_abs();
            self.externos = self.externos.saturating_sub(d);
            self.stats.estimated_bytes = self.stats.estimated_bytes.saturating_sub(d);
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
        let fields = heap.objeto(first).expect("enum deve ser objeto");
        assert_eq!(fields.class_id, 1);
        assert_eq!(fields.campo(0), (0, false));
        assert!(heap.texto(fields.campo(1).0).is_some_and(|name| name == "red"));
        assert_eq!(heap.stats().permanent_roots, 3);
        assert_eq!(heap.stats().live_objects, 6);
        assert_eq!(heap.stats().root_slots, 0);
    }
    /// Um único objeto raiz mantém transitivamente ciclos e strings de seus campos.
    #[test]
    fn tagged_edges_keep_unrooted_children_alive() {
        let mut heap = Heap::new(true);
        let outer = heap.push_frame();
        let parent = heap.novo_objeto(1, &[(0, true)]);
        heap.root(outer, parent);
        let inner = heap.push_frame();
        let text = heap.alocar_str("ação 🦀");
        heap.root(inner, text);
        heap.set(parent, 0, text, true);
        heap.pop_frame(inner);
        heap.collect();
        assert!(heap.texto(text).is_some_and(|s| s == "ação 🦀"));
        heap.set(parent, 0, 0, true);
        heap.collect();
        assert!(!heap.e_objeto_vivo(text));
        heap.pop_frame(outer);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 0);
    }
    /// Ciclos sobrevivem por raiz, depois são coletados e seus slots reutilizados.
    #[test]
    fn cycles_and_slot_reuse() {
        let mut heap = Heap::new(true);
        let frame = heap.push_frame();
        let a = heap.novo_objeto(1, &[(0, true)]);
        heap.root(frame, a);
        let b = heap.novo_objeto(2, &[(a, true)]);
        heap.root(frame, b);
        heap.set(a, 0, b, true);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 2);
        heap.pop_frame(frame);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 0);
        // Os blocos dos mortos voltam à lista livre do tamanho deles.
        let c = heap.novo_objeto(3, &[(0, false)]);
        assert!(c == a || c == b);
        assert_eq!(heap.stats().live_objects, 1);
    }
    /// Bits escalares coincidentes com handles não retêm objetos.
    #[test]
    fn scalar_bits_do_not_trace() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame();
        let text = heap.alocar_str("descartável");
        let object = heap.novo_objeto(1, &[(text, false)]);
        heap.root(frame, object);
        heap.collect();
        assert!(!heap.e_objeto_vivo(text));
        assert!(heap.e_objeto_vivo(object));
    }
    /// Frames aninhados preservam argumentos; stress coleta durante milhares de alocações.
    #[test]
    fn nested_frames_stress() {
        let mut heap = Heap::new(true);
        let outer = heap.push_frame();
        let permanent = heap.alocar_str("vivo");
        heap.root(outer, permanent);
        for _ in 0..1000 {
            let inner = heap.push_frame();
            let temporary = heap.alocar_str("temporário");
            heap.root(inner, temporary);
            heap.collect();
            assert!(heap.texto(permanent).is_some_and(|s| s == "vivo"));
            heap.pop_frame(inner);
        }
        heap.collect();
        assert_eq!(heap.stats().live_objects, 1);
    }
}

#[cfg(test)]
mod espaco_de_objetos {
    //! O espaço de objetos: blocos por número de campos, handles com o bit
    //! 1, listas livres refeitas na varredura e páginas soltas.
    use super::*;


    #[test]
    fn handles_de_objeto_e_escalares_nao_se_confundem() {
        let mut heap = Heap::new(false);
        let o = heap.alocar_objeto(7, 3);
        let s = heap.alocar_str("x");
        assert!(e_objeto(o) && e_objeto(s));
        assert!(matches!(heap.objeto(o), Some(f) if f.class_id == 7 && f.len() == 3));
        // A string não é `INSTANCIA`: não tem vista de campos.
        assert!(heap.objeto(s).is_none());
        // Escalares quaisquer não são objetos vivos (nem derrubam a consulta).
        for x in [2, 6, 18, o + 4, o - 4, o + 8, -2, i64::MAX - 1] {
            assert!(!heap.e_objeto_vivo(x) || x == o || x == s, "{x}");
        }
        heap.set(o, 1, s, true);
        assert_eq!(heap.objeto(o).expect("objeto").campo(1), (s, true));
    }

    #[test]
    fn coleta_devolve_blocos_e_mantem_os_alcancaveis() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame_with_slots(1);
        // Uma lista ligada de 10 mil nós alcançável e 10 mil mortos.
        let mut cabeca = 0;
        for i in 0..10_000 {
            let lixo = heap.alocar_objeto(1, 2);
            heap.set(lixo, 0, i, false);
            let no = heap.alocar_objeto(2, 2);
            heap.set(no, 0, i, false);
            heap.set(no, 1, cabeca, true);
            cabeca = no;
            heap.set_root(frame, 0, cabeca);
        }
        heap.collect();
        assert_eq!(heap.stats().live_objects, 10_000);
        let mut soma = 0;
        let mut no = cabeca;
        while no != 0 {
            let fields = heap.objeto(no).expect("objeto");
            soma += fields.campo(0).0;
            no = fields.campo(1).0;
        }
        assert_eq!(soma, (0..10_000).sum::<i64>());
        heap.pop_frame(frame);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 0);
        // As páginas ficam enquanto o pico recente de vivos as pede (a
        // estrutura pode voltar); o pico decai a cada coleta completa, e as
        // vazias além da folga voltam ao sistema.
        let retidas = heap.objetos.paginas.len();
        assert!(retidas > 2);
        for _ in 0..20 {
            heap.collect();
        }
        assert!(heap.objetos.paginas.len() <= 2, "{} páginas de {retidas}", heap.objetos.paginas.len());
    }

    #[test]
    fn objeto_que_cresce_segue_pelo_ponteiro_dos_campos() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame();
        let o = heap.alocar_objeto(3, 1);
        heap.root(frame, o);
        heap.set(o, 0, 41, false);
        heap.garantir_campos(o, 5);
        heap.set(o, 4, 42, false);
        heap.collect();
        let fields = heap.objeto(o).expect("objeto");
        assert_eq!((fields.len(), fields.campo(0).0, fields.campo(4).0), (5, 41, 42));
        heap.pop_frame(frame);
        heap.collect();
        // O bloco voltou ao molde: um objeto novo de 1 campo o reusa zerado.
        let novo = heap.alocar_objeto(4, 1);
        assert_eq!(novo, o);
        assert!(matches!(heap.objeto(novo), Some(f) if f.class_id == 4 && f.len() == 1 && f.campo(0) == (0, false)));
    }

    #[test]
    fn hash_de_identidade_estavel_e_distinto() {
        let mut heap = Heap::new(false);
        let a = heap.alocar_objeto(1, 0);
        let b = heap.alocar_objeto(1, 0);
        let ha = heap.hash_de_identidade(a).expect("objeto");
        assert_eq!(heap.hash_de_identidade(a), Some(ha));
        assert_ne!(heap.hash_de_identidade(b), Some(ha));
        assert!(ha > 0 && ha <= 0x3fff_ffff);
    }

    /// A alocação em linha do código gerado: avança o cursor da faixa da
    /// TLAB e grava o cabeçalho (`llvm/mod.rs`).
    #[allow(unsafe_code)]
    fn alocar_como_o_codigo_gerado(heap: &mut Heap, class_id: i64, n: usize) -> i64 {
        // A TLAB é por palavras do corpo (`n ≤ TLAB_N` campos: `max(n, 1)`).
        let w = crate::layout::palavras_de_instancia(n);
        let tamanho = tamanho_do_bloco(n);
        let b = CONTEXTO.with(|c| {
            let (cursor, fim) = (c.tlab[w][0].get(), c.tlab[w][1].get());
            if (fim as usize).saturating_sub(cursor as usize) < tamanho {
                return None;
            }
            c.tlab[w][0].set(cursor.wrapping_add(tamanho));
            Some(cursor)
        });
        let Some(b) = b else {
            let h = heap.alocar_objeto(class_id, n);
            heap.reabastecer_tlab(w);
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
    fn faixas_da_tlab_sobrevivem_e_voltam_a_lista() {
        // Numa thread nova: o `Contexto` dela começa sem TLAB.
        std::thread::spawn(|| {
            let mut heap = Heap::do_isolado(false);
            let frame = heap.push_frame_with_slots(1);
            let mut cabeca = 0;
            let mut lixo = Vec::new();
            for i in 0..10_000 {
                let elo = alocar_como_o_codigo_gerado(&mut heap, 5, 2);
                heap.set(elo, 0, smi::de(i).unwrap(), false);
                heap.set(elo, 1, cabeca, true);
                if i % 3 == 0 {
                    cabeca = elo;
                    heap.set_root(frame, 0, cabeca);
                } else {
                    lixo.push(elo);
                }
                if i % 2_500 == 1_249 {
                    heap.coletar(i % 5_000 != 1_249);
                }
            }
            heap.coletar(true);
            let mut n = 0;
            let mut h = cabeca;
            while h != 0 {
                let o = heap.objeto(h).expect("elo vivo");
                assert_eq!(o.class_id, 5);
                n += 1;
                h = o.campo(1).0;
            }
            assert_eq!(n, 3_334);
            assert_eq!(heap.stats().live_objects, 3_334);
            // Os mortos voltaram à lista: alocar de novo não cresce o heap.
            let paginas = heap.objetos.paginas.len();
            for _ in 0..2_000 {
                alocar_como_o_codigo_gerado(&mut heap, 6, 2);
            }
            assert_eq!(heap.objetos.paginas.len(), paginas);
            heap.set_root(frame, 0, 0);
            heap.coletar(false);
            assert_eq!(heap.stats().live_objects, 0);
        })
        .join()
        .unwrap();
    }

    #[test]
    fn varredura_sem_tocar_os_mortos_entrega_blocos_zerados() {
        // O caminho de fora dos testes: as varreduras só leem o mapa de
        // marcas, e a entrega zera a faixa suja.
        std::thread::spawn(|| {
            let mut heap = Heap::do_isolado(false);
            heap.objetos.zerar_mortos = false;
            let frame = heap.push_frame_with_slots(1);
            let mut cabeca = 0;
            for rodada in 0..6 {
                for i in 0..20_000 {
                    let elo = alocar_como_o_codigo_gerado(&mut heap, 5, 3);
                    // O código gerado só zera o primeiro campo: o resto (e o
                    // mapa de referências) tem de vir zerado da entrega.
                    let o = heap.objeto(elo).expect("recém-alocado");
                    assert_eq!((o.campo(1), o.campo(2)), ((0, false), (0, false)), "bloco sujo entregue");
                    heap.set(elo, 0, smi::de(i).unwrap(), false);
                    heap.set(elo, 1, cabeca, true);
                    heap.set(elo, 2, 77, false);
                    if i % 4 == 0 {
                        cabeca = elo;
                        heap.set_root(frame, 0, cabeca);
                    }
                    if i % 3_000 == 2_999 {
                        heap.coletar(i % 9_000 != 8_999);
                    }
                }
                // A lista inteira sobrevive; a cada rodada, a de antes morre.
                let mut n = 0;
                let mut h = cabeca;
                while h != 0 {
                    let o = heap.objeto(h).expect("elo vivo");
                    assert_eq!((o.class_id, o.campo(2).0), (5, 77));
                    n += 1;
                    h = o.campo(1).0;
                }
                assert_eq!(n, 5_000 * (rodada % 2 + 1));
                if rodada % 2 == 1 {
                    cabeca = 0;
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
    fn coleta_menor_segue_os_lembrados_e_solta_os_jovens_mortos() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame_with_slots(1);
        let velho = heap.alocar_objeto(1, 1);
        heap.set_root(frame, 0, velho);
        heap.coletar(false);
        // Jovem só alcançável pelo velho (a barreira de `set` o lembra).
        let jovem = heap.alocar_objeto(2, 1);
        heap.set(jovem, 0, 7, false);
        heap.set(velho, 0, jovem, true);
        let lixo = heap.alocar_objeto(3, 1);
        heap.coletar(true);
        assert!(matches!(heap.objeto(jovem), Some(f) if f.class_id == 2 && f.campo(0).0 == 7));
        assert!(!heap.e_objeto_vivo(lixo));
        assert_eq!(heap.stats().live_objects, 2);
        // Depois dela, o jovem ficou velho: sem barreira nova, outra menor
        // não o percorre, e ele continua vivo.
        heap.coletar(true);
        assert!(heap.e_objeto_vivo(jovem));
        // A completa solta o que só os velhos mortos alcançavam.
        heap.set_root(frame, 0, 0);
        heap.coletar(false);
        assert_eq!(heap.stats().live_objects, 0);
    }

    #[test]
    #[should_panic(expected = "barreira de escrita faltando")]
    fn verificacao_acha_barreira_faltando() {
        let mut heap = Heap::new(false);
        heap.verificar = true;
        let frame = heap.push_frame_with_slots(1);
        let velho = heap.alocar_objeto(1, 1);
        heap.set_root(frame, 0, velho);
        heap.coletar(false);
        let jovem = heap.alocar_objeto(2, 0);
        // Grava sem barreira (como faria código gerado sem ela).
        let b = heap.objetos.bloco_de(velho).expect("objeto");
        #[allow(unsafe_code)]
        // SAFETY: bloco vivo de 1 campo.
        unsafe {
            *campos_de(b) = jovem;
            marcar_referencia(b, 0, true);
        }
        heap.coletar(true);
    }

    /// A carga de `bench/desempenho/objetos_escapam.dart` (`lista_ligada`)
    /// direto no heap, para medir o coletor sem compilar Dart:
    /// `cargo test --release -p dartforge-runtime lista_ligada_no_heap --
    /// --ignored --nocapture` (com `DARTFORGE_GC_RASTRO=1`, as coletas).
    #[test]
    #[ignore = "medição"]
    fn lista_ligada_no_heap() {
        let mut heap = Heap::new(false);
        heap.rastrear = std::env::var("DARTFORGE_GC_RASTRO").as_deref() == Ok("1");
        let frame = heap.push_frame_with_slots(1);
        for _ in 0..6 {
            let t = std::time::Instant::now();
            let mut cab = 0;
            for i in 0..1_000_000 {
                let no = heap.alocar_objeto(1, 2);
                heap.set(no, 0, i, false);
                heap.set(no, 1, cab, true);
                cab = no;
                heap.set_root(frame, 0, cab);
            }
            eprintln!("rodada: {:?} coletas={}", t.elapsed(), heap.stats().collections);
        }
    }

    #[test]
    fn mapa_de_referencias_alem_de_32_campos() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame_with_slots(1);
        let o = heap.alocar_objeto(1, 40);
        heap.set_root(frame, 0, o);
        let mut filhos = Vec::new();
        for i in [0usize, 31, 32, 39] {
            let f = heap.alocar_objeto(2, 0);
            heap.set(o, i as i64, f, true);
            filhos.push(f);
        }
        // Um escalar que parece handle não segura nada.
        let lixo = heap.alocar_objeto(3, 0);
        heap.set(o, 33, lixo, false);
        heap.coletar(false);
        for f in &filhos {
            assert!(heap.objeto(*f).is_some());
        }
        assert!(heap.objeto(lixo).is_none());
        let obj = heap.objeto(o).expect("objeto");
        assert_eq!(obj.campo(32), (filhos[2], true));
        assert!(!obj.campo(33).1);
        // Trocar a referência por escalar apaga o bit.
        heap.set(o, 39, 5, false);
        heap.coletar(false);
        assert!(heap.objeto(filhos[3]).is_none());
    }

    #[test]
    fn corpo_de_fora_segue_na_coleta_e_na_barreira() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame_with_slots(1);
        let o = heap.alocar_objeto(1, 1);
        heap.set_root(frame, 0, o);
        heap.coletar(false);
        // O objeto velho cresce (corpo de fora) e recebe um jovem no campo
        // novo: a coleta menor o acha pelo lembrado.
        heap.garantir_campos(o, 3);
        let jovem = heap.alocar_objeto(2, 0);
        heap.set(o, 2, jovem, true);
        heap.coletar(true);
        assert!(heap.objeto(jovem).is_some());
        assert_eq!(heap.objeto(o).expect("objeto").len(), 3);
        heap.pop_frame(frame);
        heap.coletar(false);
        assert_eq!(heap.stats().live_objects, 0);
    }

    #[test]
    fn objeto_grande_tem_pagina_propria() {
        // Acima da maior classe média (`layout::MAIOR_MEDIA` palavras): uma
        // região grande só dele, devolvida na coleta completa.
        let mut heap = Heap::new(false);
        let n = crate::layout::MAIOR_MEDIA + 100;
        let o = heap.alocar_objeto(9, n);
        heap.set(o, (n - 1) as i64, 5, false);
        assert!(matches!(heap.objeto(o), Some(f) if f.campo(n - 1).0 == 5));
        heap.collect();
        assert!(!heap.e_objeto_vivo(o));
        assert_eq!(heap.objetos.n_paginas(), 0);
    }
}

#[cfg(test)]
mod falhas_de_handle {
    //! N4: cada contrato quebrado tem mensagem própria.
    use super::*;

    #[test]
    #[should_panic(expected = "handle null (0) desreferenciado")]
    fn null_desreferenciado() {
        Heap::new(false).cabecalho(0);
    }

    #[test]
    #[should_panic(expected = "handle negativo")]
    fn handle_negativo() {
        Heap::new(false).cabecalho(-2);
    }

    #[test]
    #[should_panic(expected = "handle além da tabela")]
    fn handle_alem_da_tabela() {
        let mut heap = Heap::new(false);
        heap.alocar_str("x");
        heap.cabecalho(42);
    }

    #[test]
    #[should_panic(expected = "handle já coletado")]
    fn handle_coletado() {
        let mut heap = Heap::new(false);
        let h = heap.alocar_str("x");
        heap.collect();
        heap.cabecalho(h);
    }
}

#[cfg(test)]
mod raizes_do_runtime {
    //! G6: raízes que não moram num frame, e as tabelas laterais.
    use super::*;

    #[test]
    fn excecao_pendente_sobrevive_a_coleta_sem_frame() {
        let mut heap = Heap::new(true);
        let erro = heap.alocar_str("falhou");
        heap.set_raiz_do_runtime(0, erro);
        heap.alocar_str("outra");
        heap.collect();
        assert!(heap.texto(erro).is_some_and(|s| s == "falhou"));
        heap.set_raiz_do_runtime(0, 0);
        heap.collect();
        assert_eq!(heap.stats().live_objects, 0);
    }

    #[test]
    fn coleta_sem_frame_aberto() {
        // O portão antigo só coletava com frame; agora o limiar basta (a
        // coleta menor, por contagem de alocações).
        let mut heap = Heap::new(false);
        let n = CONTAGEM_JOVEM + 1000;
        for _ in 0..n {
            heap.alocar_str("lixo");
        }
        assert!(heap.stats().collections > 0);
        assert!(heap.stats().live_objects < n);
    }

    #[test]
    fn tabela_lateral_purgada_quando_o_bloco_e_reutilizado() {
        let mut heap = Heap::new(false);
        let o = heap.alocar_objeto(200, 1);
        heap.marcar_late((o, 0));
        heap.marcar_late((o, -1));
        heap.marcar_late((o, -2));
        heap.collect();
        assert!(heap.campos_late_inicializados.is_empty());
        let nova = heap.alocar_objeto(200, 1);
        assert!(!heap.late_inicializado((nova, 0)));
        assert!(!heap.late_inicializado((nova, -1)));
        assert!(!heap.late_inicializado((nova, -2)));
    }

    #[test]
    fn coleta_menor_purga_so_as_marcas_dos_jovens_mortos() {
        let mut heap = Heap::new(false);
        let frame = heap.push_frame_with_slots(1);
        let velho = heap.alocar_objeto(200, 0);
        heap.set_root(frame, 0, velho);
        heap.marcar_late((velho, 0));
        heap.coletar(true);
        assert!(heap.late_inicializado((velho, 0)), "o velho vivo continua marcado");
        let jovem = heap.alocar_objeto(200, 0);
        heap.marcar_late((jovem, 3));
        heap.coletar(true);
        assert!(!heap.late_inicializado((jovem, 3)), "o jovem morto perde a marca");
        assert!(heap.late_inicializado((velho, 0)));
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
        let a = heap.alocar_str("á\0🦀");
        heap.root(frame, a);
        let b = heap.alocar_str("á\0🦀");
        heap.root(frame, b);
        assert_ne!(a, b);
        assert!(heap.textos_iguais(a, b));
        let c = heap.alocar_str("a\u{301}\0🦀");
        heap.root(frame, c);
        assert!(!heap.textos_iguais(a, c));
    }
    /// Retorno transfere raiz sem coleta entre pop e registro no chamador.
    #[test]
    fn returned_handles_and_identity() {
        let mut heap = Heap::new(true);
        let caller = heap.push_frame();
        let callee = heap.push_frame();
        let a = heap.novo_objeto(7, &[]);
        heap.root(callee, a);
        heap.pop_frame(callee);
        heap.root(caller, a);
        let b = heap.novo_objeto(7, &[]);
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
        let first = heap.novo_objeto(1, &[(0, true)]);
        heap.root(frame, first);
        let mut previous = first;
        for _ in 1..20_000 {
            let next = heap.novo_objeto(1, &[(first, true)]);
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
                let handle = heap.novo_objeto(1, &[(index, false)]);
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
        let kept = heap.alocar_str("keep");
        heap.set_root(frame, 0, kept);
        heap.set_root(frame, 1, kept);
        for _ in 0..1000 {
            let inner = heap.push_frame_with_slots(1);
            let temporary = heap.alocar_str("next");
            heap.set_root(inner, 0, temporary);
            heap.pop_frame(inner);
            heap.set_root(frame, 0, temporary);
            heap.collect();
            assert!(heap.texto(kept).is_some_and(|s| s == "keep"));
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
            let handle = heap.alocar(crate::layout::cid::tipada(1), 2 * 1024 * 1024 / 8, crate::layout::flags::BRUTO);
            heap.set_root(frame, 0, handle);
        }
        let stats = heap.stats();
        assert!(stats.collections >= 3);
        assert!(stats.reclaimed >= 5);
        // O pico: o jovem entre coletas menores e o lixo velho (cada payload
        // enraizado vira velho e morre na volta seguinte) até a completa,
        // que vem dois semiespaços jovens acima do que sobreviveu.
        assert!(stats.peak_estimated_bytes < 2 * LIMITE_JOVEM + 3 * 2 * 1024 * 1024, "{}", stats.peak_estimated_bytes);
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
                let handle = heap.alocar_str("temporary string");
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
pub static EPOCA_DE_LAYOUT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

thread_local! {
    /// `DARTFORGE_EFEITOS=conferir` (`efeitos_conferir.rs`): as externs em
    /// execução nesta thread que a tabela de efeitos marca `coleta = 0`. Com
    /// alguma, uma coleta é a marca errada — o defeito de um mapa de raízes
    /// errado, só que do nosso lado — e encerra o processo dizendo qual.
    static COLETA_PROIBIDA: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Proíbe a coleta até o [`permitir_coleta`] casado: a extern `nome` está
/// marcada `coleta = 0`.
pub fn proibir_coleta(nome: String) {
    COLETA_PROIBIDA.with(|p| p.borrow_mut().push(nome));
}

/// Desfaz o último [`proibir_coleta`].
pub fn permitir_coleta() {
    COLETA_PROIBIDA.with(|p| {
        p.borrow_mut().pop();
    });
}

/// Encerra o processo se uma extern marcada `coleta = 0` está em execução.
fn conferir_coleta_permitida() {
    let culpada = COLETA_PROIBIDA.with(|p| p.borrow().last().cloned());
    if let Some(nome) = culpada {
        eprintln!("dartforge: a extern {nome} está marcada coleta = 0 em efeitos.tsv e coletou");
        std::process::abort();
    }
}

#[cfg(test)]
mod espaco_unificado {
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
        h.verificar = true;
        h
    }

    /// Um `_List` geral (`REFS`) de `len` elementos nulos, com cartões se grande.
    fn lista(heap: &mut Heap, len: usize) -> Ref {
        let mut f = flags::REFS;
        if layout::tem_cartoes(len) {
            f |= flags::CARTOES;
        }
        let h = heap.alocar(cid::LIST, layout::palavras_de_lista(len), f);
        heap.palavras_mut(h)[0] = len as i64;
        h
    }

    #[test]
    fn bruto_guarda_bits_e_nao_segue_nada() {
        let mut heap = heap();
        let quadro = heap.push_frame_with_slots(1);
        let alvo = heap.alocar_objeto(200, 1);
        let m = heap.alocar(cid::MINT, 2, flags::BRUTO);
        heap.set_root(quadro, 0, m);
        // Bits que parecem um handle, num corpo BRUTO: o coletor não os segue.
        heap.palavras_mut(m)[0] = i64::MAX;
        heap.palavras_mut(m)[1] = alvo;
        heap.collect();
        assert!(!heap.e_objeto_vivo(alvo), "BRUTO seguido como referência");
        assert_eq!(heap.palavras(m)[0], i64::MAX);
        assert_eq!(heap.classe(m), cid::MINT);
        assert_eq!(heap.cabecalho(m).n, 2);
        heap.pop_frame(quadro);
    }

    #[test]
    fn refs_segue_os_elementos_e_a_barreira_lembra() {
        let mut heap = heap();
        let quadro = heap.push_frame_with_slots(1);
        let l = lista(&mut heap, 3);
        heap.set_root(quadro, 0, l);
        let a = heap.alocar_objeto(200, 0);
        heap.gravar_ref(l, 1, a);
        heap.gravar_ref(l, 3, smi::de(7).unwrap());
        heap.collect();
        assert!(heap.e_objeto_vivo(a), "elemento REFS não seguido");
        // A lista é velha: um jovem gravado nela sobrevive à coleta menor
        // pela barreira (sem ela, a verificação acusaria).
        let b = heap.alocar_objeto(201, 0);
        heap.gravar_ref(l, 2, b);
        assert_eq!(heap.cabecalho(l).estado, LEMBRADO);
        heap.coletar(true);
        assert!(heap.e_objeto_vivo(b), "jovem de lista velha coletado");
        assert_eq!(heap.palavras(l)[1..], [a, b, smi::de(7).unwrap()]);
        // Sem referência, some.
        heap.gravar_refs(l, 1, &[0, 0]);
        heap.collect();
        assert!(!heap.e_objeto_vivo(a) && !heap.e_objeto_vivo(b));
        heap.pop_frame(quadro);
    }

    #[test]
    fn instancia_segue_so_os_campos_referencia() {
        let mut heap = heap();
        let quadro = heap.push_frame_with_slots(1);
        let o = heap.alocar_instancia(200, 40);
        heap.set_root(quadro, 0, o);
        let filho = heap.alocar_objeto(201, 0);
        let solto = heap.alocar_objeto(202, 0);
        heap.definir_campo(o, 35, filho, true);
        heap.definir_campo(o, 36, solto, false);
        heap.collect();
        assert!(heap.e_objeto_vivo(filho));
        assert!(!heap.e_objeto_vivo(solto), "campo escalar seguido");
        assert_eq!(heap.palavras(o).len(), layout::palavras_de_instancia(40));
        heap.pop_frame(quadro);
    }

    #[test]
    fn cartoes_mantem_o_jovem_gravado_em_lista_grande_velha() {
        let mut heap = heap();
        let quadro = heap.push_frame_with_slots(1);
        let len = 100_000;
        let l = lista(&mut heap, len);
        heap.set_root(quadro, 0, l);
        assert!(heap.cabecalho(l).flags & flags::CARTOES != 0);
        heap.collect();
        // Velha: gravações velho → jovem espalhadas, cada uma sujando o cartão.
        let mut jovens = Vec::new();
        for i in [5, 2048, 40_000, len - 1] {
            let j = heap.alocar_objeto(200, 0);
            heap.gravar_ref(l, 1 + i, j);
            jovens.push((i, j));
        }
        heap.coletar(true);
        for &(i, j) in &jovens {
            assert!(heap.e_objeto_vivo(j), "jovem no elemento {i} coletado (cartão não percorrido)");
            assert_eq!(heap.palavras(l)[1 + i], j);
        }
        // A coleta menor zerou os cartões.
        let cartoes = &heap.palavras(l)[1 + len..];
        assert!(cartoes.iter().all(|&c| c == 0), "cartões não zerados");
        // A barreira explícita (gravação por fora) suja todos.
        let k = heap.alocar_objeto(201, 0);
        heap.palavras_mut(l)[1 + 77_777] = k;
        heap.lembrar(l);
        heap.coletar(true);
        assert!(heap.e_objeto_vivo(k));
        heap.pop_frame(quadro);
    }

    static SOLTOS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    #[allow(unsafe_code)]
    unsafe fn soltar_caixa(p: *mut u8) {
        // SAFETY: `p` veio de `Box::into_raw` no teste.
        drop(unsafe { Box::from_raw(p.cast::<[u8; 1000]>()) });
        SOLTOS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    #[test]
    #[allow(unsafe_code)]
    fn anexo_solto_quando_o_dono_morre() {
        let mut heap = heap();
        let antes = SOLTOS.load(std::sync::atomic::Ordering::SeqCst);
        let quadro = heap.push_frame_with_slots(1);
        let a = heap.alocar(cid::ACUMULADOR_DE_TEXTO, 1, flags::BRUTO);
        let p = Box::into_raw(Box::new([0u8; 1000])).cast::<u8>();
        let bytes_antes = heap.stats().estimated_bytes;
        // SAFETY: `soltar_caixa` é a única liberação de `p`.
        unsafe { heap.anexar(a, soltar_caixa, p, 1000) };
        assert_eq!(heap.anexo(a), p);
        assert_eq!(heap.stats().estimated_bytes, bytes_antes + 1000);
        // O anexo cresce (o acumulador do `StringBuffer`): conta nos gatilhos.
        heap.ajustar_anexo(a, 3000);
        assert_eq!(heap.stats().estimated_bytes, bytes_antes + 3000);
        assert_eq!(heap.objetos.bytes_de_anexos, 3000);
        heap.set_root(quadro, 0, a);
        heap.coletar(true);
        heap.collect();
        assert_eq!(SOLTOS.load(std::sync::atomic::Ordering::SeqCst), antes, "anexo solto com o dono vivo");
        heap.pop_frame(quadro);
        heap.collect();
        assert_eq!(SOLTOS.load(std::sync::atomic::Ordering::SeqCst), antes + 1, "anexo do morto não solto");
        assert_eq!(heap.objetos.bytes_de_anexos, 0);
    }

    #[test]
    fn regiao_grande_devolvida() {
        let mut heap = heap();
        let paginas = heap.objetos.n_paginas();
        let t = heap.alocar(cid::tipada(1), 5000, flags::BRUTO);
        assert!(heap.objetos.bloco_de(t).is_some());
        assert_eq!(heap.palavras(t).len(), 5000, "o n saturado lido da região");
        assert_eq!(heap.objetos.n_paginas(), paginas + 1);
        heap.collect();
        assert_eq!(heap.objetos.n_paginas(), paginas, "região grande morta não saiu do espaço");
        assert!(heap.objetos.bytes_em_cache() > 0, "região solta sem cache");
        // Reusada do cache pela alocação do mesmo tamanho.
        let u = heap.alocar(cid::tipada(1), 5000, flags::BRUTO);
        assert_eq!(heap.objetos.bytes_em_cache(), 0);
        assert!(heap.palavras(u).iter().all(|&w| w == 0), "região reusada suja");
        heap.collect();
        heap.collect();
        heap.collect();
        assert_eq!(heap.objetos.bytes_em_cache(), 0, "região parada não voltou ao sistema");
        // Uma classe média de 16 KiB não é região grande.
        let paginas = heap.objetos.n_paginas();
        let m = heap.alocar(cid::tipada(1), layout::MAIOR_MEDIA, flags::BRUTO);
        assert_eq!(heap.palavras(m).len(), layout::MAIOR_MEDIA);
        assert!(heap.objetos.n_paginas() <= paginas + 1);
    }

    /// Um estático na forma de §2.11 (em memória só de leitura do teste).
    static TEXTO_ESTATICO: [u64; 3] = [
        layout::palavra_do_cabecalho(layout::estado::PERMANENTE, flags::BRUTO, 2, cid::ONE_BYTE_STRING),
        0,
        3,
    ];

    #[test]
    fn estatico_nunca_marcado_nem_varrido() {
        let inicio = std::ptr::addr_of!(TEXTO_ESTATICO) as usize;
        registrar_imagem(inicio, inicio + std::mem::size_of_val(&TEXTO_ESTATICO));
        let s = inicio as i64 + layout::DESLOCAMENTO_DO_HANDLE;
        let v = Heap::caixa_bool(true);
        let mut heap = heap();
        heap.stress = true;
        assert!(heap.e_estatico(s) && heap.e_estatico(v) && heap.e_objeto_vivo(s));
        assert_eq!(heap.classe(s), cid::ONE_BYTE_STRING);
        let quadro = heap.push_frame_with_slots(2);
        let l = lista(&mut heap, 2);
        heap.set_root(quadro, 0, l);
        heap.set_root(quadro, 1, s);
        heap.gravar_ref(l, 1, s);
        heap.gravar_ref(l, 2, v);
        let o = heap.alocar_objeto(200, 1);
        heap.definir_campo(o, 0, s, true);
        heap.set_global_root(1, o);
        heap.fracas.insert(o, s);
        for menor in [true, false, true, false] {
            heap.coletar(menor);
        }
        assert_eq!(heap.cabecalho(s).estado, layout::estado::PERMANENTE);
        assert_eq!(heap.palavras(s)[0], 3);
        assert_eq!(heap.fracas.get(&o), Some(&s), "referência fraca a estático zerada");
        assert!(heap.e_permanente(s));
        assert_eq!(heap.hash_de_identidade(s), heap.hash_de_identidade(s));
        heap.pop_frame(quadro);
    }

    #[test]
    #[should_panic(expected = "gravação num objeto estático")]
    fn estatico_nao_e_gravado() {
        let mut heap = heap();
        heap.palavras_mut(Heap::caixa_bool(false));
    }

    #[test]
    fn permanentes_e_constantes_purgados() {
        let mut heap = heap();
        let o = heap.alocar_objeto(200, 0);
        heap.marcar_permanente(o);
        heap.marcar_constante(o, 0x1234);
        assert!(heap.e_permanente(o));
        heap.collect();
        assert!(heap.permanentes.is_empty() && heap.constantes.is_empty(), "handle morto nas tabelas de identidade");
        // Vivo por um global, fica.
        let g = heap.alocar_objeto(200, 0);
        heap.marcar_constante(g, 0x5678);
        heap.set_global_root(9, g);
        heap.collect();
        assert_eq!(heap.getter_da_constante(g), Some(0x5678));
    }

    #[test]
    fn bytes_externos_disparam_o_gatilho() {
        let mut heap = heap();
        assert!(!heap.precisa_coletar(16));
        let muito = heap.byte_threshold + 1;
        heap.contar_externos(muito as isize);
        assert!(heap.precisa_coletar(16), "bytes externos fora do gatilho");
        heap.collect();
        assert!(heap.stats().estimated_bytes >= muito, "a coleta completa perdeu os bytes externos");
        heap.contar_externos(-(muito as isize));
        assert!(heap.stats().estimated_bytes < muito);
    }

    #[test]
    fn definir_flags_troca_so_a_forma() {
        let mut heap = heap();
        let l = heap.alocar(cid::LIST, 3, flags::BRUTO | flags::ELEMENTO_INT);
        heap.definir_flags(l, flags::FORMA | flags::ELEMENTO, flags::REFS);
        assert_eq!(heap.cabecalho(l).flags, flags::REFS);
    }

    #[test]
    fn classes_medias_e_exatas_convivem() {
        let mut heap = heap();
        let quadro = heap.push_frame_with_slots(1);
        let l = lista(&mut heap, 200);
        heap.set_root(quadro, 0, l);
        let mut hs = Vec::new();
        for w in [1, 7, 64, 65, 70, 71, 300, 1000, 2014] {
            let h = heap.alocar(cid::tipada(1), w, flags::BRUTO);
            heap.palavras_mut(h)[w - 1] = w as i64;
            hs.push((w, h));
        }
        for (i, &(_, h)) in hs.iter().enumerate() {
            heap.gravar_ref(l, 1 + i, h);
        }
        heap.coletar(true);
        heap.collect();
        for &(w, h) in &hs {
            assert_eq!(heap.palavras(h).len(), w);
            assert_eq!(heap.palavras(h)[w - 1], w as i64, "corpo de {w} palavras perdido");
        }
        heap.pop_frame(quadro);
    }
}

/// O estado do ARC no heap (docs/ARC-IMPLEMENTACAO.md, sobre
/// docs/ARC-CICLOS-ESPECIFICACAO.md §19 e §22): o RC das ocorrências fortes
/// entre objetos velhos e nos slots proprietários. Raízes observacionais são
/// vistas na sincronização, sem contar RC; jovens usam coleta menor até promoção.
pub struct ArcDoHeap {
    pub estado: crate::arc::EstadoDoArc,
    /// Os velhos entregues crus nesta época ([`Heap::palavras_mut`],
    /// [`Heap::campos_de_objeto`], [`Heap::definir_flags`]): as ocorrências
    /// contadas que tinham antes. A sincronização conta as de agora e solta
    /// estas; até lá, as gravações contadas na hora pulam o objeto.
    fotos: crate::hash::HashMap<Ref, Vec<Ref>>,
    efemeros: EfemerosDoArc,
    /// `DARTFORGE_ARC_CONFERIR=1`: depois de cada sincronização, nenhum
    /// alcançável morto e o RC de cada vivo igual às ocorrências recontadas.
    conferir: bool,
    /// `DARTFORGE_ARC_CICLOS=sempre`: a rodada de ciclos (*trial
    /// deletion*) em toda sincronização; `nunca`: nem ela nem o ponto fixo
    /// da completa (o diagnóstico); o padrão: o ponto fixo na completa.
    ciclos: Option<bool>,
    /// O ARC puro (o padrão; docs/ARC-IMPLEMENTACAO.md §6): toda gravação
    /// de referência conta na hora, também num jovem, e a drenagem
    /// ([`Heap::drenar_arc`]) decide a vida sem percorrer o heap. Com
    /// `DARTFORGE_ARC_BERCARIO=1`, o modo da primeira etapa: o berçário pela
    /// coleta menor rastreada e as arestas do jovem contadas na promoção.
    puro: bool,
}

impl std::fmt::Debug for ArcDoHeap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArcDoHeap")
            .field("registrados", &self.estado.registrados())
            .field("estatisticas", &self.estado.estatisticas)
            .field("fotos", &self.fotos.len())
            .field("efemeros", &self.efemeros.contados.len())
            .finish()
    }
}

/// As entradas de efêmero contadas: cada uma é uma ocorrência do valor,
/// atribuída à chave no grafo (o valor vive enquanto a chave e o portador
/// vivem; a morte do portador solta a entrada na sincronização).
#[derive(Default)]
struct EfemerosDoArc {
    contados: crate::hash::HashMap<Ref, (Ref, Ref)>,
    por_chave: crate::hash::HashMap<Ref, Vec<Ref>>,
}

impl EfemerosDoArc {
    fn inserir(&mut self, portador: Ref, chave: Ref, valor: Ref) {
        self.contados.insert(portador, (chave, valor));
        self.por_chave.entry(chave).or_default().push(portador);
    }
    fn remover(&mut self, portador: Ref) -> Option<(Ref, Ref)> {
        let (chave, valor) = self.contados.remove(&portador)?;
        if let Some(l) = self.por_chave.get_mut(&chave) {
            l.retain(|&p| p != portador);
            if l.is_empty() {
                self.por_chave.remove(&chave);
            }
        }
        Some((chave, valor))
    }
}

/// O grafo do ARC sobre o heap: as posições que a marcação percorre e as
/// entradas de efêmero contadas (docs/ARC-CICLOS-ESPECIFICACAO.md §19.2).
struct GrafoDoHeap<'a> {
    heap: &'a mut Heap,
    efemeros: &'a mut EfemerosDoArc,
}

impl crate::arc::GrafoArc for GrafoDoHeap<'_> {
    fn arestas(&self, h: Ref, f: &mut dyn FnMut(Ref)) {
        // SAFETY: as posições são palavras do corpo de um bloco vivo.
        #[allow(unsafe_code)]
        self.heap.posicoes_de_ref(h, &mut |p| f(unsafe { *p }));
        // Sem efêmero contado (o caso comum), nada de hash por objeto.
        if self.efemeros.por_chave.is_empty() {
            return;
        }
        if let Some(ps) = self.efemeros.por_chave.get(&h) {
            for p in ps {
                if let Some(&(k, v)) = self.efemeros.contados.get(p)
                    && k == h
                {
                    f(v);
                }
            }
        }
    }
    fn tirar_arestas(&mut self, h: Ref, f: &mut dyn FnMut(Ref)) {
        // SAFETY: as posições são palavras do corpo de um bloco vivo.
        #[allow(unsafe_code)]
        self.heap.posicoes_de_ref(h, &mut |p| unsafe {
            f(*p);
            *p = 0;
        });
        if !self.efemeros.por_chave.is_empty() {
            // As arestas condicionais (a chave segura o valor) e o
            // rompimento delas, como em `arestas` e `romper`.
            let mut valores = Vec::new();
            if let Some(ps) = self.efemeros.por_chave.get(&h) {
                for p in ps {
                    if let Some(&(k, v)) = self.efemeros.contados.get(p)
                        && k == h
                    {
                        valores.push(v);
                    }
                }
            }
            for v in valores {
                f(v);
            }
            if let Some(ps) = self.efemeros.por_chave.remove(&h) {
                for p in ps {
                    self.efemeros.contados.remove(&p);
                    if let Some(e) = self.heap.efemeros.get_mut(&p)
                        && e.0 == h
                    {
                        *e = (0, 0);
                    }
                }
            }
        }
    }
    fn romper(&mut self, h: Ref) {
        // SAFETY: as posições são palavras do corpo de um bloco vivo.
        #[allow(unsafe_code)]
        self.heap.posicoes_de_ref(h, &mut |p| unsafe { *p = 0 });
        if self.efemeros.por_chave.is_empty() {
            return;
        }
        if let Some(ps) = self.efemeros.por_chave.remove(&h) {
            for p in ps {
                self.efemeros.contados.remove(&p);
                if let Some(e) = self.heap.efemeros.get_mut(&p)
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
/// (`EstadoDoArc::sem_arestas`).
fn bloco_sem_referencias(h: Ref) -> bool {
    // SAFETY: o ARC só chama com handles registrados, blocos vivos do espaço
    // (a imagem não tem metadados).
    #[allow(unsafe_code)]
    unsafe {
        (*((h - DESLOCAMENTO_DO_HANDLE) as *const Cabecalho)).flags & flags_do_bloco::FORMA == flags_do_bloco::BRUTO
    }
}

/// Uma violação do contador é bug do compilador ou do runtime, nunca erro
/// Dart (docs/ARC-CICLOS-ESPECIFICACAO.md §19.3).
#[cold]
fn falha_do_arc(e: crate::arc::ErroArc) -> ! {
    panic!("bug do ARC: {e:?}")
}

/// A geometria da página do bloco `h` para a tabela de metadados do ARC
/// (`arc::Geometria`); `None` fora do espaço (o metadado vai para a reserva).
fn geometria_arc(espaco: &EspacoDeObjetos, h: Ref) -> Option<crate::arc::Geometria> {
    espaco.geometria(h).map(|(primeiro, tamanho, blocos)| crate::arc::Geometria { primeiro, tamanho, blocos })
}

/// ARC puro, no laço da drenagem (depois do passo 4, em que todo jovem que
/// sobrevive é registrado): o objeto do espaço sem metadados morreu nesta
/// drenagem — o jovem que ninguém contou, ou o morto pelo RC, cujos
/// metadados `tomar_mortos` já tirou. O estático (`PERMANENTE`) não é do
/// espaço e nunca morre.
#[allow(unsafe_code)]
fn morreu_na_drenagem(espaco: &EspacoDeObjetos, validar: bool, estado: &crate::arc::EstadoDoArc, h: Ref) -> bool {
    if !e_objeto(h) {
        return false;
    }
    let no_espaco = if validar {
        espaco.bloco_de(h).is_some()
    } else {
        // SAFETY: um handle de objeto é um bloco ou um estático (o contrato
        // de `bloco_do_espaco` sem a validação).
        unsafe { (*((h - DESLOCAMENTO_DO_HANDLE) as *const Cabecalho)).estado != PERMANENTE }
    };
    no_espaco && estado.meta(h).is_none()
}

#[allow(unsafe_code)]
impl Heap {
    /// O ARC está ligado.
    pub fn arc_ativo(&self) -> bool {
        self.arc.is_some()
    }

    /// Os contadores do ARC, se ligado.
    pub fn estatisticas_do_arc(&self) -> Option<crate::arc::EstatisticasArc> {
        self.arc.as_ref().map(|a| a.estado.estatisticas)
    }

    /// A troca de `antigo` por `novo` numa posição forte do bloco `b` (de
    /// `h`), contada na hora se o dono não foi fotografado. No ARC puro
    /// conta sempre, e o destino ainda sem metadados é registrado (o jovem
    /// que ganha a primeira ocorrência). No modo do berçário só conta com o
    /// dono velho (as arestas do jovem entram na promoção).
    fn arc_trocou(&mut self, b: *mut Cabecalho, h: Ref, antigo: Ref, novo: Ref) {
        let novo_no_espaco = self.bloco_do_espaco(novo).is_some();
        let Some(arc) = self.arc.as_deref_mut() else { return };
        // SAFETY: bloco vivo do espaço.
        let e = unsafe { (*b).estado };
        if antigo == novo || arc.fotos.contains_key(&h) || (!arc.puro && e != VELHO && e != LEMBRADO) {
            return;
        }
        if arc.puro && novo_no_espaco {
            Self::arc_registrar_se_preciso(&self.objetos, arc, novo);
        }
        arc.estado.reter(novo).unwrap_or_else(|e| falha_do_arc(e));
        arc.estado.soltar(antigo).unwrap_or_else(|e| falha_do_arc(e));
    }

    /// O objeto do espaço `h` ainda sem metadados ganha registro (RC zero,
    /// vivo) antes da primeira ocorrência contada.
    fn arc_registrar_se_preciso(espaco: &EspacoDeObjetos, arc: &mut ArcDoHeap, h: Ref) {
        if smi::e_handle(h) && arc.estado.meta(h).is_none() {
            arc.estado.registrar_vivo_em(h, geometria_arc(espaco, h));
        }
    }

    /// Retém `v` (registrando o objeto do espaço sem metadados): uma
    /// ocorrência nova, no ARC puro.
    fn arc_reter_registrando(&self, arc: &mut ArcDoHeap, v: Ref) {
        if smi::e_handle(v) && self.bloco_do_espaco(v).is_some() {
            Self::arc_registrar_se_preciso(&self.objetos, arc, v);
        }
        arc.estado.reter(v).unwrap_or_else(|e| falha_do_arc(e));
    }

    /// As referências que o código gerado gravou em linha num objeto recém
    /// alocado (`AllocObject`, contexto, célula e closure em linha,
    /// `dartforge_arc_inicial`): no ARC puro, cada uma conta. No modo do
    /// berçário não faz nada (contam na promoção).
    pub fn arc_contar_iniciais(&mut self, h: Ref) {
        if !smi::e_handle(h) || self.bloco_do_espaco(h).is_none() {
            return;
        }
        let Some(mut arc) = self.arc.take() else { return };
        if arc.puro {
            let mut valores = Vec::new();
            // SAFETY: as posições são palavras do corpo de um bloco vivo.
            self.posicoes_de_ref(h, &mut |p| valores.push(unsafe { *p }));
            for v in valores {
                self.arc_reter_registrando(&mut arc, v);
            }
        }
        self.arc = Some(arc);
    }

    /// A gravação crua de um native que não passa pelo `Heap` (o `_data` do
    /// `Map` em `nativos_hash.rs`, a pilha do JSON): a troca de `antigo` por
    /// `novo` numa posição forte de `h`.
    pub fn arc_gravacao_crua(&mut self, h: Ref, antigo: Ref, novo: Ref) {
        if let Some(b) = self.bloco_do_espaco(h) {
            self.arc_trocou(b, h, antigo, novo);
        }
    }

    /// A foto das ocorrências contadas de `h` antes de uma escrita crua
    /// (uma por época): do velho no modo do berçário, de todo objeto no ARC
    /// puro (o jovem também conta as gravações).
    fn arc_foto(&mut self, b: *mut Cabecalho, h: Ref) {
        // SAFETY: bloco vivo do espaço.
        let e = unsafe { (*b).estado };
        if e != VELHO && e != LEMBRADO && !self.arc.as_ref().is_some_and(|a| a.puro) {
            return;
        }
        if self.arc.as_ref().is_some_and(|a| a.fotos.contains_key(&h)) {
            return;
        }
        let mut valores = Vec::new();
        // SAFETY: as posições são palavras do corpo de um bloco vivo.
        self.posicoes_de_ref(h, &mut |p| valores.push(unsafe { *p }));
        let arc = self.arc.as_deref_mut().expect("ARC ligado");
        valores.retain(|&v| arc.estado.meta(v).is_some());
        arc.fotos.insert(h, valores);
    }

    /// Os jovens promovidos pela marcação menor em curso e os velhos
    /// lembrados, como handles (antes da varredura dos jovens).
    fn dados_da_promocao(&self) -> (Vec<Ref>, Vec<Ref>) {
        let mut promovidos = Vec::new();
        self.objetos.jovens_marcados(&mut |b| promovidos.push(b as i64 + DESLOCAMENTO_DO_HANDLE));
        let lembrados = self.objetos.lembrados.iter().map(|&b| b as i64 + DESLOCAMENTO_DO_HANDLE).collect();
        (promovidos, lembrados)
    }

    /// Liga o ARC (`dartforge_memoria_arc_v1`, na entrada do programa): uma
    /// coleta completa por rastreamento e o registro de todo vivo com o RC
    /// das ocorrências que chegam a ele, incluindo slots proprietários.
    /// Raízes observacionais apenas protegem os objetos que veem.
    pub fn ativar_arc(&mut self) {
        if self.arc.is_some() {
            return;
        }
        self.coletar(false);
        let mut arc = Box::new(ArcDoHeap {
            estado: crate::arc::EstadoDoArc::novo(),
            fotos: crate::hash::HashMap::default(),
            efemeros: EfemerosDoArc::default(),
            conferir: std::env::var("DARTFORGE_ARC_CONFERIR").is_ok_and(|v| v == "1"),
            ciclos: match std::env::var("DARTFORGE_ARC_CICLOS").as_deref() {
                Ok("sempre") => Some(true),
                Ok("nunca") => Some(false),
                _ => None,
            },
            puro: !std::env::var("DARTFORGE_ARC_BERCARIO").is_ok_and(|v| v == "1"),
        });
        arc.estado.traco = std::env::var("DARTFORGE_ARC_TRACO").is_ok_and(|v| v == "1");
        arc.estado.sem_arestas = Some(bloco_sem_referencias);
        let alcancados = self.alcancaveis_por_rastreamento();
        for &x in &alcancados {
            arc.estado.registrar_vivo_em(x, geometria_arc(&self.objetos, x));
        }
        let mut valores = Vec::new();
        for &x in &alcancados {
            // SAFETY: as posições são palavras do corpo de um bloco vivo.
            self.posicoes_de_ref(x, &mut |p| valores.push(unsafe { *p }));
            for v in valores.drain(..) {
                arc.estado.reter(v).unwrap_or_else(|e| falha_do_arc(e));
            }
        }
        for (&p, &(k, v)) in &self.efemeros {
            if arc.estado.vivo(p) && smi::e_handle(k) && smi::e_handle(v) {
                arc.estado.reter(v).unwrap_or_else(|e| falha_do_arc(e));
                arc.efemeros.inserir(p, k, v);
            }
        }
        for h in self.raizes_proprietarias() {
            self.arc_reter_registrando(&mut arc, h);
        }
        for &x in &alcancados {
            arc.estado.revisar(x);
        }
        self.arc = Some(arc);
    }

    /// Os objetos do espaço alcançáveis das raízes, com a regra dos efêmeros
    /// (o valor quando o portador e a chave são alcançados), sem marcas.
    fn alcancaveis_por_rastreamento(&self) -> Vec<Ref> {
        let mut pilha = Vec::new();
        self.raizes(&mut pilha);
        let mut visto: crate::hash::HashSet<Ref> = crate::hash::HashSet::default();
        let mut saida = Vec::new();
        loop {
            while let Some(h) = pilha.pop() {
                if !smi::e_handle(h) || !e_objeto(h) || self.bloco_do_espaco(h).is_none() || !visto.insert(h) {
                    continue;
                }
                saida.push(h);
                // SAFETY: as posições são palavras do corpo de um bloco vivo.
                self.posicoes_de_ref(h, &mut |p| pilha.push(unsafe { *p }));
            }
            let alcancado = |h: &Ref| !smi::e_handle(*h) || visto.contains(h) || (e_objeto(*h) && self.bloco_do_espaco(*h).is_none());
            for (p, (k, v)) in &self.efemeros {
                if alcancado(p) && alcancado(k) && smi::e_handle(*v) && !visto.contains(v) {
                    pilha.push(*v);
                }
            }
            if pilha.is_empty() {
                break;
            }
        }
        saida
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
    fn ponto_fixo_arc(&mut self, arc: &mut ArcDoHeap) {
        let l: crate::hash::HashSet<Ref> = self.alcancaveis_por_rastreamento().into_iter().collect();
        let d: Vec<Ref> = arc
            .estado
            .vivos()
            .filter(|h| !l.contains(h) && arc.estado.meta(*h).is_some_and(|m| m.estado == crate::arc::EstadoArc::Vivo && !m.imortal))
            .collect();
        if arc.estado.traco {
            eprintln!("[arc-traco] ponto fixo: {} alcançados, {} mortos", l.len(), d.len());
        }
        if d.is_empty() {
            return;
        }
        let em_d: crate::hash::HashSet<Ref> = d.iter().copied().collect();
        let saem: Vec<Ref> = arc
            .efemeros
            .contados
            .iter()
            .filter(|&(p, &(k, _))| em_d.contains(p) && !em_d.contains(&k))
            .map(|(&p, _)| p)
            .collect();
        for p in saem {
            if let Some((_, v)) = arc.efemeros.remover(p)
                && !em_d.contains(&v)
            {
                arc.estado.soltar(v).unwrap_or_else(|e| falha_do_arc(e));
            }
        }
        let n = d.len();
        let ArcDoHeap { estado, efemeros, .. } = arc;
        let mut grafo = GrafoDoHeap { heap: self, efemeros };
        estado.descartar(&mut grafo, &d).unwrap_or_else(|e| falha_do_arc(e));
        estado.estatisticas.mortos_por_ciclo += n as u64;
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
    ///    ([`Heap::ponto_fixo_arc`]); com `DARTFORGE_ARC_CICLOS=sempre`, a
    ///    rodada de ciclos em toda sincronização;
    /// 5. as tabelas laterais esquecem os mortos (fracas, efêmeros,
    ///    finalizadores) até não haver morte nova;
    /// 6. na completa, a reclamação: as marcas são os vivos do RC e a
    ///    varredura completa devolve os blocos dos mortos.
    fn sincronizar_arc(&mut self, completa: bool, promovidos: Vec<Ref>, lembrados: Vec<Ref>) {
        let mut arc = self.arc.take().expect("ARC ligado");
        if arc.estado.traco {
            eprintln!("[arc-traco] sincronizar completa={completa} promovidos={promovidos:?} lembrados={lembrados:?} fotos={:?}", arc.fotos.keys().collect::<Vec<_>>());
        }
        let novos: crate::hash::HashSet<Ref> = promovidos.iter().copied().collect();
        for &h in &promovidos {
            arc.estado.registrar_vivo_em(h, geometria_arc(&self.objetos, h));
        }
        // A promoção reconstruiu RC; repõe cada ocorrência proprietária,
        // sem reter novamente os owners dos objetos que já eram velhos.
        for h in self.raizes_proprietarias() {
            if novos.contains(&h) { self.arc_reter_registrando(&mut arc, h); }
        }
        let mut valores = Vec::new();
        let mut soltar: Vec<Ref> = Vec::new();
        for &h in &promovidos {
            // SAFETY: as posições são palavras do corpo de um bloco vivo.
            self.posicoes_de_ref(h, &mut |p| valores.push(unsafe { *p }));
            for v in valores.drain(..) {
                arc.estado.reter(v).unwrap_or_else(|e| falha_do_arc(e));
            }
        }
        for &x in &lembrados {
            if novos.contains(&x) || arc.fotos.contains_key(&x) || !arc.estado.vivo(x) {
                continue;
            }
            // SAFETY: as posições são palavras do corpo de um bloco vivo.
            self.posicoes_de_ref(x, &mut |p| valores.push(unsafe { *p }));
            for v in valores.drain(..) {
                if novos.contains(&v) {
                    arc.estado.reter(v).unwrap_or_else(|e| falha_do_arc(e));
                }
            }
        }
        for (x, foto) in std::mem::take(&mut arc.fotos) {
            if arc.estado.vivo(x) {
                // SAFETY: as posições são palavras do corpo de um bloco vivo.
                self.posicoes_de_ref(x, &mut |p| valores.push(unsafe { *p }));
                for v in valores.drain(..) {
                    arc.estado.reter(v).unwrap_or_else(|e| falha_do_arc(e));
                }
            }
            soltar.extend(foto);
        }
        // Os efêmeros: as entradas vivas de agora contra as contadas.
        let mut atuais: crate::hash::HashSet<Ref> = crate::hash::HashSet::default();
        for (&p, &(k, v)) in &self.efemeros {
            if !(arc.estado.vivo(p) && smi::e_handle(k) && smi::e_handle(v)) {
                continue;
            }
            atuais.insert(p);
            match arc.efemeros.contados.get(&p) {
                Some(&(k0, v0)) if k0 == k && v0 == v => {}
                antes => {
                    if antes.is_some()
                        && let Some((_, v0)) = arc.efemeros.remover(p)
                    {
                        soltar.push(v0);
                    }
                    arc.estado.reter(v).unwrap_or_else(|e| falha_do_arc(e));
                    arc.efemeros.inserir(p, k, v);
                }
            }
        }
        let saidos: Vec<Ref> = arc.efemeros.contados.keys().copied().filter(|p| !atuais.contains(p)).collect();
        for p in saidos {
            if let Some((_, v0)) = arc.efemeros.remover(p) {
                soltar.push(v0);
            }
        }
        for v in soltar {
            arc.estado.soltar(v).unwrap_or_else(|e| falha_do_arc(e));
        }
        for &h in &promovidos {
            arc.estado.revisar(h);
        }
        // Todas as raízes protegem os zeros que veem; os slots proprietários
        // já tiveram suas ocorrências incluídas no RC.
        let mut raizes = Vec::new();
        self.raizes(&mut raizes);
        let raizes: crate::hash::HashSet<Ref> = raizes.into_iter().filter(|&h| smi::e_handle(h)).collect();
        let protegido = |h: Ref| raizes.contains(&h);
        for &h in &raizes {
            arc.estado.candidatar(h);
        }
        // Com `DARTFORGE_ARC_CICLOS=sempre`, a rodada de ciclos se repete a
        // cada volta com morte nova: o descarte (e o efêmero que sai com o
        // portador) solta sobreviventes que podem ter ficado num ciclo sem
        // raiz. Na completa, o ponto fixo decide a vida de todos (§22.3).
        let ciclos = arc.ciclos == Some(true);
        let mut ponto_fixo = completa && arc.ciclos != Some(false);
        let mut prontas = Vec::new();
        let mut nativas = Vec::new();
        let mut finalizar = Vec::new();
        arc.estado.retomar_adiados();
        loop {
            {
                let ArcDoHeap { estado, efemeros, .. } = &mut *arc;
                let mut grafo = GrafoDoHeap { heap: self, efemeros };
                estado.drenar_zeros(&mut grafo, usize::MAX, &protegido).unwrap_or_else(|e| falha_do_arc(e));
                if ciclos {
                    estado.coletar_ciclos(&mut grafo, &protegido).unwrap_or_else(|e| falha_do_arc(e));
                    estado.drenar_zeros(&mut grafo, usize::MAX, &protegido).unwrap_or_else(|e| falha_do_arc(e));
                }
            }
            if ponto_fixo {
                ponto_fixo = false;
                self.ponto_fixo_arc(&mut arc);
                let ArcDoHeap { estado, efemeros, .. } = &mut *arc;
                let mut grafo = GrafoDoHeap { heap: self, efemeros };
                estado.drenar_zeros(&mut grafo, usize::MAX, &protegido).unwrap_or_else(|e| falha_do_arc(e));
            }
            let mortos = arc.estado.tomar_mortos();
            if mortos.is_empty() {
                break;
            }
            let morto: crate::hash::HashSet<Ref> = mortos.into_iter().collect();
            self.fracas.retain(|portador, alvo| {
                if morto.contains(alvo) {
                    *alvo = 0;
                }
                !morto.contains(portador)
            });
            let portadores: Vec<Ref> = self.efemeros.keys().copied().filter(|p| morto.contains(p)).collect();
            for p in portadores {
                self.efemeros.remove(&p);
                if let Some((_, v)) = arc.efemeros.remover(p) {
                    arc.estado.soltar(v).unwrap_or_else(|e| falha_do_arc(e));
                }
            }
            self.anexos.retain_mut(|a| {
                if morto.contains(&a.desanexo) {
                    a.desanexo = 0;
                }
                if !morto.contains(&a.valor) {
                    return true;
                }
                match a.acao {
                    AcaoDeFinalizador::Dart(acao) => prontas.push(acao),
                    AcaoDeFinalizador::Nativa(f, token) => nativas.push((f, token)),
                }
                false
            });
            self.campos_late_inicializados.retain(|(h, _)| !morto.contains(h));
            self.late_novos.retain(|(h, _)| !morto.contains(h));
            self.permanentes.retain(|h| !morto.contains(h));
            self.constantes.retain(|h, _| !morto.contains(h));
            self.codigo_do_tearoff.retain(|h, _| !morto.contains(h));
            self.finalizaveis.retain(|h, &mut par| {
                let fica = !morto.contains(h);
                if !fica {
                    finalizar.push(par);
                }
                fica
            });
        }
        for &acao in &prontas { self.arc_reter_registrando(&mut arc, acao); }
        self.finalizacoes_prontas.extend(prontas);
        if completa {
            self.reclamar_arc(&arc);
        } else {
            self.menores_desde_completa += 1;
        }
        if arc.conferir {
            self.conferir_arc(&mut arc);
        }
        if self.rastrear {
            let e = arc.estado.estatisticas;
            eprintln!(
                "[arc] {} promovidos={} contados={} retains={} releases={} mortos_rc={} mortos_ciclo={} rodadas={} examinados={} candidatos={}",
                if completa { "completa" } else { "menor" },
                promovidos.len(),
                arc.estado.registrados(),
                e.retains,
                e.releases,
                e.mortos_por_rc,
                e.mortos_por_ciclo,
                e.rodadas_de_ciclo,
                e.examinados,
                arc.estado.candidatos()
            );
        }
        self.arc = Some(arc);
        for (finalizador, par) in finalizar {
            finalizador(par);
        }
        for (f, token) in nativas {
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
    fn drenar_arc(&mut self, completa: bool) {
        conferir_coleta_permitida();
        let inicio = std::time::Instant::now();
        // O tempo de cada etapa, para o rastro (raízes, fotos, jovens das
        // raízes, efêmeros, decisão dos jovens, soltura dos jovens mortos,
        // raízes candidatas, zeros/ciclos, memória).
        let mut fases: Vec<u128> = Vec::with_capacity(10);
        if let Some(a) = &self.agenda {
            a.pendente.set(false);
        }
        self.devolver_tlabs();
        self.stats.collections += 1;
        let mut arc = self.arc.take().expect("ARC ligado");
        let mut raizes = Vec::new();
        self.raizes(&mut raizes);
        let protegidos: crate::hash::HashSet<Ref> = raizes.into_iter().filter(|&h| smi::e_handle(h)).collect();
        fases.push(inicio.elapsed().as_micros());
        // 1. As fotos.
        let mut valores = Vec::new();
        let n_fotos = arc.fotos.len();
        for (x, foto) in std::mem::take(&mut arc.fotos) {
            if self.bloco_do_espaco(x).is_some() && !arc.estado.morto(x) {
                // SAFETY: as posições são palavras do corpo de um bloco vivo.
                self.posicoes_de_ref(x, &mut |p| valores.push(unsafe { *p }));
                for v in valores.drain(..) {
                    self.arc_reter_registrando(&mut arc, v);
                }
                // Registrado, o fotografado segue o RC: sem ocorrência nem
                // raiz, morre adiante e solta as que acabou de contar.
                Self::arc_registrar_se_preciso(&self.objetos, &mut arc, x);
            }
            for v in foto {
                arc.estado.soltar(v).unwrap_or_else(|e| falha_do_arc(e));
            }
        }
        fases.push(inicio.elapsed().as_micros());
        // 2. Os jovens que só as raízes veem.
        let mut jovens = Vec::new();
        self.objetos.jovens_entregues(&mut |b| jovens.push(b));
        // Só as raízes podem acrescentar registros nesta etapa. Consultar
        // todos os jovens aqui custava uma busca de metadados por alocação,
        // embora a decisão de cada jovem já ocorra no passo 4.
        for &h in &protegidos {
            if self.filho_jovem(h) && arc.estado.meta(h).is_none() {
                arc.estado.registrar_vivo_em(h, geometria_arc(&self.objetos, h));
            }
        }
        fases.push(inicio.elapsed().as_micros());
        // 3. Os efêmeros: as entradas vivas de agora contra as contadas.
        let mut soltar: Vec<Ref> = Vec::new();
        let mut atuais: crate::hash::HashSet<Ref> = crate::hash::HashSet::default();
        let entradas: Vec<(Ref, Ref, Ref)> = self.efemeros.iter().map(|(&p, &(k, v))| (p, k, v)).collect();
        for (p, k, v) in entradas {
            if !(arc.estado.vivo(p) && smi::e_handle(k) && smi::e_handle(v)) {
                continue;
            }
            atuais.insert(p);
            match arc.efemeros.contados.get(&p) {
                Some(&(k0, v0)) if k0 == k && v0 == v => {}
                antes => {
                    if antes.is_some()
                        && let Some((_, v0)) = arc.efemeros.remover(p)
                    {
                        soltar.push(v0);
                    }
                    self.arc_reter_registrando(&mut arc, v);
                    arc.efemeros.inserir(p, k, v);
                }
            }
        }
        let saidos: Vec<Ref> = arc.efemeros.contados.keys().copied().filter(|p| !atuais.contains(p)).collect();
        for p in saidos {
            if let Some((_, v0)) = arc.efemeros.remover(p) {
                soltar.push(v0);
            }
        }
        for v in soltar {
            arc.estado.soltar(v).unwrap_or_else(|e| falha_do_arc(e));
        }
        fases.push(inicio.elapsed().as_micros());
        // 4. Os jovens: o registrado fica, o resto morreu.
        let mut mortos_jovens: Vec<Ref> = Vec::new();
        let n_jovens = jovens.len();
        for &b in &jovens {
            let h = b as i64 + DESLOCAMENTO_DO_HANDLE;
            if arc.estado.decidir_jovem(h) {
                // SAFETY: bloco entregue desde a última drenagem, ocupado.
                unsafe {
                    (*b).estado = VELHO;
                    marcar_bloco(b);
                }
            } else {
                mortos_jovens.push(h);
            }
        }
        fases.push(inicio.elapsed().as_micros());
        for &h in &mortos_jovens {
            // SAFETY: as posições são palavras do corpo de um bloco ainda não solto.
            self.posicoes_de_ref(h, &mut |p| valores.push(unsafe { *p }));
            for v in valores.drain(..) {
                arc.estado.soltar(v).unwrap_or_else(|e| falha_do_arc(e));
            }
        }
        fases.push(inicio.elapsed().as_micros());
        // As raízes não geram decremento ao sumir: o que elas seguram volta a
        // ser candidato a ciclo.
        for &h in &protegidos {
            arc.estado.candidatar(h);
        }
        let protegido = |h: Ref| protegidos.contains(&h);
        fases.push(inicio.elapsed().as_micros());
        // 5. Zeros, ciclos e o ponto fixo da completa.
        let rodada = completa && arc.ciclos != Some(false);
        let mut ponto_fixo = rodada && !self.efemeros.is_empty();
        let ciclos = arc.ciclos == Some(true) || (rodada && !ponto_fixo);
        let mut prontas = Vec::new();
        let mut nativas = Vec::new();
        let mut finalizar = Vec::new();
        let mut mortos_rc: Vec<Ref> = Vec::new();
        let mut primeira = true;
        arc.estado.retomar_adiados();
        // O laço por partes, para o rastro: zeros, ciclos, ponto fixo dos
        // efêmeros, tabelas laterais (µs).
        let mut laco = [0u128; 4];
        loop {
            let t0 = std::time::Instant::now();
            {
                let ArcDoHeap { estado, efemeros, .. } = &mut *arc;
                let mut grafo = GrafoDoHeap { heap: self, efemeros };
                estado.drenar_zeros(&mut grafo, usize::MAX, &protegido).unwrap_or_else(|e| falha_do_arc(e));
                laco[0] += t0.elapsed().as_micros();
                if ciclos {
                    let t1 = std::time::Instant::now();
                    estado.coletar_ciclos(&mut grafo, &protegido).unwrap_or_else(|e| falha_do_arc(e));
                    estado.drenar_zeros(&mut grafo, usize::MAX, &protegido).unwrap_or_else(|e| falha_do_arc(e));
                    laco[1] += t1.elapsed().as_micros();
                }
            }
            if ponto_fixo {
                let t2 = std::time::Instant::now();
                ponto_fixo = false;
                self.ponto_fixo_arc(&mut arc);
                let ArcDoHeap { estado, efemeros, .. } = &mut *arc;
                let mut grafo = GrafoDoHeap { heap: self, efemeros };
                estado.drenar_zeros(&mut grafo, usize::MAX, &protegido).unwrap_or_else(|e| falha_do_arc(e));
                laco[2] += t2.elapsed().as_micros();
            }
            let t3 = std::time::Instant::now();
            let mut mortos = arc.estado.tomar_mortos();
            mortos_rc.extend_from_slice(&mortos);
            if primeira {
                primeira = false;
                mortos.extend_from_slice(&mortos_jovens);
            }
            if mortos.is_empty() {
                break;
            }
            // As tabelas laterais (pequenas) perdem os mortos: o objeto do
            // espaço sem metadados morreu nesta drenagem — depois do passo 4,
            // todo vivo do espaço é registrado. Um conjunto com os mortos da
            // rodada (milhões, nas árvores que caem de uma vez) custava mais
            // que a cascata.
            drop(mortos);
            let validar = self.validar_handles;
            let espaco = &self.objetos;
            let estado = &arc.estado;
            let morto = |h: &Ref| morreu_na_drenagem(espaco, validar, estado, *h);
            self.fracas.retain(|portador, alvo| {
                if morto(alvo) {
                    *alvo = 0;
                }
                !morto(portador)
            });
            let portadores: Vec<Ref> = self.efemeros.keys().copied().filter(|p| morto(p)).collect();
            // A chave morta zera a entrada (o rompimento da chave registrada
            // já zerou; a do jovem morto, aqui).
            for e in self.efemeros.values_mut() {
                if morto(&e.0) {
                    *e = (0, 0);
                }
            }
            self.anexos.retain_mut(|a| {
                if morto(&a.desanexo) {
                    a.desanexo = 0;
                }
                if !morto(&a.valor) {
                    return true;
                }
                match a.acao {
                    AcaoDeFinalizador::Dart(acao) => prontas.push(acao),
                    AcaoDeFinalizador::Nativa(f, token) => nativas.push((f, token)),
                }
                false
            });
            self.campos_late_inicializados.retain(|(h, _)| !morto(h));
            self.late_novos.retain(|(h, _)| !morto(h));
            self.permanentes.retain(|h| !morto(h));
            self.constantes.retain(|h, _| !morto(h));
            self.codigo_do_tearoff.retain(|h, _| !morto(h));
            self.finalizaveis.retain(|h, &mut par| {
                let fica = !morto(h);
                if !fica {
                    finalizar.push(par);
                }
                fica
            });
            for p in portadores {
                self.efemeros.remove(&p);
                if let Some((_, v)) = arc.efemeros.remover(p) {
                    arc.estado.soltar(v).unwrap_or_else(|e| falha_do_arc(e));
                }
            }
            laco[3] += t3.elapsed().as_micros();
        }
        for &acao in &prontas { self.arc_reter_registrando(&mut arc, acao); }
        self.finalizacoes_prontas.extend(prontas);
        fases.push(inicio.elapsed().as_micros());
        // 6. A memória.
        let (mortos_varridos, mut soltos) = self.objetos.varrer_jovens();
        for &h in &mortos_rc {
            if let Some(b) = self.bloco_do_espaco(h) {
                // SAFETY: bloco do morto pelo RC, fora da lista dos jovens
                // (a varredura acima a esvaziou), que ninguém mais lê.
                soltos += unsafe { self.objetos.soltar_morto(b) };
            }
        }
        self.stats.reclaimed += (mortos_varridos + mortos_rc.len()) as u64;
        self.stats.estimated_bytes = self.stats.estimated_bytes.saturating_sub(soltos);
        self.allocations = 0;
        self.bytes_jovens = 0;
        if completa {
            let vivos = self.objetos.vivos;
            self.menores_desde_completa = 0;
            self.trabalho_na_completa = vivos;
            self.recalcular_gatilhos(vivos);
        } else {
            self.menores_desde_completa += 1;
        }
        fases.push(inicio.elapsed().as_micros());
        if arc.conferir {
            self.conferir_arc(&mut arc);
        }
        if self.rastrear {
            let mut anterior = 0;
            let etapas: Vec<String> = fases
                .iter()
                .map(|&t| {
                    let d = t - anterior;
                    anterior = t;
                    d.to_string()
                })
                .collect();
            let e = arc.estado.estatisticas;
            eprintln!(
                "[arc] {} us={} fases={} laco={}/{}/{}/{} jovens={n_jovens} mortos_jovens={} fotos={n_fotos} contados={} retains={} releases={} mortos_rc={} mortos_ciclo={} rodadas={} examinados={} candidatos={} zeros_vistos={} adiados={}",
                if completa { "completa" } else { "drenagem" },
                inicio.elapsed().as_micros(),
                etapas.join("/"),
                laco[0],
                laco[1],
                laco[2],
                laco[3],
                mortos_jovens.len(),
                arc.estado.registrados(),
                e.retains,
                e.releases,
                e.mortos_por_rc,
                e.mortos_por_ciclo,
                e.rodadas_de_ciclo,
                e.examinados,
                arc.estado.candidatos(),
                e.zeros_vistos,
                e.zeros_adiados
            );
        }
        self.arc = Some(arc);
        for (finalizador, par) in finalizar {
            finalizador(par);
        }
        for (f, token) in nativas {
            // SAFETY: a `NativeFinalizerFunction` do anexo, `void f(void*)`;
            // como na VM, roda durante a coleta e não pode tocar o heap.
            let f: extern "C" fn(usize) = unsafe { std::mem::transmute(f) };
            f(token);
        }
    }

    /// A reclamação física (§19.4): as marcas passam a ser os vivos do RC e
    /// a varredura completa devolve os blocos dos mortos (e solta os corpos
    /// de fora e os anexos deles).
    fn reclamar_arc(&mut self, arc: &ArcDoHeap) {
        self.objetos.limpar_marcas();
        let mut vivos = 0usize;
        for h in arc.estado.vivos() {
            if let Some(b) = self.bloco_do_espaco(h) {
                // SAFETY: bloco vivo do espaço.
                unsafe { marcar_bloco(b) };
                vivos += 1;
            }
        }
        self.objetos.marcados += vivos;
        let (mortos, _, bytes_de_objetos) = self.objetos.varrer();
        self.stats.reclaimed += mortos as u64;
        self.stats.estimated_bytes = bytes_de_objetos.saturating_add(self.externos);
        self.menores_desde_completa = 0;
        self.trabalho_na_completa = vivos;
        self.recalcular_gatilhos(vivos);
    }

    /// `DARTFORGE_ARC_CONFERIR=1` (§20.3, o modo auditor): nenhum objeto
    /// alcançável das raízes morreu, e o RC de cada vivo é a soma das
    /// ocorrências recontadas.
    ///
    /// # Panics
    /// Com a primeira divergência.
    fn conferir_arc(&mut self, arc: &mut ArcDoHeap) {
        let mut pilha = Vec::new();
        self.raizes(&mut pilha);
        let raizes: crate::hash::HashSet<Ref> = pilha.iter().copied().collect();
        let mut origem: crate::hash::HashMap<Ref, Ref> = crate::hash::HashMap::default();
        let mut visto: crate::hash::HashSet<Ref> = crate::hash::HashSet::default();
        while let Some(h) = pilha.pop() {
            if !smi::e_handle(h) || !e_objeto(h) || self.bloco_do_espaco(h).is_none() || !visto.insert(h) {
                continue;
            }
            if !arc.estado.vivo(h) {
                let pai = origem.get(&h).copied();
                let desc = |x: Ref| {
                    let c = self.cabecalho(x);
                    let pos = self.palavras(x).iter().position(|&w| w == h);
                    format!(
                        "cid {} estado {} flags {:#x} n {} posição {pos:?} metadados {:?} foto {}",
                        c.class_id,
                        c.estado,
                        c.flags,
                        c.n,
                        arc.estado.meta(x),
                        arc.fotos.contains_key(&x)
                    )
                };
                panic!(
                    "bug do ARC: objeto alcançável sem RC vivo: handle {h} (cid {}, estado {}), metadados {:?}, raiz {}; alcançado de {pai:?} ({})",
                    self.cabecalho(h).class_id,
                    self.cabecalho(h).estado,
                    arc.estado.meta(h),
                    raizes.contains(&h),
                    pai.map(desc).unwrap_or_default()
                );
            }
            // SAFETY: as posições são palavras do corpo de um bloco vivo.
            self.posicoes_de_ref(h, &mut |p| {
                let v = unsafe { *p };
                if smi::e_handle(v) && !visto.contains(&v) {
                    origem.entry(v).or_insert(h);
                    pilha.push(v);
                }
            });
        }
        let ArcDoHeap { estado, efemeros, .. } = arc;
        let proprietarias = self.raizes_proprietarias();
        let grafo = GrafoDoHeap { heap: self, efemeros };
        if let Err(e) = estado.auditar(&grafo, &proprietarias) {
            if let crate::arc::ErroArc::Auditoria { handle, .. } = e {
                panic!("bug do ARC: {e:?} (cid {})", grafo.heap.cabecalho(handle).class_id);
            }
            falha_do_arc(e);
        }
    }
}

#[cfg(test)]
mod arc_no_heap {
    //! O ARC sobre o heap real (docs/ARC-CICLOS-ESPECIFICACAO.md §19.2, §22.1).
    use super::*;
    use crate::layout::{self, cid, flags};

    fn lista(heap: &mut Heap, len: usize) -> Ref {
        let h = heap.alocar(cid::LIST, layout::palavras_de_lista(len), flags::REFS);
        heap.palavras_mut(h)[0] = len as i64;
        h
    }

    fn heap_arc() -> Heap {
        let mut heap = Heap::new(false);
        heap.verificar = true;
        heap.ativar_arc();
        heap.arc.as_mut().unwrap().conferir = true;
        heap
    }

    #[test]
    fn fila_de_finalizacoes_conta_aliases_e_consumo() {
        for puro in [false, true] {
            let mut heap = heap_arc();
            heap.arc.as_mut().unwrap().puro = puro;
            let acao = heap.alocar_str("duas finalizações");
            heap.adicionar_finalizacao_pronta(acao);
            heap.adicionar_finalizacao_pronta(acao);
            heap.collect();
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(acao).unwrap().rc, 2);
            assert_eq!(heap.concluir_finalizacao_pronta(), Some(acao));
            heap.collect();
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(acao).unwrap().rc, 1);
            heap.encerrar_finalizadores();
            assert_eq!(heap.concluir_finalizacao_pronta(), None);
            heap.collect();
            assert!(!heap.e_objeto_vivo(acao));
        }
    }

    #[test]
    fn anexo_morto_publica_owner_da_acao_na_fila() {
        for (puro, menor) in [(false, false), (false, true), (true, false), (true, true)] {
            let mut heap = heap_arc();
            heap.arc.as_mut().unwrap().puro = puro;
            let quadro = heap.push_frame_proprietario(3);
            let dono = heap.alocar_str("finalizador");
            heap.set_root(quadro, 0, dono);
            let acao = heap.alocar_str("ação pronta");
            heap.set_root(quadro, 1, acao);
            let valor = heap.alocar_str("alvo fraco");
            heap.set_root(quadro, 2, valor);
            heap.anexos.push(AnexoDeFinalizador {
                dono, valor, desanexo: 0, acao: AcaoDeFinalizador::Dart(acao),
            });
            heap.set_root(quadro, 2, 0);
            heap.coletar(menor);
            assert!(heap.anexos.is_empty());
            assert_eq!(heap.finalizacoes_prontas.front(), Some(&acao));
            assert!(!heap.e_objeto_vivo(valor));
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(acao).unwrap().rc, 2);
            heap.pop_frame(quadro);
            heap.collect();
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(acao).unwrap().rc, 1);
            assert_eq!(heap.concluir_finalizacao_pronta(), Some(acao));
            heap.collect();
            assert!(!heap.e_objeto_vivo(acao));
        }
    }

    fn vivo(heap: &Heap, h: Ref) -> bool {
        heap.arc.as_ref().unwrap().estado.vivo(h)
    }

    #[test]
    fn quadros_proprietarios_contam_copias_trocas_e_promocao() {
        for puro in [true, false] {
            let mut heap = heap_arc();
            heap.arc.as_mut().unwrap().puro = puro;
            let dono = heap.push_frame_proprietario(2);
            let observador = heap.push_frame_with_slots(1);
            let a = lista(&mut heap, 0);
            heap.set_root(dono, 0, a);
            heap.set_root(dono, 1, a);
            heap.set_root(observador, 0, a);
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(a).unwrap().rc, 2);
            heap.set_root(dono, 0, a);
            for _ in 0..2 {
                heap.coletar(true);
                assert_eq!(heap.arc.as_ref().unwrap().estado.meta(a).unwrap().rc, 2);
            }
            let b = lista(&mut heap, 0);
            heap.set_root(dono, 0, b);
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(a).unwrap().rc, 1);
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(b).unwrap().rc, 1);
            heap.set_root(dono, 1, 0);
            heap.coletar(true);
            assert!(vivo(&heap, a), "a raiz observacional ainda protege o zero");
            heap.pop_frame(observador);
            heap.pop_frame(dono);
            heap.collect();
            assert!(!vivo(&heap, a) && !vivo(&heap, b));
        }
    }

    #[test]
    fn proprietarios_e_arestas_fortes_contam_ocorrencias_distintas() {
        for puro in [true, false] {
            let mut heap = heap_arc();
            heap.arc.as_mut().unwrap().puro = puro;
            let quadro = heap.push_frame_proprietario(1);
            let a = lista(&mut heap, 1);
            heap.set_root(quadro, 0, a);
            let b = lista(&mut heap, 1);
            heap.root(quadro, b);
            heap.gravar_ref(a, 1, b);
            heap.gravar_ref(b, 1, a);
            heap.collect();
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(a).unwrap().rc, 2);
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(b).unwrap().rc, 2);
            heap.set_root(quadro, 0, smi::de(7).unwrap());
            heap.collect();
            assert!(vivo(&heap, a) && vivo(&heap, b));
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(a).unwrap().rc, 1);
            heap.pop_frame(quadro);
            heap.collect();
            assert!(!vivo(&heap, a) && !vivo(&heap, b));
        }
    }

    #[test]
    fn movimento_proprietario_consumindo_alias_e_retorno_preserva_rc() {
        for puro in [true, false] {
            let mut heap = heap_arc();
            heap.arc.as_mut().unwrap().puro = puro;
            let chamador = heap.push_frame_proprietario(2);
            let chamado = heap.push_frame_proprietario(1);
            let a = lista(&mut heap, 0);
            heap.set_root(chamado, 0, a);
            heap.set_root(chamador, 0, a);
            heap.mover_raiz(chamado, 0, chamador, 0);
            assert_eq!(heap.frames.last().unwrap().1[0], 0);
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(a).unwrap().rc, 1);
            heap.pop_frame(chamado);
            heap.collect();
            assert!(vivo(&heap, a));
            heap.mover_raiz(chamador, 0, chamador, 0);
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(a).unwrap().rc, 1);
            let b = lista(&mut heap, 0);
            heap.set_root(chamador, 1, b);
            heap.mover_raiz(chamador, 0, chamador, 1);
            heap.collect();
            assert!(vivo(&heap, a) && !vivo(&heap, b));
            heap.pop_frame(chamador);
            heap.collect();
            assert!(!vivo(&heap, a));
        }
    }

    #[test]
    fn movimento_recusa_raiz_observacional_antes_de_consumir_owner() {
        let mut heap = heap_arc();
        let dono = heap.push_frame_proprietario(1);
        let observador = heap.push_frame_with_slots(1);
        let a = lista(&mut heap, 0);
        heap.set_root(dono, 0, a);
        let falha = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            heap.mover_raiz(dono, 0, observador, 0);
        }));
        assert!(falha.is_err());
        assert_eq!(heap.frames[0].1[0], a);
        assert_eq!(heap.frames[1].1[0], 0);
        assert_eq!(heap.arc.as_ref().unwrap().estado.meta(a).unwrap().rc, 1);
        heap.collect();
        heap.pop_frame(observador);
        heap.pop_frame(dono);
        heap.collect();
        assert!(!vivo(&heap, a));
    }

    #[test]
    fn construcao_de_closure_transfere_protecao_para_aresta_forte() {
        for puro in [true, false] {
            let mut heap = heap_arc();
            heap.arc.as_mut().unwrap().puro = puro;
            heap.stress = true;
            let quadro = heap.push_frame_proprietario(1);
            let ambiente = heap.novo_contexto(&[(42, false)]);
            heap.set_root(quadro, 0, ambiente);
            let closure = heap.nova_closure(123, (ambiente, true), 0, 0);
            heap.set_root(quadro, 0, closure);
            heap.collect();
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(ambiente).unwrap().rc, 1);
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(closure).unwrap().rc, 1);
            assert_eq!(heap.closure(closure).unwrap().contexto.0, ambiente);
            heap.pop_frame(quadro);
            heap.collect();
            assert!(!vivo(&heap, ambiente) && !vivo(&heap, closure));
        }
    }

    #[test]
    fn owners_temporarios_das_listas_sobrevivem_a_crescimento_e_descompactacao() {
        for puro in [true, false] {
            let mut heap = heap_arc();
            heap.arc.as_mut().unwrap().puro = puro;
            heap.stress = true;
            let quadro = heap.push_frame_proprietario(1);
            let l = heap.nova_expansivel(1, 1, crate::listas::Elemento::Int);
            heap.set_root(quadro, 0, l);
            heap.lista_set(l, 0, Valor::Int(i64::MAX));
            heap.lista_push(l, Valor::Double(2.5));
            heap.collect();
            assert_eq!(heap.lista_forma(l), crate::listas::Elemento::Geral);
            assert!(matches!(heap.lista_get(l, 0), Valor::Ref(r) if matches!(heap.valor(r), Valor::Int(i64::MAX))));
            assert!(matches!(heap.lista_get(l, 1), Valor::Ref(r) if matches!(heap.valor(r), Valor::Double(x) if x == 2.5)));
            assert_eq!(heap.frames.len(), 1, "nenhum quadro temporário pode permanecer aberto");
            let dados = heap.lista_dados(l);
            let caixas = heap.palavras(dados)[1..3].to_vec();
            for &r in &caixas {
                assert_eq!(heap.arc.as_ref().unwrap().estado.meta(r).unwrap().rc, 1);
            }
            heap.pop_frame(quadro);
            heap.collect();
            assert!(!vivo(&heap, l) && !vivo(&heap, dados));
            assert!(caixas.iter().all(|&r| !vivo(&heap, r)));
        }
    }

    #[test]
    fn visao_tipadas_guarda_base_apos_consumir_owners_temporarios() {
        for puro in [true, false] {
            let mut heap = heap_arc();
            heap.arc.as_mut().unwrap().puro = puro;
            heap.stress = true;
            let quadro = heap.push_frame_proprietario(1);
            let base = heap.nova_tipada(crate::tipadas::tipo::UINT8, 8);
            heap.set_root(quadro, 0, base);
            let c = crate::tipadas::cid_da_visao(crate::tipadas::tipo::UINT8, false);
            let primeira = heap.nova_visao(c, base, 2, 6);
            heap.set_root(quadro, 0, primeira);
            let segunda = heap.nova_visao(c, primeira, 1, 3);
            heap.set_root(quadro, 0, segunda);
            heap.collect();
            let vista = heap.tipada(segunda).unwrap();
            assert_eq!(vista.base, Some(base));
            assert_eq!(vista.deslocamento, 3);
            assert_eq!(vista.len, 3);
            assert!(!vivo(&heap, primeira));
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(base).unwrap().rc, 1);
            heap.pop_frame(quadro);
            heap.collect();
            assert!(!vivo(&heap, base) && !vivo(&heap, segunda));
        }
    }

    #[test]
    fn owners_persistentes_contam_aliases_movimento_e_promocao() {
        for puro in [true, false] {
            let mut heap = heap_arc();
            heap.arc.as_mut().unwrap().puro = puro;
            let a = lista(&mut heap, 0);
            heap.set_global_root(10, a);
            heap.set_global_root(20, a);
            heap.set_raiz_do_runtime(0, a);
            for _ in 0..2 {
                heap.coletar(true);
                assert_eq!(heap.arc.as_ref().unwrap().estado.meta(a).unwrap().rc, 3);
            }
            heap.mover_raiz_global(10, 10);
            heap.set_global_root(20, a);
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(a).unwrap().rc, 3);
            heap.mover_raiz_global(10, 20);
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(a).unwrap().rc, 2);
            heap.soltar_raiz_global(20);
            heap.soltar_raiz_global(20);
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(a).unwrap().rc, 1);
            heap.collect();
            assert!(vivo(&heap, a));
            heap.set_raiz_do_runtime(0, 0);
            heap.collect();
            assert!(!vivo(&heap, a));
        }
    }

    #[test]
    fn ativacao_arc_conta_globais_e_slots_runtime_abertos_no_tracing() {
        let mut heap = Heap::new(false);
        let a = lista(&mut heap, 0);
        heap.set_global_root(10, a);
        heap.set_raiz_do_runtime(1, a);
        heap.ativar_arc();
        heap.arc.as_mut().unwrap().conferir = true;
        assert_eq!(heap.arc.as_ref().unwrap().estado.meta(a).unwrap().rc, 2);
        let b = lista(&mut heap, 0);
        heap.set_global_root(10, b);
        assert_eq!(heap.arc.as_ref().unwrap().estado.meta(a).unwrap().rc, 1);
        assert_eq!(heap.arc.as_ref().unwrap().estado.meta(b).unwrap().rc, 1);
        heap.set_raiz_do_runtime(1, 0);
        heap.collect();
        assert!(!vivo(&heap, a) && vivo(&heap, b));
        heap.set_global_root(10, smi::de(7).unwrap());
        heap.collect();
        assert!(!vivo(&heap, b));
    }

    #[test]
    fn tabelas_canonicas_contam_slots_sem_duplicar_registro() {
        for puro in [true, false] {
            let mut heap = heap_arc();
            heap.arc.as_mut().unwrap().puro = puro;
            heap.stress = true;
            let unidades = vec![65u16];
            let texto = heap.alocar_str("A");
            heap.guardar_literal(unidades.clone(), texto);
            heap.guardar_literal(unidades.clone(), texto);
            let tearoff = heap.nova_closure(123, (0, false), 0, 0);
            heap.registrar_tearoff(123, tearoff);
            heap.registrar_tearoff(123, tearoff);
            let valor_enum = heap.enum_value(900, 0, "zero");
            assert_eq!(heap.enum_value(900, 0, "zero"), valor_enum);
            for _ in 0..2 {
                heap.coletar(true);
                for h in [texto, tearoff, valor_enum] {
                    assert_eq!(heap.arc.as_ref().unwrap().estado.meta(h).unwrap().rc, 1);
                }
            }
            assert_eq!(heap.literal(&unidades), Some(texto));
            assert_eq!(heap.tearoff_registrado(123), Some(tearoff));
            let substituto = heap.nova_closure(123, (0, false), 0, 0);
            heap.registrar_tearoff(123, substituto);
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(tearoff).unwrap().rc, 0);
            assert_eq!(heap.arc.as_ref().unwrap().estado.meta(substituto).unwrap().rc, 1);
            heap.collect();
            assert_eq!(heap.tearoff_registrado(123), Some(substituto));
            assert!(!vivo(&heap, tearoff), "o índice de identidade não é owner");
            assert!([texto, substituto, valor_enum].iter().all(|&h| vivo(&heap, h)));
        }
    }

    #[test]
    fn owners_de_mensagem_contam_no_percurso_do_tracing() {
        let mut heap = Heap::new(false);
        let texto = heap.alocar_str("fila");
        let a = heap.reter_owner_mensagem(texto);
        let b = heap.reter_owner_mensagem(texto);
        let antes = heap.stats().roots_scanned;
        heap.collect();
        assert_eq!(heap.stats().roots_scanned - antes, 2);
        assert!(heap.e_objeto_vivo(texto));
        heap.soltar_owner_mensagem(a);
        let antes = heap.stats().roots_scanned;
        heap.collect();
        assert_eq!(heap.stats().roots_scanned - antes, 1);
        heap.soltar_owner_mensagem(b);
        heap.collect();
        assert!(!heap.e_objeto_vivo(texto));
    }

    #[test]
    fn ativacao_arc_conta_quadros_proprietarios_ja_abertos() {
        let mut heap = Heap::new(false);
        let quadro = heap.push_frame_proprietario(0);
        let a = lista(&mut heap, 0);
        heap.root(quadro, a);
        heap.root(quadro, a);
        heap.ativar_arc();
        heap.arc.as_mut().unwrap().conferir = true;
        assert_eq!(heap.arc.as_ref().unwrap().estado.meta(a).unwrap().rc, 2);
        heap.collect();
        heap.pop_frame(quadro);
        heap.collect();
        assert!(!vivo(&heap, a));
    }

    #[test]
    fn arc_promove_conta_e_solta_pelo_rc() {
        let mut heap = heap_arc();
        let quadro = heap.push_frame_with_slots(1);
        let a = lista(&mut heap, 2);
        let b = lista(&mut heap, 1);
        heap.gravar_ref(a, 1, b);
        heap.set_root(quadro, 0, a);
        heap.coletar(true);
        assert!(vivo(&heap, a) && vivo(&heap, b));
        assert_eq!(heap.arc.as_ref().unwrap().estado.meta(b).unwrap().rc, 1);
        // a[0] = null num velho: b perde a única ocorrência e morre.
        heap.gravar_ref(a, 1, 0);
        heap.coletar(true);
        assert!(!vivo(&heap, b));
        heap.pop_frame(quadro);
        heap.coletar(true);
        assert!(!vivo(&heap, a));
        heap.collect();
    }

    #[test]
    fn arc_raizes_registram_jovens_sem_reter_lixo_nem_recontar_velhos() {
        let mut heap = heap_arc();
        let quadro = heap.push_frame_with_slots(2);
        let velho = lista(&mut heap, 0);
        heap.set_root(quadro, 0, velho);
        heap.coletar(true);
        let geracao = heap.arc.as_ref().unwrap().estado.meta(velho).unwrap().geracao;
        let jovem = lista(&mut heap, 0);
        let lixo = lista(&mut heap, 0);
        heap.set_root(quadro, 1, jovem);
        heap.coletar(true);
        assert!(vivo(&heap, velho) && vivo(&heap, jovem));
        assert!(!vivo(&heap, lixo));
        assert_eq!(heap.arc.as_ref().unwrap().estado.meta(velho).unwrap().geracao, geracao);
        assert_eq!(heap.arc.as_ref().unwrap().estado.meta(jovem).unwrap().rc, 0);
        heap.pop_frame(quadro);
        heap.coletar(true);
        assert!(!vivo(&heap, velho) && !vivo(&heap, jovem));
    }

    #[test]
    fn arc_ciclo_morre_na_completa_e_a_raiz_o_segura() {
        let mut heap = heap_arc();
        let quadro = heap.push_frame_with_slots(1);
        let a = lista(&mut heap, 1);
        let b = lista(&mut heap, 1);
        heap.gravar_ref(a, 1, b);
        heap.gravar_ref(b, 1, a);
        heap.set_root(quadro, 0, a);
        heap.collect();
        assert!(vivo(&heap, a) && vivo(&heap, b), "a raiz segura o ciclo");
        heap.pop_frame(quadro);
        heap.coletar(true);
        assert!(vivo(&heap, a), "a menor não roda a rodada de ciclos");
        heap.collect();
        assert!(!vivo(&heap, a) && !vivo(&heap, b));
    }

    #[test]
    fn arc_escrita_crua_num_velho_reconcilia_pela_foto() {
        let mut heap = heap_arc();
        let quadro = heap.push_frame_with_slots(1);
        let a = lista(&mut heap, 2);
        let b = lista(&mut heap, 1);
        let c = lista(&mut heap, 1);
        heap.gravar_ref(a, 1, b);
        heap.gravar_ref(a, 2, c);
        heap.set_root(quadro, 0, a);
        heap.coletar(true);
        // Troca crua: a = [c, c]; b perde, c ganha.
        let p = heap.palavras_mut(a);
        p[1] = c;
        heap.lembrar(a);
        heap.coletar(true);
        assert!(!vivo(&heap, b));
        assert_eq!(heap.arc.as_ref().unwrap().estado.meta(c).unwrap().rc, 2);
        heap.pop_frame(quadro);
        heap.collect();
    }

    // -- Grafos aleatórios com oráculo independente (§26.2) ------------------

    /// xorshift64*: a sequência de cada semente é reproduzível.
    struct Sorteio(u64);

    impl Sorteio {
        fn novo(semente: u64) -> Sorteio {
            Sorteio(semente.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
        }
        fn prox(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            self.0 = x;
            x.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }
        fn ate(&mut self, n: usize) -> usize {
            (self.prox() % n as u64) as usize
        }
    }

    /// O alvo de uma fraca ou a entrada de um efêmero no modelo. O alvo (ou a
    /// chave) que deixou de ser alcançável vira `Pendente`: a coleta pode
    /// zerá-lo a qualquer momento, e a completa tem de zerá-lo.
    #[derive(Clone, Copy, PartialEq, Debug)]
    enum Fraco<T> {
        Vale(T),
        Pendente,
    }

    /// O grafo do lado do teste: listas com os elementos, os slots do quadro,
    /// as fracas e os efêmeros. O alcance é calculado só daqui, sem olhar o
    /// heap: o forte pelas raízes e o ponto fixo condicional dos efêmeros
    /// (o valor de um portador alcançado é alcançado quando a chave é).
    #[derive(Default)]
    struct Modelo {
        campos: crate::hash::HashMap<Ref, Vec<Ref>>,
        raizes: Vec<Ref>,
        fracas: crate::hash::HashMap<Ref, Fraco<Ref>>,
        efemeros: crate::hash::HashMap<Ref, Fraco<(Ref, Ref)>>,
        /// Os registrados no ARC que ficaram inalcançáveis: a completa tem de
        /// matá-los.
        lixo: Vec<Ref>,
    }

    impl Modelo {
        fn alcancaveis(&self) -> crate::hash::HashSet<Ref> {
            let mut vivos = crate::hash::HashSet::default();
            let mut pilha: Vec<Ref> = self.raizes.iter().copied().filter(|&h| h != 0).collect();
            loop {
                while let Some(h) = pilha.pop() {
                    if vivos.insert(h) {
                        pilha.extend(self.campos[&h].iter().copied().filter(|&f| f != 0));
                    }
                }
                for (p, e) in &self.efemeros {
                    if let Fraco::Vale((k, v)) = *e
                        && vivos.contains(p)
                        && vivos.contains(&k)
                        && !vivos.contains(&v)
                    {
                        pilha.push(v);
                    }
                }
                if pilha.is_empty() {
                    return vivos;
                }
            }
        }

        fn usaveis(&self) -> Vec<Ref> {
            let mut v: Vec<Ref> = self.alcancaveis().into_iter().collect();
            v.sort_unstable();
            v
        }

        fn sortear(&self, r: &mut Sorteio) -> Option<Ref> {
            let v = self.usaveis();
            if v.is_empty() { None } else { Some(v[r.ate(v.len())]) }
        }
    }

    /// Depois de uma coleta: o alcançável continua com os elementos do
    /// modelo (e, no ARC, registrado vivo); a fraca e o efêmero de alvo
    /// alcançável continuam, e os de alvo inalcançável viram `Pendente` (a
    /// completa os exige zerados). Os inalcançáveis saem do modelo.
    fn conferir_modelo(heap: &Heap, m: &mut Modelo, completa: bool, arc: bool, contexto: &str) {
        let vivos = m.alcancaveis();
        for &h in &vivos {
            if arc {
                assert!(vivo(heap, h), "{contexto}: alcançável {h:#x} sem RC vivo");
            }
            let p = heap.palavras(h);
            for (i, &f) in m.campos[&h].iter().enumerate() {
                assert_eq!(p[1 + i], f, "{contexto}: elemento {i} de {h:#x}");
            }
        }
        if completa && arc {
            let agora: Vec<Ref> = m.campos.keys().copied().filter(|h| !vivos.contains(h)).collect();
            m.lixo.extend(agora);
            for &h in &m.lixo {
                assert!(
                    !vivo(heap, h),
                    "{contexto}: inalcançável {h:#x} sobreviveu à completa ({:?}; efêmeros {:?}; fracas {:?})",
                    heap.arc.as_ref().unwrap().estado.meta(h),
                    heap.efemeros.iter().filter(|(p, (k, v))| **p == h || *k == h || *v == h).collect::<Vec<_>>(),
                    heap.fracas.iter().filter(|(p, a)| **p == h || **a == h).collect::<Vec<_>>(),
                );
            }
            m.lixo.clear();
        }
        let mortos: Vec<Ref> = m.campos.keys().copied().filter(|h| !vivos.contains(h)).collect();
        for h in mortos {
            m.campos.remove(&h);
            if arc && vivo(heap, h) {
                m.lixo.push(h);
            }
        }
        let campos_do_modelo = &m.campos;
        m.fracas.retain(|p, alvo| {
            if !vivos.contains(p) {
                return false;
            }
            let atual = heap.fracas.get(p).copied().unwrap_or(0);
            match *alvo {
                Fraco::Vale(a) if vivos.contains(&a) => assert_eq!(atual, a, "{contexto}: fraca {p:#x} com alvo vivo"),
                Fraco::Vale(a) => {
                    assert!(atual == a || atual == 0, "{contexto}: fraca {p:#x}");
                    *alvo = Fraco::Pendente;
                }
                Fraco::Pendente => {}
            }
            if completa && *alvo == Fraco::Pendente {
                assert_eq!(
                    atual,
                    0,
                    "{contexto}: fraca {p:#x} não zerada na completa (alvo {:?}; efêmeros {:?}; quem aponta {:?})",
                    heap.arc.as_ref().map(|a| a.estado.meta(atual)),
                    heap.efemeros
                        .iter()
                        .filter(|(q, (k, v))| **q == atual || *k == atual || *v == atual)
                        .map(|(q, kv)| (*q, *kv, vivos.contains(q), vivos.contains(&kv.0), heap.arc.as_ref().map(|a| a.estado.vivo(*q))))
                        .collect::<Vec<_>>(),
                    campos_do_modelo.iter().filter(|(_, c)| c.contains(&atual)).map(|(h, _)| (*h, vivos.contains(h))).collect::<Vec<_>>(),
                );
            }
            true
        });
        m.efemeros.retain(|p, e| {
            if !vivos.contains(p) {
                return false;
            }
            let atual = heap.efemeros.get(p).copied().unwrap_or((0, 0));
            match *e {
                Fraco::Vale((k, v)) if vivos.contains(&k) => assert_eq!(atual, (k, v), "{contexto}: efêmero {p:#x} com chave viva"),
                Fraco::Vale((k, v)) => {
                    assert!(atual == (k, v) || atual == (0, 0), "{contexto}: efêmero {p:#x}");
                    *e = Fraco::Pendente;
                }
                Fraco::Pendente => {}
            }
            if completa && *e == Fraco::Pendente {
                assert_eq!(atual, (0, 0), "{contexto}: efêmero {p:#x} não zerado na completa");
            }
            true
        });
    }

    /// Uma semente: alocações, gravações contadas e cruas, raízes que entram
    /// e saem, fracas e efêmeros, coletas menores e completas, com a auditoria
    /// do ARC (`conferir`) em toda sincronização e o oráculo do modelo depois
    /// de cada coleta.
    fn grafo_aleatorio(semente: u64, arc: bool, ciclos_sempre: bool, passos: usize) {
        const SLOTS: usize = 6;
        let mut r = Sorteio::novo(semente);
        let mut heap = if arc { heap_arc() } else { Heap::new(false) };
        if ciclos_sempre {
            heap.arc.as_mut().unwrap().ciclos = Some(true);
        }
        heap.verificar = true;
        let quadro = heap.push_frame_with_slots(SLOTS);
        let mut m = Modelo { raizes: vec![0; SLOTS], ..Modelo::default() };
        for passo in 0..passos {
            let contexto = format!("semente {semente}, passo {passo}, arc {arc}");
            match r.ate(20) {
                0..=4 => {
                    let len = 1 + r.ate(3);
                    let h = lista(&mut heap, len);
                    let s = r.ate(SLOTS);
                    heap.set_root(quadro, s, h);
                    m.raizes[s] = h;
                    m.campos.insert(h, vec![0; len]);
                }
                5..=8 => {
                    if let Some(a) = m.sortear(&mut r) {
                        let i = r.ate(m.campos[&a].len());
                        let b = if r.ate(5) == 0 { 0 } else { m.sortear(&mut r).unwrap_or(0) };
                        if r.ate(3) == 0 {
                            heap.palavras_mut(a)[1 + i] = b;
                            heap.lembrar(a);
                        } else {
                            heap.gravar_ref(a, 1 + i, b);
                        }
                        m.campos.get_mut(&a).unwrap()[i] = b;
                    }
                }
                9..=10 => {
                    let s = r.ate(SLOTS);
                    heap.set_root(quadro, s, 0);
                    m.raizes[s] = 0;
                }
                11 => {
                    if let Some(h) = m.sortear(&mut r) {
                        let s = r.ate(SLOTS);
                        heap.set_root(quadro, s, h);
                        m.raizes[s] = h;
                    }
                }
                12 => {
                    if let (Some(p), Some(a)) = (m.sortear(&mut r), m.sortear(&mut r))
                        && !m.efemeros.contains_key(&p)
                    {
                        heap.fracas.insert(p, a);
                        m.fracas.insert(p, Fraco::Vale(a));
                    }
                }
                13..=14 => {
                    if let (Some(p), Some(k), Some(v)) = (m.sortear(&mut r), m.sortear(&mut r), m.sortear(&mut r))
                        && !m.fracas.contains_key(&p)
                    {
                        heap.efemeros.insert(p, (k, v));
                        m.efemeros.insert(p, Fraco::Vale((k, v)));
                    }
                }
                15..=17 => {
                    heap.coletar(true);
                    conferir_modelo(&heap, &mut m, false, arc, &contexto);
                }
                _ => {
                    heap.collect();
                    conferir_modelo(&heap, &mut m, true, arc, &contexto);
                }
            }
        }
        for s in 0..SLOTS {
            heap.set_root(quadro, s, 0);
            m.raizes[s] = 0;
        }
        heap.collect();
        conferir_modelo(&heap, &mut m, true, arc, &format!("semente {semente}, fim, arc {arc}"));
        if arc {
            assert_eq!(heap.arc.as_ref().unwrap().estado.vivos().count(), 0, "semente {semente}: sobrou vivo sem raiz");
        }
        heap.pop_frame(quadro);
    }

    /// Quantas sementes: `DF_ARC_SEMENTES` amplia a busca fora do CI.
    fn sementes(padrao: u64) -> u64 {
        std::env::var("DF_ARC_SEMENTES").ok().and_then(|v| v.parse().ok()).unwrap_or(padrao)
    }

    #[test]
    fn arc_grafos_aleatorios_contra_o_modelo() {
        for semente in 0..sementes(150) {
            grafo_aleatorio(semente, true, false, 400);
        }
    }

    /// Com a rodada de ciclos em toda sincronização (a *trial deletion*
    /// também nas menores, além do ponto fixo da completa).
    #[test]
    fn arc_grafos_aleatorios_com_ciclos_sempre() {
        for semente in 0..sementes(150) {
            grafo_aleatorio(semente.wrapping_add(1 << 32), true, true, 400);
        }
    }

    /// O mesmo oráculo contra o rastreamento: valida o modelo.
    #[test]
    fn arc_grafos_aleatorios_o_modelo_vale_no_rastreamento() {
        for semente in 0..sementes(60) {
            grafo_aleatorio(semente, false, false, 400);
        }
    }
}
