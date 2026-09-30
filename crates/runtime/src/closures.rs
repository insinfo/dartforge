// Runtime nativo: células, contextos e closures (espaço unificado, P2:
// docs/NATIVO-ESPACO-UNIFICADO.md §2.5 e `caixas.rs`).
//
// O código gerado cria e lê células, contextos e closures em linha
// (`llvm/caixas_ir.rs`). As funções abaixo ficam para a ABI: as de criação
// como caminho lento, e as de leitura para o runtime e para o que ainda as
// declara (`llvm/externs.rs`). A ABI de pares `(bits, tag)` (tags 1 int, 2
// bool, 3 referência, 4 double) das externs antigas vira um `Campo`: a
// palavra na representação gravada e se ela é referência.

/// O `Campo` de um par `(bits, tag)` da ABI antiga: só a tag 3 é referência;
/// `bool` é 0/1 e `double`, os bits.
fn campo_de_par(bits: i64, tag: u8) -> crate::heap::Campo {
    match tag {
        1 | 4 => (bits, false),
        2 => (i64::from(bits != 0), false),
        3 => (bits, true),
        _ => panic!("tag de valor inválida"),
    }
}

/// O corpo tipado de `h` se ela é uma closure com a ABI `abi` (a que quem
/// chama espera), senão 0 — e quem chama segue pela entrada uniforme. Só
/// lê o heap.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_closure_tipada(h: i64, abi: i64) -> i64 {
    heap_sem_emprestimo(|heap| match heap.closure(h) {
        Some(c) if c.abi == abi && abi != 0 => c.tipado,
        _ => 0,
    })
}

/// Devolve o tear-off canônico de uma função top-level (mesmo handle sempre).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_tearoff(code_id: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().tearoff(code_id))
}

/// Consulta o código simbólico de uma closure para despacho indireto.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_closure_code(handle: i64) -> i64 {
    HEAP.with(|heap| heap.borrow().closure(handle).expect("closure esperada").codigo)
}

/// Os quatro campos de ninguém (`abi` 0): o que [`dartforge_closure_cabecalho`]
/// devolve para quem não é closure.
static CLOSURE_NENHUMA: [i64; 4] = [0; 4];

/// O endereço dos quatro campos da closure `h` (código, contexto, corpo
/// tipado, ABI — a ordem do antigo `CabecalhoDeClosure`), ou os de
/// [`CLOSURE_NENHUMA`] para quem não é closure. O código gerado lê o bloco
/// direto (`lower/closures.rs`); fica para a ABI.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_closure_cabecalho(h: i64) -> i64 {
    heap_sem_emprestimo(|heap| match heap.closure(h) {
        Some(_) => h + (crate::layout::desl::CORPO as i64 - crate::layout::DESLOCAMENTO_DO_HANDLE),
        None => CLOSURE_NENHUMA.as_ptr() as i64,
    })
}

// --- P1: convenção uniforme das closures (docs/NATIVO-PLANO.md §7.4) --------

/// Lê uma captura mutável como referência. A palavra de um escalar não diz o
/// tipo: sai como `int` (o código gerado lê a célula na representação do
/// local e nunca chama esta).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_cell_get_ref(handle: i64) -> i64 {
    let (bits, e_ref) = HEAP.with(|heap| heap.borrow().celula(handle));
    if e_ref { bits } else { HEAP.with(|heap| heap.borrow_mut().caixa_int(bits)) }
}

/// O endereço da entrada uniforme de uma closure (o código gravado nela). Um
/// valor que não é closure (null, ou outro objeto chamado como função) deixa
/// `NoSuchMethodError` pendente e devolve 0, que o ponto de chamada troca pela
/// entrada `@df_clo_invalido`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_closure_entry(handle: i64) -> i64 {
    let codigo = HEAP.with(|heap| heap.borrow().closure(handle).map(|c| c.codigo));
    // Um objeto de classe com `call` (classe chamável): a entrada do método
    // na tabela da classe tem a mesma convenção (receptor, args, desc).
    codigo.or_else(|| metodo_da_classe(dartforge_value_class(handle), hash_do_nome("c:call")).map(|f| f as i64))
        .unwrap_or_else(|| {
            dartforge_nsm_chamada();
            0
        })
}

/// Lança o `NoSuchMethodError` de uma chamada de valor função que não casa
/// (valor que não é função, ou aridade/nomes errados).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nsm_chamada() {
    let nome = HEAP.with(|heap| heap.borrow_mut().alocar_str("call"));
    let erro = com_raizes(&[nome], || dartforge_no_such_method_error_new(nome));
    com_raizes(&[erro], || dartforge_exception_throw(erro, 3));
}

/// A tupla RTI do corpo de uma closure genérica: os `n_fora` argumentos de
/// tipo de quem a criou (`de_fora`, `0` sem nenhum; o que faltar é
/// `dynamic`) seguidos dos dela — os da chamada (`propria`, a tupla do slot
/// depois dos argumentos) ou, quando a chamada não passou nenhum (`0`), os
/// padrões (`padrao`, a instanciação pelos limites).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_tupla_juntar(de_fora: i64, n_fora: i64, propria: i64, padrao: i64) -> i64 {
    let mut v = argumentos_da_tupla(de_fora);
    v.resize(usize::try_from(n_fora).unwrap_or(0), 0);
    v.extend(argumentos_da_tupla(if propria > 0 { propria } else { padrao }));
    RTI.with(|u| u.borrow_mut().internar(Tipo::Tupla(v)))
}

/// Confere um descritor de chamada `[n_pos, n_nom, hash…]` contra a
/// assinatura `[n_obrig, n_pos, n_nom, hash…, obrigatório…]` da função
/// chamada: posicionais entre os obrigatórios e o total, todo nomeado
/// passado existe, todo nomeado `required` foi passado.
///
/// # Safety
/// Os dois ponteiros são vetores constantes emitidos pelo compilador, com o
/// tamanho que os seus próprios cabeçalhos dizem.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_args_casam(desc: *const i64, sig: *const i64) -> u8 {
    // SAFETY: cabeçalhos de vetores constantes do emissor.
    let (npos, nnom) = unsafe { (*desc, *desc.add(1)) };
    let (obrig, total, snom) = unsafe { (*sig, *sig.add(1), *sig.add(2)) };
    if npos < obrig || npos > total {
        return 0;
    }
    let nnom = usize::try_from(nnom).unwrap_or(0);
    let snom = usize::try_from(snom).unwrap_or(0);
    // SAFETY: os tamanhos vêm dos cabeçalhos.
    let passados = unsafe { std::slice::from_raw_parts(desc.add(2), nnom) };
    let nomes = unsafe { std::slice::from_raw_parts(sig.add(3), snom) };
    let exigidos = unsafe { std::slice::from_raw_parts(sig.add(3 + snom), snom) };
    if passados.iter().any(|h| !nomes.contains(h)) {
        return 0;
    }
    for (h, req) in nomes.iter().zip(exigidos) {
        if *req != 0 && !passados.contains(h) {
            return 0;
        }
    }
    1
}

/// A posição (entre os nomeados) do argumento de nome `hash` no descritor,
/// ou -1 se ele não foi passado.
///
/// # Safety
/// `desc` é um descritor constante emitido pelo compilador.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arg_indice(desc: *const i64, hash: i64) -> i64 {
    // SAFETY: cabeçalho e nomes do descritor constante.
    let nnom = usize::try_from(unsafe { *desc.add(1) }).unwrap_or(0);
    let passados = unsafe { std::slice::from_raw_parts(desc.add(2), nnom) };
    passados
        .iter()
        .position(|h| *h == hash)
        .map_or(-1, |p| p as i64)
}

/// O comprimento de uma lista do núcleo, ou 0 para o que não é lista.
fn comprimento_da_lista(h: i64) -> usize {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        if heap.e_lista(h) { heap.lista_len(h) } else { 0 }
    })
}

/// `Function._apply` da VM recebe `[função, posicionais…, nomeados…]` e os
/// nomes em uma segunda lista, já produzidas pelo patch Dart de `Function.apply`.
/// Recompõe a ABI uniforme das closures, inclusive o descritor de nomes.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Function_apply(arguments: i64, names: i64) -> i64 {
    com_raizes(&[arguments, names], || {
        let count = comprimento_da_lista(arguments);
        let named = comprimento_da_lista(names);
        if count == 0 || named >= count {
            dartforge_nsm_chamada();
            return 0;
        }
        // Os elementos em posição `Ref` (as listas são as gerais do patch; a
        // caixa, se houver, fica viva na própria lista).
        let (function, args, nomes) = HEAP.with(|heap| {
            let mut heap = heap.borrow_mut();
            let function = heap.lista_get_ref(arguments, 0);
            // O slot depois dos argumentos é o da tupla de tipos (nenhuma).
            let args: Vec<i64> = (1..count).map(|i| heap.lista_get_ref(arguments, i)).chain(std::iter::once(0)).collect();
            let nomes: Vec<Option<String>> = (0..named)
                .map(|i| {
                    let v = heap.lista_get_ref(names, i);
                    heap.texto(v).map(|t| t.para_string())
                })
                .collect();
            (function, args, nomes)
        });
        let mut desc = vec![(count - 1 - named) as i64, named as i64];
        for name in nomes {
            let Some(name) = name else {
                dartforge_nsm_chamada();
                return 0;
            };
            // Mesmo FNV-1a 64 do descritor emitido em `lower/closures.rs`.
            let hash = name.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
                (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
            });
            desc.push(hash as i64);
        }
        com_raizes(&args, || {
            let code = dartforge_closure_entry(function);
            if code == 0 { return 0; }
            // SAFETY: `dartforge_closure_entry` devolve o endereço de uma entrada
            // uniforme gerada com assinatura (closure, argumentos, descritor).
            let entry: extern "C" fn(i64, *const i64, *const i64) -> i64 = unsafe { std::mem::transmute(code as usize) };
            entry(function, args.as_ptr(), desc.as_ptr())
        })
    })
}

#[cfg(test)]
mod function_apply_tests {
    use super::*;

    extern "C" fn entry(_closure: i64, args: *const i64, desc: *const i64) -> i64 {
        // O patch Dart fornece 1 posicional e dois nomeados na ordem c,b.
        dartforge_gc_collect();
        let matches = unsafe {
            *desc == 1 && *desc.add(1) == 2
                && *desc.add(2) == hash("c") && *desc.add(3) == hash("b")
                && *args.add(1) == 0 && *args.add(2) == 0
        };
        let value_alive = HEAP.with(|heap| heap.borrow().texto(unsafe { *args }).is_some_and(|t| t.para_string() == "valor"));
        i64::from(matches && value_alive)
    }

    fn hash(name: &str) -> i64 {
        name.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
        }) as i64
    }

    #[test]
    fn function_apply_encaminha_descritor_nomeado_a_entrada_uniforme() {
        use crate::heap::Valor;
        use crate::listas::Elemento;
        let function = dartforge_tearoff(entry as usize as i64);
        com_raizes(&[function], || {
            let c = HEAP.with(|heap| heap.borrow_mut().alocar_str("c"));
            com_raizes(&[c], || {
                let b = HEAP.with(|heap| heap.borrow_mut().alocar_str("b"));
                com_raizes(&[b], || {
                    let value = HEAP.with(|heap| heap.borrow_mut().alocar_str("valor"));
                    com_raizes(&[value], || {
                        let arguments = HEAP.with(|heap| {
                            let mut heap = heap.borrow_mut();
                            let l = heap.nova_lista(crate::layout::cid::LIST, 4, Elemento::Geral);
                            heap.lista_set(l, 0, Valor::Ref(function));
                            heap.lista_set(l, 1, Valor::Ref(value));
                            l
                        });
                        com_raizes(&[arguments], || {
                            let names = HEAP.with(|heap| {
                                let mut heap = heap.borrow_mut();
                                let l = heap.nova_lista(crate::layout::cid::LIST, 2, Elemento::Geral);
                                heap.lista_set(l, 0, Valor::Ref(c));
                                heap.lista_set(l, 1, Valor::Ref(b));
                                l
                            });
                            assert_eq!(dartforge_nativo_Function_apply(arguments, names), 1);
                        });
                    });
                });
            });
        });
    }
}
