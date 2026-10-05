// O rastro no formato da VM (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
// §13.14): a tabela endereço → (função, linha, coluna) que cada imagem
// compilada com `--rastro=simbolico` registra na partida, os endereços de
// retorno guardados no `throw` e a simbolização, feita só quando o texto é
// pedido (`toString`, a exceção não capturada).
//
// A entrada da tabela (`emit_native/src/llvm/rastro.rs`) tem 12 bytes: o
// rótulo e o registro da função, cada um como deslocamento a partir do
// próprio campo, e uma palavra. Os dois bits baixos do campo do registro (o
// registro é alinhado em 4) dizem a espécie: 0, um ponto de chamada (a
// palavra é `linha << 12 | coluna`; as entradas com o mesmo rótulo são os
// quadros embutidos, de dentro para fora); 1, a identidade de uma função
// pela entrada uniforme das closures dela (a palavra é o token); 2, o
// `await` de cada estado de um corpo `async`, na ordem; 3, o elo de uma
// closure com quem espera (`posição << 2 | direto << 1 | célula`). O
// registro da função é o deslocamento até a url, um byte de marcas e o
// nome, os textos terminados em zero.
//
// **A cadeia de quem espera** (o `AsyncAwareStackUnwinder` da VM,
// `runtime/vm/stack_trace.cc`). O corpo `async` retomado e o
// `_FutureListener.handleValue` empurram o quadro ou o ouvinte na pilha do
// rastro (`dartforge_rastro_entrar`); no `throw`, o percurso dos quadros de
// máquina casa cada quadro marcado com a entrada dele e, achado o primeiro
// corpo `async` já suspenso (ou o ouvinte), segue a cadeia pelo heap:
// quadro → `Completer` → `_Future` → `_FutureListener` → a closure que
// espera, pelos elos das capturas marcadas `@pragma('vm:awaiter-link')`, até
// o quadro do próximo corpo suspenso. Cada um sai como
// `<asynchronous suspension>` seguido do quadro dele, na posição do `await`.
//
// Fragmento do runtime: sem `use` (os fragmentos são um programa só).
// Escrito sem compilar nem executar (2026-10-05).

/// As seções do rastro registradas, `(começo, fim)`.
static SECOES_DO_RASTRO: std::sync::Mutex<Vec<(usize, usize)>> = std::sync::Mutex::new(Vec::new());
/// Alguma imagem registrou a tabela: o `throw` guarda os endereços.
static HA_TABELA_DE_RASTRO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// O que as seções dizem, e quantas seções entraram: montado no primeiro
/// pedido, refeito quando chega seção nova.
static INDICE_DO_RASTRO: std::sync::RwLock<(usize, IndiceDoRastro)> =
    std::sync::RwLock::new((0, IndiceDoRastro { pontos: Vec::new(), funcoes: Vec::new() }));
/// Os campos do `dart:async` que a cadeia de quem espera lê:
/// `(id da classe, "Classe.campo", posição)`.
static CAMPOS_DO_RASTRO: std::sync::Mutex<Vec<(i64, String, usize)>> = std::sync::Mutex::new(Vec::new());

/// Quantos quadros o rastro guarda (os pares ficam em campos de um objeto,
/// até 2¹⁶ por objeto).
const QUADROS_NO_RASTRO: usize = 1024;

/// O texto de hoje, sem tabela ou sem quadro Dart.
const RASTRO_SEM_TABELA: &str = "#0      main (dart:native)\n";

/// As marcas do registro de uma função (`emit_native/src/hir.rs`,
/// `marcas_do_rastro`).
const MARCA_CORPO_ASYNC: u8 = 1;
const MARCA_OCULTA: u8 = 2;
const MARCA_ESCUTA: u8 = 4;

/// O id de classe do quadro de uma função `async` (`lower/async_sm.rs`,
/// `ID_QUADRO_ASYNC`): `[estado, completer, corpo, …]`.
const ID_QUADRO_ASYNC: i64 = 0x3FFF_FF00;

/// Os estados de `_FutureListener` (`future_impl.dart`).
const OUVINTE_THEN: i64 = 1;
const OUVINTE_CATCH_ERROR: i64 = 2;
const OUVINTE_WHEN_COMPLETE: i64 = 8;
const OUVINTE_AWAIT: i64 = 16;

/// A distância máxima entre o endereço gravado por
/// `dartforge_rastro_entrar` (um local dela) e o topo da pilha do quadro que
/// a chamou.
const JANELA_DA_PILHA: usize = 16 * 1024;

/// Os pares guardados de um quadro da cadeia de quem espera (os de um quadro
/// síncrono são `(retorno, começo)`, com os bits altos limpos).
const PAR_ESPERA: u64 = 1 << 62;
const PAR_OUVINTE: u64 = 1 << 61;

/// Um ponto de chamada, já com os endereços absolutos.
#[derive(Clone, Copy)]
struct PontoDoRastro {
    rotulo: usize,
    registro: usize,
    linha_coluna: u32,
}

/// Uma função conhecida pela entrada uniforme das closures dela.
#[derive(Clone, Default)]
struct FuncaoDoRastro {
    registro: usize,
    token: u32,
    /// A posição do `await` de cada estado (o índice `k − 1`).
    esperas: Vec<u32>,
    /// O elo com quem espera: `(posição, direto, célula)`.
    elo: Option<(usize, bool, bool)>,
}

/// As seções lidas.
struct IndiceDoRastro {
    /// Os pontos de chamada pelo rótulo; os do mesmo rótulo, na ordem da
    /// seção, são a cadeia de quadros embutidos.
    pontos: Vec<PontoDoRastro>,
    /// As funções pela entrada, em ordem.
    funcoes: Vec<(usize, FuncaoDoRastro)>,
}

impl IndiceDoRastro {
    /// A cadeia do quadro de `retorno` (o maior rótulo abaixo dele, dentro da
    /// função que começa em `inicio`), de dentro para fora; vazia sem ponto.
    fn cadeia(&self, retorno: u64, inicio: u64) -> &[PontoDoRastro] {
        let r = retorno as usize;
        let k = self.pontos.partition_point(|p| p.rotulo < r);
        let Some(ultimo) = k.checked_sub(1) else { return &[] };
        let rotulo = self.pontos[ultimo].rotulo;
        // A função do quadro: o rótulo tem de estar nela. Sem o começo (o
        // desenrolador não o deu), só um rótulo próximo.
        if (inicio != 0 && rotulo < inicio as usize) || (inicio == 0 && r - rotulo > 1 << 16) {
            return &[];
        }
        let mut primeiro = ultimo;
        while primeiro > 0 && self.pontos[primeiro - 1].rotulo == rotulo {
            primeiro -= 1;
        }
        &self.pontos[primeiro..=ultimo]
    }

    fn funcao(&self, entrada: usize) -> Option<&FuncaoDoRastro> {
        self.funcoes.binary_search_by_key(&entrada, |(e, _)| *e).ok().map(|i| &self.funcoes[i].1)
    }
}

/// Um quadro do rastro guardado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum QuadroDoRastro {
    /// O endereço de retorno e o começo da função.
    Sincrono(u64, u64),
    /// `<asynchronous suspension>`.
    Lacuna,
    /// Quem espera num `await`: a entrada do corpo e o estado.
    Espera(u64, i64),
    /// A closure que escuta um `Future`: a entrada dela.
    Ouvinte(u64),
}

fn par_do_quadro(q: QuadroDoRastro) -> (u64, u64) {
    match q {
        QuadroDoRastro::Sincrono(r, f) => (r, f),
        QuadroDoRastro::Lacuna => (0, 0),
        QuadroDoRastro::Espera(e, k) => (PAR_ESPERA | (k as u64 & (PAR_OUVINTE - 1)), e),
        QuadroDoRastro::Ouvinte(e) => (PAR_OUVINTE, e),
    }
}

fn quadro_do_par((a, b): (u64, u64)) -> QuadroDoRastro {
    if a == 0 && b == 0 {
        QuadroDoRastro::Lacuna
    } else if a & PAR_ESPERA != 0 {
        QuadroDoRastro::Espera(b, (a & !PAR_ESPERA) as i64)
    } else if a == PAR_OUVINTE {
        QuadroDoRastro::Ouvinte(b)
    } else {
        QuadroDoRastro::Sincrono(a, b)
    }
}

thread_local! {
    /// Os pares do último `throw` desta thread (veja [`par_do_quadro`]), de
    /// dentro para fora; vazio sem tabela.
    static RETORNOS_DO_LANCAMENTO: std::cell::RefCell<Vec<(u64, u64)>> = const { std::cell::RefCell::new(Vec::new()) };
    /// A pilha do rastro: `(tipo, valor, endereço)` empurrado por
    /// `dartforge_rastro_entrar` — o quadro de um corpo `async` (tipo 0) ou
    /// o ouvinte de `handleValue` (tipo 1) e um endereço da pilha de máquina
    /// logo abaixo do quadro que chamou.
    static PILHA_DO_RASTRO: std::cell::RefCell<Vec<(i64, i64, usize)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// O registro da tabela do rastro de uma imagem: o módulo compilado com
/// `--rastro=simbolico` chama isto na partida com o começo e o fim da seção
/// (`dfpcl`; as sentinelas `.dfpcl$a`/`$z` no COFF). Idempotente por seção;
/// não aloca no heap do coletor nem lança.
///
/// # Safety
/// `[inicio, fim)` é a seção do rastro de uma imagem carregada, que fica
/// carregada até o fim do processo.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_registrar_rastro(inicio: *const u8, fim: *const u8) {
    let (inicio, fim) = (inicio as usize, fim as usize);
    if inicio == 0 || fim <= inicio {
        return;
    }
    let mut secoes = SECOES_DO_RASTRO.lock().unwrap_or_else(|e| e.into_inner());
    if secoes.iter().any(|&(a, _)| a == inicio) {
        return;
    }
    secoes.push((inicio, fim));
    HA_TABELA_DE_RASTRO.store(true, std::sync::atomic::Ordering::Release);
}

/// A posição de um campo do `dart:async` que a cadeia de quem espera lê
/// (`"Classe.campo"`, `n` bytes em `nome`). Idempotente; não aloca no heap
/// do coletor nem lança.
///
/// # Safety
/// `nome` aponta `n` bytes UTF-8 válidos.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_registrar_campo_do_rastro(classe: i64, nome: *const u8, n: i64, posicao: i64) {
    let (Ok(n), Ok(posicao)) = (usize::try_from(n), usize::try_from(posicao)) else { return };
    if nome.is_null() {
        return;
    }
    // SAFETY: o contrato da função.
    let bytes = unsafe { std::slice::from_raw_parts(nome, n) };
    let Ok(nome) = std::str::from_utf8(bytes) else { return };
    let mut campos = CAMPOS_DO_RASTRO.lock().unwrap_or_else(|e| e.into_inner());
    if !campos.iter().any(|(c, x, _)| *c == classe && x == nome) {
        campos.push((classe, nome.to_string(), posicao));
    }
}

/// Empurra na pilha do rastro o quadro de um corpo `async` (`tipo` 0) ou o
/// ouvinte de `_FutureListener.handleValue` (`tipo` 1) na entrada da função;
/// devolve a profundidade, que a saída passa a [`dartforge_rastro_sair`].
#[unsafe(no_mangle)]
#[inline(never)]
pub extern "C" fn dartforge_rastro_entrar(tipo: i64, valor: i64) -> i64 {
    let marca = std::hint::black_box(0u8);
    let endereco = std::hint::black_box(std::ptr::addr_of!(marca)) as usize;
    PILHA_DO_RASTRO.with(|p| {
        let mut p = p.borrow_mut();
        p.push((tipo, valor, endereco));
        p.len() as i64
    })
}

/// A saída da função que entrou com `profundidade`: corta a pilha do rastro
/// ali (o que funções desenroladas por exceção deixaram em cima cai junto).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rastro_sair(profundidade: i64) {
    let n = usize::try_from(profundidade.saturating_sub(1)).unwrap_or(0);
    PILHA_DO_RASTRO.with(|p| p.borrow_mut().truncate(n));
}

/// Alguma imagem registrou a tabela do rastro.
fn ha_tabela_de_rastro() -> bool {
    HA_TABELA_DE_RASTRO.load(std::sync::atomic::Ordering::Acquire)
}

/// Os pares da pilha daqui para fora (os quadros síncronos e a cadeia de
/// quem espera), com tabela; vazio sem.
fn capturar_retornos() -> Vec<(u64, u64)> {
    if !ha_tabela_de_rastro() {
        return Vec::new();
    }
    let quadros = crate::heap::enderecos_de_retorno(QUADROS_NO_RASTRO);
    let pilha = PILHA_DO_RASTRO.with(|p| p.borrow().clone());
    // Sem nada na pilha do rastro, nenhum quadro casa: só os síncronos (sem
    // montar o índice no `throw`).
    if pilha.is_empty() {
        return quadros.into_iter().map(|(r, f, _)| (r, f)).collect();
    }
    let quadros = com_indice_do_rastro(|indice| heap_sem_emprestimo(|heap| desenrolar(indice, heap, &quadros, &pilha)));
    quadros.into_iter().map(par_do_quadro).collect()
}

/// O `throw`: guarda os pares desta pilha.
fn guardar_retornos_do_lancamento() {
    if !ha_tabela_de_rastro() {
        return;
    }
    let retornos = capturar_retornos();
    RETORNOS_DO_LANCAMENTO.with(|r| *r.borrow_mut() = retornos);
}

/// Os pares do último `throw` desta thread (vazio sem tabela).
fn retornos_do_lancamento() -> Vec<(u64, u64)> {
    RETORNOS_DO_LANCAMENTO.with(|r| r.borrow().clone())
}

/// Esquece os pares do último `throw` (a exceção foi tratada).
fn esquecer_retornos_do_lancamento() {
    RETORNOS_DO_LANCAMENTO.with(|r| r.borrow_mut().clear());
}

// --- a cadeia de quem espera (`AsyncAwareStackUnwinder`) ---------------------

/// O quadro de quem espera em construção (o `AwaiterFrame` da VM): a closure
/// e o próximo objeto da cadeia (um quadro de corpo `async`, um `Completer`,
/// um `_Future`).
#[derive(Default)]
struct QuemEspera {
    closure: Option<i64>,
    proximo: Option<i64>,
}

/// O nome da classe de `h`, se é objeto.
fn classe_de(heap: &crate::heap::Heap, h: i64) -> Option<String> {
    let cid = heap.classe_do_objeto(h)?;
    CLASS_NAMES.with(|m| m.borrow().get(&cid).cloned())
}

/// O campo `campo` do objeto `h` (a posição registrada para a classe dele).
fn campo_do_rastro(heap: &crate::heap::Heap, h: i64, campo: &str) -> Option<crate::heap::Campo> {
    let cid = heap.classe_do_objeto(h)?;
    let posicao = {
        let campos = CAMPOS_DO_RASTRO.lock().unwrap_or_else(|e| e.into_inner());
        campos.iter().find(|(c, nome, _)| *c == cid && nome.rsplit('.').next() == Some(campo)).map(|(_, _, p)| *p)?
    };
    heap.objeto(h)?.get(posicao)
}

/// O campo de referência (não nulo).
fn ref_do_rastro(heap: &crate::heap::Heap, h: i64, campo: &str) -> Option<i64> {
    match campo_do_rastro(heap, h, campo)? {
        (r, true) if r != 0 => Some(r),
        _ => None,
    }
}

/// Um `int` guardado num campo (cru ou em caixa).
fn inteiro_do_campo(heap: &crate::heap::Heap, c: crate::heap::Campo) -> Option<i64> {
    match c {
        (b, false) => Some(b),
        (r, true) => heap.int_de(r),
    }
}

fn e_quadro_async(heap: &crate::heap::Heap, h: i64) -> bool {
    heap.classe_do_objeto(h) == Some(ID_QUADRO_ASYNC)
}

/// O estado do quadro de um corpo `async` (0 antes do primeiro `await`).
fn estado_do_quadro(heap: &crate::heap::Heap, q: i64) -> i64 {
    heap.objeto(q).and_then(|o| o.get(0)).and_then(|c| inteiro_do_campo(heap, c)).unwrap_or(0)
}

/// As marcas do registro de função `r`.
#[allow(unsafe_code)]
fn marcas_do_registro(r: usize) -> u8 {
    // SAFETY: o registro de uma entrada de uma seção registrada (o byte das
    // marcas segue o deslocamento até a url).
    unsafe { ((r + 4) as *const u8).read() }
}

/// O elo da closure `c` com quem espera (`TryGetAwaiterLink`): `None` se a
/// função dela não tem elo; `Some(None)` se o elo é nulo.
fn elo_da_closure(indice: &IndiceDoRastro, heap: &crate::heap::Heap, c: i64) -> Option<Option<i64>> {
    let clo = heap.closure(c)?;
    let f = indice.funcao(clo.codigo as usize)?;
    let valor = if marcas_do_registro(f.registro) & MARCA_CORPO_ASYNC != 0 {
        // A closure do corpo: o quadro na posição 0 do ambiente.
        heap.objeto(clo.contexto.0)?.get(0)?
    } else {
        let (posicao, direto, celula) = f.elo?;
        let v = if direto { clo.contexto } else { heap.objeto(clo.contexto.0)?.get(posicao)? };
        if celula && v.1 { heap.objeto(v.0).and_then(|o| o.get(0)).unwrap_or((0, false)) } else { v }
    };
    Some((valor.1 && valor.0 != 0).then_some(valor.0))
}

/// `ComputeNextFrameFromAwaiterLink` (com `FollowAwaiterLinks`): segue os
/// elos das closures enquanto o elo é outra closure; o último elo que não é
/// closure passa a ser o próximo.
fn seguir_elos(indice: &IndiceDoRastro, heap: &crate::heap::Heap, q: &mut QuemEspera) {
    let mut elo: Option<i64> = None;
    let mut passos = 0;
    while let Some(c) = q.closure {
        elo = None;
        let Some(l) = elo_da_closure(indice, heap, c) else { break };
        elo = l;
        match l {
            Some(x) if heap.closure(x).is_some() && passos < 64 => {
                q.closure = Some(x);
                passos += 1;
            }
            _ => break,
        }
    }
    if let Some(x) = elo
        && heap.closure(x).is_none()
    {
        q.proximo = Some(x);
    }
}

/// `InitializeAwaiterFrameFromFutureListener`.
fn iniciar_pelo_ouvinte(indice: &IndiceDoRastro, heap: &crate::heap::Heap, q: &mut QuemEspera, ouvinte: i64) {
    if classe_de(heap, ouvinte).as_deref() != Some("_FutureListener") {
        *q = QuemEspera::default();
        return;
    }
    let estado = campo_do_rastro(heap, ouvinte, "state").and_then(|c| inteiro_do_campo(heap, c)).unwrap_or(-1);
    let resultado = if [
        OUVINTE_THEN,
        OUVINTE_CATCH_ERROR,
        OUVINTE_WHEN_COMPLETE,
        OUVINTE_THEN | OUVINTE_CATCH_ERROR,
        OUVINTE_THEN | OUVINTE_CATCH_ERROR | OUVINTE_AWAIT,
    ]
    .contains(&estado)
    {
        ref_do_rastro(heap, ouvinte, "result")
    } else {
        None
    };
    q.closure = ref_do_rastro(heap, ouvinte, "callback").filter(|&c| heap.closure(c).is_some());
    q.proximo = resultado;
    seguir_elos(indice, heap, q);
}

/// `UnwindFrameToFutureListener`: o primeiro ouvinte do `_Future` do próximo.
fn ate_o_ouvinte(indice: &IndiceDoRastro, heap: &crate::heap::Heap, q: &mut QuemEspera) {
    let ouvinte = q.proximo.and_then(|f| ref_do_rastro(heap, f, "_resultOrListeners"));
    match ouvinte {
        Some(l) if classe_de(heap, l).as_deref() == Some("_FutureListener") => iniciar_pelo_ouvinte(indice, heap, q, l),
        _ => *q = QuemEspera::default(),
    }
}

/// `UnwindAwaiterFrame`: do quadro suspenso ao `Future` dele (pelo
/// `Completer`), e do `Future` ao ouvinte.
fn desenrolar_quem_espera(indice: &IndiceDoRastro, heap: &crate::heap::Heap, q: &mut QuemEspera) {
    let mut proximo = q.proximo;
    if let Some(p) = proximo
        && e_quadro_async(heap, p)
    {
        // O `completer` do quadro (posição 1).
        proximo = heap.objeto(p).and_then(|o| o.get(1)).and_then(|c| (c.1 && c.0 != 0).then_some(c.0));
    }
    if let Some(p) = proximo {
        match classe_de(heap, p).as_deref() {
            Some("_AsyncAwaitCompleter") => proximo = ref_do_rastro(heap, p, "_future"),
            Some("_SyncCompleter" | "_AsyncCompleter") => proximo = ref_do_rastro(heap, p, "future"),
            _ => {}
        }
    }
    match proximo {
        Some(f) if classe_de(heap, f).as_deref() == Some("_Future") => {
            q.proximo = Some(f);
            ate_o_ouvinte(indice, heap, q);
        }
        _ => *q = QuemEspera::default(),
    }
}

/// `UnwindToAwaiter`.
fn ate_quem_espera(indice: &IndiceDoRastro, heap: &crate::heap::Heap, q: &mut QuemEspera) {
    let mut passos = 0;
    loop {
        desenrolar_quem_espera(indice, heap, q);
        passos += 1;
        if q.closure.is_some() || q.proximo.is_none() || passos > 1024 {
            break;
        }
    }
}

/// A entrada da pilha do rastro do quadro de máquina cujo topo é `sp`: a
/// mais alta ainda livre, do `tipo`, gravada logo abaixo de `sp`. As de cima
/// dela ficam para trás (de quadros mais fundos já casados, ou deixadas por
/// quadros desenrolados).
fn casar(pilha: &[(i64, i64, usize)], topo: &mut usize, tipo: i64, sp: u64) -> Option<i64> {
    let sp = sp as usize;
    if sp == 0 {
        return None;
    }
    for i in (0..*topo).rev() {
        let (t, v, endereco) = pilha[i];
        if t == tipo && endereco < sp && sp - endereco < JANELA_DA_PILHA {
            *topo = i;
            return Some(v);
        }
    }
    None
}

/// O percurso do `AsyncAwareStackUnwinder::Unwind`: os quadros síncronos até
/// o primeiro com quem o espere, e depois a cadeia de quem espera.
fn desenrolar(indice: &IndiceDoRastro, heap: &crate::heap::Heap, quadros: &[(u64, u64, u64)], pilha: &[(i64, i64, usize)]) -> Vec<QuadroDoRastro> {
    let mut saida = Vec::with_capacity(quadros.len() + 8);
    let mut topo = pilha.len();
    let mut q = QuemEspera::default();
    'sincronos: for &(retorno, inicio, sp) in quadros {
        saida.push(QuadroDoRastro::Sincrono(retorno, inicio));
        for p in indice.cadeia(retorno, inicio) {
            let marcas = marcas_do_registro(p.registro);
            if marcas & MARCA_CORPO_ASYNC != 0 {
                // `InitializeAwaiterFrameFromSuspendState`: o corpo retomado
                // (já suspenso uma vez) segue pela cadeia.
                if let Some(quadro) = casar(pilha, &mut topo, 0, sp)
                    && estado_do_quadro(heap, quadro) != 0
                {
                    q = QuemEspera { closure: None, proximo: Some(quadro) };
                    ate_quem_espera(indice, heap, &mut q);
                }
            } else if marcas & MARCA_ESCUTA != 0
                && let Some(ouvinte) = casar(pilha, &mut topo, 1, sp)
            {
                iniciar_pelo_ouvinte(indice, heap, &mut q, ouvinte);
                ate_quem_espera(indice, heap, &mut q);
            }
            if q.closure.is_some() {
                break 'sincronos;
            }
        }
    }
    let mut algum = false;
    let mut passos = 0;
    while let Some(c) = q.closure {
        passos += 1;
        if passos > QUADROS_NO_RASTRO {
            break;
        }
        let entrada = heap.closure(c).map_or(0, |clo| clo.codigo as u64);
        algum = true;
        match q.proximo {
            Some(p) if e_quadro_async(heap, p) => {
                let k = estado_do_quadro(heap, p);
                // `pc == 0`: o corpo já foi retomado (está na pilha).
                if k > 0 {
                    saida.push(QuadroDoRastro::Lacuna);
                    saida.push(QuadroDoRastro::Espera(entrada, k));
                }
            }
            _ => {
                saida.push(QuadroDoRastro::Lacuna);
                saida.push(QuadroDoRastro::Ouvinte(entrada));
            }
        }
        ate_quem_espera(indice, heap, &mut q);
    }
    if algum {
        saida.push(QuadroDoRastro::Lacuna);
    }
    saida
}

// --- as seções e o texto ----------------------------------------------------

/// Lê as entradas de `[inicio, fim)`. A entrada toda zero é sentinela ou
/// enchimento.
///
/// # Safety
/// `[inicio, fim)` é uma seção do rastro mapeada.
#[allow(unsafe_code)]
unsafe fn ler_secao_do_rastro(inicio: usize, fim: usize, pontos: &mut Vec<PontoDoRastro>, funcoes: &mut Vec<(usize, FuncaoDoRastro)>) {
    let mut p = (inicio + 3) & !3;
    while p + 12 <= fim {
        // SAFETY: `p .. p + 12` está dentro da seção.
        let (rotulo, registro, palavra) = unsafe {
            (
                (p as *const i32).read_unaligned(),
                ((p + 4) as *const i32).read_unaligned(),
                ((p + 8) as *const u32).read_unaligned(),
            )
        };
        if rotulo != 0 || registro != 0 || palavra != 0 {
            let rot = p.wrapping_add_signed(rotulo as isize);
            let especie = (registro & 3) as u32;
            let reg = (p + 4).wrapping_add_signed((registro - especie as i32) as isize);
            match especie {
                0 => pontos.push(PontoDoRastro { rotulo: rot, registro: reg, linha_coluna: palavra }),
                _ => {
                    if funcoes.last().is_none_or(|(e, _)| *e != rot) {
                        funcoes.push((rot, FuncaoDoRastro::default()));
                    }
                    let f = &mut funcoes.last_mut().expect("a função acabada de pôr").1;
                    match especie {
                        1 => {
                            f.registro = reg;
                            f.token = palavra;
                        }
                        2 => f.esperas.push(palavra),
                        _ => f.elo = Some(((palavra >> 2) as usize, palavra & 2 != 0, palavra & 1 != 0)),
                    }
                }
            }
        }
        p += 12;
    }
}

/// Chama `f` com o índice de todas as seções registradas.
#[allow(unsafe_code)]
fn com_indice_do_rastro<R>(f: impl FnOnce(&IndiceDoRastro) -> R) -> R {
    let secoes = SECOES_DO_RASTRO.lock().unwrap_or_else(|e| e.into_inner()).clone();
    {
        let indice = INDICE_DO_RASTRO.read().unwrap_or_else(|e| e.into_inner());
        if indice.0 == secoes.len() {
            return f(&indice.1);
        }
    }
    let mut pontos = Vec::new();
    let mut funcoes = Vec::new();
    for &(inicio, fim) in &secoes {
        // SAFETY: seções registradas por `dartforge_registrar_rastro`.
        unsafe { ler_secao_do_rastro(inicio, fim, &mut pontos, &mut funcoes) };
    }
    // Estável: os do mesmo rótulo ficam na ordem da seção (de dentro para
    // fora).
    pontos.sort_by_key(|p| p.rotulo);
    funcoes.sort_by_key(|(e, _)| *e);
    funcoes.dedup_by_key(|(e, _)| *e);
    let mut indice = INDICE_DO_RASTRO.write().unwrap_or_else(|e| e.into_inner());
    *indice = (secoes.len(), IndiceDoRastro { pontos, funcoes });
    f(&indice.1)
}

/// O nome, a url e as marcas do registro de função em `r`.
///
/// # Safety
/// `r` é um registro de função de uma seção registrada.
#[allow(unsafe_code)]
unsafe fn registro_do_rastro(r: usize) -> (String, String, u8) {
    // SAFETY: o formato do registro (`llvm/rastro.rs`): `i32` até a url, o
    // byte das marcas, o nome; os textos terminados em zero.
    unsafe {
        let ate_a_url = (r as *const i32).read_unaligned();
        let url = std::ffi::CStr::from_ptr(r.wrapping_add_signed(ate_a_url as isize) as *const std::ffi::c_char);
        let marcas = ((r + 4) as *const u8).read();
        let nome = std::ffi::CStr::from_ptr((r + 5) as *const std::ffi::c_char);
        (nome.to_string_lossy().into_owned(), url.to_string_lossy().into_owned(), marcas)
    }
}

/// O texto do rastro dos pares no formato da VM: `#<índice>` alinhado à
/// esquerda em 6 colunas, o nome e `(url:linha:coluna)`, a linha e a coluna
/// só quando conhecidas; os quadros embutidos de um ponto, de dentro para
/// fora; os ocultos, fora; e `<asynchronous suspension>` entre os trechos
/// da cadeia de quem espera, uma vez por lacuna. Sem quadro nenhum, o texto
/// de hoje.
fn simbolizar_retornos(pares: &[(u64, u64)]) -> String {
    com_indice_do_rastro(|indice| simbolizar_com(indice, pares))
}

fn escrever_quadro(texto: &mut String, n: usize, nome: &str, url: &str, linha_coluna: u32) {
    use std::fmt::Write as _;
    let (linha, coluna) = (linha_coluna >> 12, linha_coluna & 0xFFF);
    let _ = write!(texto, "#{n:<6} {nome} ({url}");
    if linha > 0 {
        let _ = write!(texto, ":{linha}");
        if coluna > 0 {
            let _ = write!(texto, ":{coluna}");
        }
    }
    texto.push_str(")\n");
}

/// [`simbolizar_retornos`] com o índice dado.
#[allow(unsafe_code)]
fn simbolizar_com(indice: &IndiceDoRastro, pares: &[(u64, u64)]) -> String {
    let mut texto = String::new();
    let mut n = 0usize;
    let mut na_lacuna = false;
    let mut algum_quadro = false;
    for &par in pares {
        match quadro_do_par(par) {
            QuadroDoRastro::Sincrono(retorno, inicio) => {
                for p in indice.cadeia(retorno, inicio) {
                    // SAFETY: o registro de uma entrada da seção.
                    let (nome, url, marcas) = unsafe { registro_do_rastro(p.registro) };
                    if marcas & MARCA_OCULTA != 0 {
                        continue;
                    }
                    escrever_quadro(&mut texto, n, &nome, &url, p.linha_coluna);
                    n += 1;
                    na_lacuna = false;
                    algum_quadro = true;
                }
            }
            QuadroDoRastro::Lacuna => {
                if !na_lacuna {
                    texto.push_str("<asynchronous suspension>\n");
                }
                na_lacuna = true;
            }
            QuadroDoRastro::Espera(entrada, k) => {
                let Some(f) = indice.funcao(entrada as usize) else { continue };
                let posicao = usize::try_from(k).ok().and_then(|k| k.checked_sub(1)).and_then(|i| f.esperas.get(i)).copied().unwrap_or(0);
                // SAFETY: o registro de uma entrada da seção.
                let (nome, url, _) = unsafe { registro_do_rastro(f.registro) };
                escrever_quadro(&mut texto, n, &nome, &url, posicao);
                n += 1;
                na_lacuna = false;
                algum_quadro = true;
            }
            QuadroDoRastro::Ouvinte(entrada) => {
                let Some(f) = indice.funcao(entrada as usize) else { continue };
                // SAFETY: o registro de uma entrada da seção.
                let (nome, url, marcas) = unsafe { registro_do_rastro(f.registro) };
                if marcas & MARCA_OCULTA != 0 {
                    continue;
                }
                escrever_quadro(&mut texto, n, &nome, &url, f.token);
                n += 1;
                na_lacuna = false;
                algum_quadro = true;
            }
        }
    }
    if !algum_quadro {
        return RASTRO_SEM_TABELA.to_string();
    }
    texto
}

/// Os pares guardados num `StackTrace` do rastro simbólico: o campo 0 nulo
/// (o texto, feito no primeiro pedido), o 1 o número de pares e os pares
/// depois, todos brutos. Vazio no `StackTrace` de texto.
fn retornos_do_objeto(heap: &crate::heap::Heap, h: i64) -> Vec<(u64, u64)> {
    let Some(o) = heap.objeto(h) else { return Vec::new() };
    let Some((n, false)) = o.get(1) else { return Vec::new() };
    let n = usize::try_from(n).unwrap_or(0);
    let mut retornos = Vec::with_capacity(n);
    for i in 0..n {
        match (o.get(2 + 2 * i), o.get(3 + 2 * i)) {
            (Some((r, false)), Some((f, false))) => retornos.push((r as u64, f as u64)),
            _ => break,
        }
    }
    retornos
}

/// Um `StackTrace` com os pares `retornos` (veja [`retornos_do_objeto`]);
/// uma alocação, sem referência a enraizar.
fn alocar_rastro_de_retornos(retornos: &[(u64, u64)]) -> i64 {
    let cid = id_da_classe_stack_trace();
    let mut campos: Vec<crate::heap::Campo> = Vec::with_capacity(2 + 2 * retornos.len());
    campos.push((0, true));
    campos.push((retornos.len() as i64, false));
    for &(r, f) in retornos {
        campos.push((r as i64, false));
        campos.push((f as i64, false));
    }
    HEAP.with(|h| h.borrow_mut().novo_objeto(cid, &campos))
}

/// O texto do `StackTrace` `h` como `String` (a exceção não capturada, a
/// descrição do runtime): o do campo 0, ou os pares simbolizados.
/// `None`: `h` não é objeto.
fn texto_do_rastro(h: i64) -> Option<String> {
    let (texto, retornos) = HEAP.with(|heap| {
        let heap = heap.borrow();
        let o = heap.objeto(h)?;
        Some(match o.first() {
            Some((t, true)) if t != 0 => (heap.texto(t).map(|t| t.para_string()), Vec::new()),
            _ => (None, retornos_do_objeto(&heap, h)),
        })
    })?;
    Some(texto.unwrap_or_else(|| simbolizar_retornos(&retornos)))
}

/// `_StackTrace.toString` (a VM o tem em C++): o texto do campo 0, ou os
/// pares simbolizados agora e guardados no campo 0 para o próximo pedido.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rastro_texto(h: i64) -> i64 {
    let (pronto, retornos) = HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.objeto(h).and_then(|o| o.first()) {
            Some((t, true)) if t != 0 => (t, Vec::new()),
            _ => (0, retornos_do_objeto(&heap, h)),
        }
    });
    if pronto != 0 {
        return pronto;
    }
    let texto = if retornos.is_empty() { RASTRO_SEM_TABELA.to_string() } else { simbolizar_retornos(&retornos) };
    let t = com_raizes(&[h], || texto_de_erro(&texto));
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        if heap.objeto(h).is_some_and(|o| !o.is_empty()) {
            heap.definir_campo(h, 0, t, true);
        }
    });
    t
}

#[cfg(test)]
mod testes_rastro {
    use super::*;

    /// Os textos e os registros de um teste: `(base, registro de cada nome)`.
    /// A url em 0; cada registro tem o deslocamento até ela, o byte das
    /// marcas e o nome.
    fn registros(nomes: &[(&str, u8)]) -> Vec<usize> {
        let mut dados = vec![0u8; 16 + 32 * nomes.len()];
        dados[..15].copy_from_slice(b"file:///a.dart\0");
        let dados: &'static mut [u8] = Box::leak(dados.into_boxed_slice());
        let base = dados.as_ptr() as usize;
        let mut saida = Vec::new();
        for (i, (nome, marcas)) in nomes.iter().enumerate() {
            let r = 16 + 32 * i;
            dados[r..r + 4].copy_from_slice(&(-(r as i32)).to_le_bytes());
            dados[r + 4] = *marcas;
            dados[r + 5..r + 5 + nome.len()].copy_from_slice(nome.as_bytes());
            saida.push(base + r);
        }
        saida
    }

    /// Uma seção montada à mão: `(espécie, rótulo, registro, palavra)`, com
    /// uma sentinela zerada no começo.
    fn secao(entradas: &[(u32, usize, usize, u32)]) -> (usize, usize) {
        let secao: &'static mut [u8] = Box::leak(vec![0u8; 12 * (entradas.len() + 1)].into_boxed_slice());
        let s = secao.as_ptr() as usize;
        for (k, &(especie, rotulo, registro, palavra)) in entradas.iter().enumerate() {
            let k = k + 1;
            let e = s + 12 * k;
            secao[12 * k..12 * k + 4].copy_from_slice(&((rotulo as i64 - e as i64) as i32).to_le_bytes());
            let campo = if especie == 3 { 3 } else { (registro as i64 - (e + 4) as i64) as i32 + especie as i32 };
            secao[12 * k + 4..12 * k + 8].copy_from_slice(&campo.to_le_bytes());
            secao[12 * k + 8..12 * k + 12].copy_from_slice(&palavra.to_le_bytes());
        }
        (s, s + secao.len())
    }

    fn indice(entradas: &[(u32, usize, usize, u32)]) -> IndiceDoRastro {
        let (s, f) = secao(entradas);
        let (mut pontos, mut funcoes) = (Vec::new(), Vec::new());
        // SAFETY: a seção acima, viva até o fim do processo.
        unsafe { ler_secao_do_rastro(s, f, &mut pontos, &mut funcoes) };
        pontos.sort_by_key(|p| p.rotulo);
        funcoes.sort_by_key(|(e, _)| *e);
        IndiceDoRastro { pontos, funcoes }
    }

    /// Cada endereço de retorno acha o rótulo da função dele, e o que não
    /// acha fica de fora; os quadros embutidos saem de dentro para fora; os
    /// ocultos, não.
    #[test]
    fn simboliza_no_formato_da_vm() {
        let r = registros(&[("main", 0), ("f", 0), ("g", 0), ("stub", MARCA_OCULTA)]);
        let codigo: &'static [u8] = Box::leak(vec![0u8; 200].into_boxed_slice());
        let c = codigo.as_ptr() as usize;
        let ind = indice(&[
            (0, c + 50, r[1], 7 << 12),
            (0, c + 10, r[0], 3 << 12 | 5),
            // Um ponto com `g` copiada em `main` (de dentro para fora).
            (0, c + 30, r[2], 9 << 12 | 2),
            (0, c + 30, r[0], 4 << 12 | 1),
            (0, c + 120, r[3], 1 << 12),
        ]);
        assert_eq!(ind.pontos.len(), 5, "a sentinela não é entrada");
        let c = c as u64;
        let texto = simbolizar_com(&ind, &[(c + 5, c), (c + 20, c), (c + 35, c), (c + 60, c + 55), (c + 60, c + 40), (c + 125, c + 100)]);
        assert_eq!(
            texto,
            "#0      main (file:///a.dart:3:5)\n#1      g (file:///a.dart:9:2)\n#2      main (file:///a.dart:4:1)\n#3      f (file:///a.dart:7)\n"
        );
        assert_eq!(simbolizar_com(&ind, &[(c + 5, c)]), RASTRO_SEM_TABELA);
    }

    /// A cadeia de quem espera: a lacuna antes de cada quadro e no fim, uma
    /// vez só entre trechos; o quadro de quem espera na posição do `await`
    /// do estado; o ouvinte no token.
    #[test]
    fn escreve_a_cadeia_de_quem_espera() {
        let r = registros(&[("f", MARCA_CORPO_ASYNC), ("main", MARCA_CORPO_ASYNC), ("main.<anonymous closure>", 0)]);
        let codigo: &'static [u8] = Box::leak(vec![0u8; 200].into_boxed_slice());
        let c = codigo.as_ptr() as usize;
        let ind = indice(&[
            (0, c + 10, r[0], 2 << 12 | 3),
            (1, c + 100, r[1], 1 << 12 | 6),
            (2, c + 100, r[1], 5 << 12 | 9),
            (2, c + 100, r[1], 6 << 12 | 9),
            (1, c + 150, r[2], 8 << 12 | 20),
            (3, c + 150, 0, 1 << 2 | 1),
        ]);
        assert_eq!(ind.funcao(c + 100).map(|f| f.esperas.len()), Some(2));
        assert_eq!(ind.funcao(c + 150).and_then(|f| f.elo), Some((1, false, true)));
        let c = c as u64;
        let pares: Vec<(u64, u64)> = [
            QuadroDoRastro::Sincrono(c + 15, c),
            QuadroDoRastro::Lacuna,
            QuadroDoRastro::Espera(c + 100, 2),
            QuadroDoRastro::Lacuna,
            QuadroDoRastro::Lacuna,
            QuadroDoRastro::Ouvinte(c + 150),
            QuadroDoRastro::Lacuna,
        ]
        .into_iter()
        .map(par_do_quadro)
        .collect();
        assert_eq!(
            simbolizar_com(&ind, &pares),
            "#0      f (file:///a.dart:2:3)\n<asynchronous suspension>\n#1      main (file:///a.dart:6:9)\n<asynchronous suspension>\n#2      main.<anonymous closure> (file:///a.dart:8:20)\n<asynchronous suspension>\n"
        );
        for q in [QuadroDoRastro::Lacuna, QuadroDoRastro::Espera(77, 3), QuadroDoRastro::Ouvinte(78), QuadroDoRastro::Sincrono(5, 1)] {
            assert_eq!(quadro_do_par(par_do_quadro(q)), q);
        }
    }

    /// A entrada da pilha do rastro casa com o quadro cujo topo está logo
    /// acima dela; a de um quadro desenrolado fica para trás.
    #[test]
    fn casa_a_pilha_do_rastro_pelo_topo_do_quadro() {
        let pilha = [(0, 11, 9_000usize), (1, 22, 5_000), (0, 33, 4_000)];
        let mut topo = pilha.len();
        // O quadro mais fundo (topo em 5 100): a entrada de 4 000 é de um
        // quadro mais fundo que ele, já desenrolado, e não casa com o tipo.
        assert_eq!(casar(&pilha, &mut topo, 1, 5_100), Some(22));
        assert_eq!(topo, 1);
        assert_eq!(casar(&pilha, &mut topo, 0, 9_050), Some(11));
        assert_eq!(casar(&pilha, &mut topo, 0, 9_050), None);
        assert_eq!(casar(&pilha, &mut 3, 0, 0), None);
    }
}
