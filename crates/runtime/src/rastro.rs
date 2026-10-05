// O rastro no formato da VM (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
// §13.14): a tabela endereço → (função, linha, coluna) que cada imagem
// compilada com `--rastro=simbolico` registra na partida, os endereços de
// retorno guardados no `throw` e a simbolização, feita só quando o texto é
// pedido (`toString`, a exceção não capturada).
//
// A entrada da tabela (`emit_native/src/llvm/rastro.rs`) tem 12 bytes: o
// rótulo antes da chamada e o registro da função, cada um como deslocamento
// a partir do próprio campo, e `linha << 12 | coluna`. O registro da função
// é o deslocamento até a url e o nome, ambos terminados em zero.
//
// Fragmento do runtime: sem `use` (os fragmentos são um programa só).
// Escrito sem compilar nem executar (2026-10-05).

/// As seções do rastro registradas, `(começo, fim)`.
static SECOES_DO_RASTRO: std::sync::Mutex<Vec<(usize, usize)>> = std::sync::Mutex::new(Vec::new());
/// Alguma imagem registrou a tabela: o `throw` guarda os endereços.
static HA_TABELA_DE_RASTRO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// As entradas de todas as seções, pelo rótulo, e quantas seções entraram:
/// montado no primeiro pedido de texto, refeito quando chega seção nova.
static INDICE_DO_RASTRO: std::sync::RwLock<(usize, Vec<PontoDoRastro>)> = std::sync::RwLock::new((0, Vec::new()));

/// Quantos quadros o rastro guarda (os pares ficam em campos de um objeto,
/// até 2¹⁶ por objeto).
const QUADROS_NO_RASTRO: usize = 1024;

/// O texto de hoje, sem tabela ou sem quadro Dart.
const RASTRO_SEM_TABELA: &str = "#0      main (dart:native)\n";

/// Uma entrada da tabela, já com os endereços absolutos.
#[derive(Clone, Copy)]
struct PontoDoRastro {
    rotulo: usize,
    registro: usize,
    linha_coluna: u32,
}

thread_local! {
    /// Os endereços de retorno do último `throw` desta thread (retorno,
    /// começo da função), de dentro para fora; vazio sem tabela.
    static RETORNOS_DO_LANCAMENTO: std::cell::RefCell<Vec<(u64, u64)>> = const { std::cell::RefCell::new(Vec::new()) };
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

/// Alguma imagem registrou a tabela do rastro.
fn ha_tabela_de_rastro() -> bool {
    HA_TABELA_DE_RASTRO.load(std::sync::atomic::Ordering::Acquire)
}

/// Os endereços de retorno da pilha daqui para fora, com tabela; vazio sem.
fn capturar_retornos() -> Vec<(u64, u64)> {
    if !ha_tabela_de_rastro() {
        return Vec::new();
    }
    crate::heap::enderecos_de_retorno(QUADROS_NO_RASTRO)
}

/// O `throw`: guarda os endereços de retorno desta pilha (o percurso do
/// desenrolador, sem tocar no heap).
fn guardar_retornos_do_lancamento() {
    if !ha_tabela_de_rastro() {
        return;
    }
    let retornos = capturar_retornos();
    RETORNOS_DO_LANCAMENTO.with(|r| *r.borrow_mut() = retornos);
}

/// Os endereços do último `throw` desta thread (vazio sem tabela).
fn retornos_do_lancamento() -> Vec<(u64, u64)> {
    RETORNOS_DO_LANCAMENTO.with(|r| r.borrow().clone())
}

/// Esquece os endereços do último `throw` (a exceção foi tratada).
fn esquecer_retornos_do_lancamento() {
    RETORNOS_DO_LANCAMENTO.with(|r| r.borrow_mut().clear());
}

/// Lê as entradas de `[inicio, fim)`. A entrada toda zero é sentinela ou
/// enchimento.
///
/// # Safety
/// `[inicio, fim)` é uma seção do rastro mapeada.
#[allow(unsafe_code)]
unsafe fn ler_secao_do_rastro(inicio: usize, fim: usize, pontos: &mut Vec<PontoDoRastro>) {
    let mut p = (inicio + 3) & !3;
    while p + 12 <= fim {
        // SAFETY: `p .. p + 12` está dentro da seção.
        let (rotulo, registro, linha_coluna) = unsafe {
            (
                (p as *const i32).read_unaligned(),
                ((p + 4) as *const i32).read_unaligned(),
                ((p + 8) as *const u32).read_unaligned(),
            )
        };
        if rotulo != 0 || registro != 0 || linha_coluna != 0 {
            pontos.push(PontoDoRastro {
                rotulo: p.wrapping_add_signed(rotulo as isize),
                registro: (p + 4).wrapping_add_signed(registro as isize),
                linha_coluna,
            });
        }
        p += 12;
    }
}

/// Chama `f` com as entradas de todas as seções registradas, pelo rótulo.
#[allow(unsafe_code)]
fn com_indice_do_rastro<R>(f: impl FnOnce(&[PontoDoRastro]) -> R) -> R {
    let secoes = SECOES_DO_RASTRO.lock().unwrap_or_else(|e| e.into_inner()).clone();
    {
        let indice = INDICE_DO_RASTRO.read().unwrap_or_else(|e| e.into_inner());
        if indice.0 == secoes.len() {
            return f(&indice.1);
        }
    }
    let mut pontos = Vec::new();
    for &(inicio, fim) in &secoes {
        // SAFETY: seções registradas por `dartforge_registrar_rastro`.
        unsafe { ler_secao_do_rastro(inicio, fim, &mut pontos) };
    }
    pontos.sort_unstable_by_key(|p| p.rotulo);
    let mut indice = INDICE_DO_RASTRO.write().unwrap_or_else(|e| e.into_inner());
    *indice = (secoes.len(), pontos);
    f(&indice.1)
}

/// O nome e a url do registro de função em `r`.
///
/// # Safety
/// `r` é um registro de função de uma seção registrada.
#[allow(unsafe_code)]
unsafe fn registro_do_rastro(r: usize) -> (String, String) {
    // SAFETY: o formato do registro (`llvm/rastro.rs`): `i32` até a url,
    // depois o nome, os dois terminados em zero.
    unsafe {
        let ate_a_url = (r as *const i32).read_unaligned();
        let url = std::ffi::CStr::from_ptr(r.wrapping_add_signed(ate_a_url as isize) as *const std::ffi::c_char);
        let nome = std::ffi::CStr::from_ptr((r + 4) as *const std::ffi::c_char);
        (nome.to_string_lossy().into_owned(), url.to_string_lossy().into_owned())
    }
}

/// O texto do rastro dos `retornos` no formato da VM:
/// `#<índice>` alinhado à esquerda em 6 colunas, o nome e
/// `(url:linha:coluna)`, a linha e a coluna só quando conhecidas. Cada
/// endereço de retorno acha o maior rótulo abaixo dele dentro da função
/// dele; o que não acha (o runtime, o sistema, função sem posição) fica de
/// fora. Sem quadro nenhum, o texto de hoje.
fn simbolizar_retornos(retornos: &[(u64, u64)]) -> String {
    com_indice_do_rastro(|pontos| simbolizar_com(pontos, retornos))
}

/// [`simbolizar_retornos`] com as entradas `pontos` (pelo rótulo).
#[allow(unsafe_code)]
fn simbolizar_com(pontos: &[PontoDoRastro], retornos: &[(u64, u64)]) -> String {
    use std::fmt::Write as _;
    let mut texto = String::new();
    {
        let mut n = 0usize;
        for &(retorno, inicio) in retornos {
            let retorno = retorno as usize;
            let k = pontos.partition_point(|p| p.rotulo < retorno);
            let Some(p) = k.checked_sub(1).map(|k| pontos[k]) else { continue };
            // A função do quadro: o rótulo tem de estar nela. Sem o começo
            // (o desenrolador não o deu), só um rótulo próximo.
            if (inicio != 0 && p.rotulo < inicio as usize) || (inicio == 0 && retorno - p.rotulo > 1 << 16) {
                continue;
            }
            // SAFETY: o registro de uma entrada da seção.
            let (nome, url) = unsafe { registro_do_rastro(p.registro) };
            let (linha, coluna) = (p.linha_coluna >> 12, p.linha_coluna & 0xFFF);
            let _ = write!(texto, "#{n:<6} {nome} ({url}");
            if linha > 0 {
                let _ = write!(texto, ":{linha}");
                if coluna > 0 {
                    let _ = write!(texto, ":{coluna}");
                }
            }
            texto.push_str(")\n");
            n += 1;
        }
    }
    if texto.is_empty() {
        texto.push_str(RASTRO_SEM_TABELA);
    }
    texto
}

/// Os endereços guardados num `StackTrace` do rastro simbólico: o campo 0
/// nulo (o texto, feito no primeiro pedido), o 1 o número de quadros e os
/// pares (retorno, começo da função) depois, todos brutos. Vazio no
/// `StackTrace` de texto.
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

/// Um `StackTrace` com os endereços de `retornos` (veja
/// [`retornos_do_objeto`]); uma alocação, sem referência a enraizar.
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
/// descrição do runtime): o do campo 0, ou os endereços simbolizados.
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
/// endereços simbolizados agora e guardados no campo 0 para o próximo
/// pedido.
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

    /// Uma seção e dois registros montados à mão: o texto sai no formato da
    /// VM, cada endereço de retorno acha o rótulo da função dele, e o que
    /// não acha fica de fora.
    #[test]
    fn simboliza_no_formato_da_vm() {
        // Os textos: a url em 0, o registro de `main` em 16 e o de `f` em 32.
        let mut dados = vec![0u8; 48];
        dados[..15].copy_from_slice(b"file:///a.dart\0");
        let dados: &'static mut [u8] = Box::leak(dados.into_boxed_slice());
        let base = dados.as_ptr() as usize;
        for (r, nome) in [(16usize, &b"main\0"[..]), (32, &b"f\0"[..])] {
            dados[r..r + 4].copy_from_slice(&(-(r as i32)).to_le_bytes());
            dados[r + 4..r + 4 + nome.len()].copy_from_slice(nome);
        }
        // O código: rótulos em +10 (main) e +50 (f).
        let codigo: &'static [u8] = Box::leak(vec![0u8; 100].into_boxed_slice());
        let c = codigo.as_ptr() as usize;
        // A seção: duas entradas e uma sentinela zerada no meio.
        let secao: &'static mut [u8] = Box::leak(vec![0u8; 36].into_boxed_slice());
        let s = secao.as_ptr() as usize;
        let mut entrada = |k: usize, rotulo: usize, registro: usize, lc: u32| {
            let e = s + 12 * k;
            secao[12 * k..12 * k + 4].copy_from_slice(&((rotulo as i64 - e as i64) as i32).to_le_bytes());
            secao[12 * k + 4..12 * k + 8].copy_from_slice(&((registro as i64 - (e + 4) as i64) as i32).to_le_bytes());
            secao[12 * k + 8..12 * k + 12].copy_from_slice(&lc.to_le_bytes());
        };
        entrada(0, c + 50, base + 32, 7 << 12);
        entrada(2, c + 10, base + 16, 3 << 12 | 5);
        let mut pontos = Vec::new();
        // SAFETY: a seção acima, viva até o fim do processo.
        unsafe { ler_secao_do_rastro(s, s + 36, &mut pontos) };
        assert_eq!(pontos.len(), 2, "a sentinela não é entrada");
        pontos.sort_unstable_by_key(|p| p.rotulo);
        let c = c as u64;
        let texto = simbolizar_com(&pontos, &[(c + 5, c), (c + 20, c), (c + 60, c + 55), (c + 60, c + 40)]);
        assert_eq!(texto, "#0      main (file:///a.dart:3:5)\n#1      f (file:///a.dart:7)\n");
        assert_eq!(simbolizar_com(&pontos, &[(c + 5, c)]), RASTRO_SEM_TABELA);
    }
}
