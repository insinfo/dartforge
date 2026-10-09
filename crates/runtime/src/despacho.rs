// Runtime nativo: operadores sobre `dynamic`/`num`/`Object` (P2).
//
// TAPA-BURACO, marcado para remoção em P5: com o SDK da fonte, `a + b` sobre
// `num` é a chamada do seletor `+` no `_IntegerImplementation`/`_Double` da
// própria fonte. Até lá, o operador cujo receptor não é objeto do programa
// (o lowering despacha antes os operadores declarados em classes do usuário)
// cai aqui, com a semântica da VM: `/` sempre `double`, `~/` trunca, `%` é o
// resto euclidiano (nunca negativo), `int` com `double` promove, `String +
// String` concatena e `String * int` repete. O resultado é sempre uma
// referência (escalar encaixotado).

/// Códigos dos operadores (o mesmo número no lowering, `operadores.rs`).
const OP_ADD: i64 = 0;
const OP_SUB: i64 = 1;
const OP_MUL: i64 = 2;
const OP_DIV: i64 = 3;
const OP_TDIV: i64 = 4;
const OP_REM: i64 = 5;
const OP_LT: i64 = 6;
const OP_LE: i64 = 7;
const OP_GT: i64 = 8;
const OP_GE: i64 = 9;
const OP_AND: i64 = 10;
const OP_OR: i64 = 11;
const OP_XOR: i64 = 12;
const OP_SHL: i64 = 13;
const OP_SHR: i64 = 14;
const OP_USHR: i64 = 15;
const OP_NEG: i64 = 16;
const OP_NOT: i64 = 17;

fn nome_do_operador(op: i64) -> &'static str {
    match op {
        OP_ADD => "+",
        OP_SUB => "-",
        OP_MUL => "*",
        OP_DIV => "/",
        OP_TDIV => "~/",
        OP_REM => "%",
        OP_LT => "<",
        OP_LE => "<=",
        OP_GT => ">",
        OP_GE => ">=",
        OP_AND => "&",
        OP_OR => "|",
        OP_XOR => "^",
        OP_SHL => "<<",
        OP_SHR => ">>",
        OP_USHR => ">>>",
        OP_NEG => "unary-",
        _ => "~",
    }
}

/// Um operando numérico, lido da caixa.
#[derive(Clone, Copy)]
enum Num {
    I(i64),
    D(f64),
}

fn ler_num(h: i64) -> Option<Num> {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        // `Smi` ou `_Mint` (R10); senão o `_Double`.
        match heap.valor(h) {
            crate::heap::Valor::Int(i) => Some(Num::I(i)),
            crate::heap::Valor::Double(d) => Some(Num::D(d)),
            _ => None,
        }
    })
}

fn e_string(h: i64) -> bool {
    HEAP.with(|heap| heap.borrow().e_texto(h))
}

fn caixa_int(v: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().caixa_int(v))
}

fn caixa_double(v: f64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().caixa_double(v))
}

fn caixa_bool(v: bool) -> i64 {
    Heap::caixa_bool(v)
}

/// `NoSuchMethodError` do operador `op` (receptor sem o operador).
fn nsm_operador(op: i64) -> i64 {
    let nome = HEAP.with(|heap| heap.borrow_mut().alocar_str(nome_do_operador(op)));
    let erro = com_raizes(&[nome], || dartforge_no_such_method_error_new(nome));
    com_raizes(&[erro], || dartforge_exception_throw(erro, 3));
    0
}

fn divisao_por_zero() -> i64 {
    let msg = HEAP.with(|heap| heap.borrow_mut().alocar_str("IntegerDivisionByZeroException"));
    let erro = com_raizes(&[msg], || dartforge_unsupported_error_new(msg));
    com_raizes(&[erro], || dartforge_exception_throw(erro, 3));
    0
}

/// `a + b` de duas listas do núcleo: uma `_GrowableList` nova com os
/// elementos das duas, na forma delas quando é a mesma, e o tipo da da
/// esquerda (`List.+`). `None` se algum lado não é lista.
fn concatenar_listas(a: i64, b: i64) -> Option<i64> {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        if !heap.e_lista(a) || !heap.e_lista(b) {
            return None;
        }
        // Cópias proprietárias de `a`, `b` e da saída protegem a alocação
        // e as caixas quando as formas dos elementos são diferentes.
        let quadro = heap.push_frame_proprietario(0);
        heap.root(quadro, a);
        heap.root(quadro, b);
        let (len_a, len_b) = (heap.lista_len(a), heap.lista_len(b));
        let (forma_a, forma_b) = (heap.lista_forma(a), heap.lista_forma(b));
        let forma = if forma_a == forma_b { forma_a } else { crate::listas::Elemento::Geral };
        let nova = heap.nova_expansivel(len_a + len_b, len_a + len_b, forma);
        heap.root(quadro, nova);
        for i in 0..len_a {
            let v = heap.lista_get(a, i);
            heap.lista_set(nova, i, v);
        }
        for i in 0..len_b {
            let v = heap.lista_get(b, i);
            heap.lista_set(nova, len_a + i, v);
        }
        let tipo = heap.metadado(a);
        if tipo != 0 {
            heap.set_metadado(nova, tipo);
        }
        heap.pop_frame(quadro);
        Some(nova)
    })
}

/// Resto euclidiano do Dart (`int.%`): nunca negativo.
fn resto_int(a: i64, b: i64) -> i64 {
    let r = a.wrapping_rem(b);
    if r < 0 { if b < 0 { r - b } else { r + b } } else { r }
}

fn resto_double(a: f64, b: f64) -> f64 {
    let r = a % b;
    if r < 0.0 { r + b.abs() } else { r }
}

/// `a op b` sobre referências.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_dyn_op(op: i64, a: i64, b: i64) -> i64 {
    if op == OP_ADD && e_string(a) {
        if !e_string(b) {
            let e = dartforge_type_error_new();
            com_raizes(&[e], || dartforge_exception_throw(e, 3));
            return 0;
        }
        return dartforge_string_concat(a, b);
    }
    if op == OP_MUL && e_string(a) {
        return match ler_num(b) {
            Some(Num::I(n)) => dartforge_string_repeat(a, n),
            _ => nsm_operador(op),
        };
    }
    if op == OP_ADD
        && let Some(lista) = concatenar_listas(a, b)
    {
        // `List.+` devolve uma lista nova e expansível.
        return lista;
    }
    let (Some(x), Some(y)) = (ler_num(a), ler_num(b)) else {
        return nsm_operador(op);
    };
    match (x, y) {
        (Num::I(x), Num::I(y)) => match op {
            OP_ADD => caixa_int(x.wrapping_add(y)),
            OP_SUB => caixa_int(x.wrapping_sub(y)),
            OP_MUL => caixa_int(x.wrapping_mul(y)),
            OP_DIV => caixa_double(x as f64 / y as f64),
            OP_TDIV => {
                if y == 0 {
                    divisao_por_zero()
                } else {
                    caixa_int(x.wrapping_div(y))
                }
            }
            OP_REM => {
                if y == 0 {
                    divisao_por_zero()
                } else {
                    caixa_int(resto_int(x, y))
                }
            }
            OP_LT => caixa_bool(x < y),
            OP_LE => caixa_bool(x <= y),
            OP_GT => caixa_bool(x > y),
            OP_GE => caixa_bool(x >= y),
            OP_AND => caixa_int(x & y),
            OP_OR => caixa_int(x | y),
            OP_XOR => caixa_int(x ^ y),
            OP_SHL => caixa_int(if y >= 64 { 0 } else { x.wrapping_shl(y as u32) }),
            OP_SHR => caixa_int(if y >= 64 { x >> 63 } else { x >> y }),
            OP_USHR => caixa_int(if y >= 64 { 0 } else { ((x as u64) >> y) as i64 }),
            _ => nsm_operador(op),
        },
        (x, y) => {
            let f = |n: Num| match n {
                Num::I(i) => i as f64,
                Num::D(d) => d,
            };
            let (x, y) = (f(x), f(y));
            match op {
                OP_ADD => caixa_double(x + y),
                OP_SUB => caixa_double(x - y),
                OP_MUL => caixa_double(x * y),
                OP_DIV => caixa_double(x / y),
                OP_TDIV => {
                    let q = (x / y).trunc();
                    if q.is_finite() {
                        caixa_int(q as i64)
                    } else {
                        divisao_por_zero()
                    }
                }
                OP_REM => caixa_double(resto_double(x, y)),
                OP_LT => caixa_bool(x < y),
                OP_LE => caixa_bool(x <= y),
                OP_GT => caixa_bool(x > y),
                OP_GE => caixa_bool(x >= y),
                _ => nsm_operador(op),
            }
        }
    }
}

/// O elemento `i` de uma lista do núcleo, para o `for-in` e o espalhamento;
/// `None` para outro valor ou índice fora.
fn elemento_iteravel(h: i64, i: i64) -> Option<crate::heap::Valor> {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let i = usize::try_from(i).ok()?;
        (heap.e_lista(h) && i < heap.lista_len(h)).then(|| heap.lista_get(h, i))
    })
}

/// `elemento_iteravel` como referência (escalar encaixotado); outro valor
/// lança `TypeError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_iteravel_get_ref(h: i64, i: i64) -> i64 {
    match elemento_iteravel(h, i) {
        Some(v) => HEAP.with(|heap| heap.borrow_mut().como_ref(v)),
        None => {
            lancar_type_error();
            0
        }
    }
}

/// `elemento_iteravel` pelos bits (a representação escalar do destino).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_iteravel_get_bits(h: i64, i: i64) -> i64 {
    match elemento_iteravel(h, i) {
        Some(crate::heap::Valor::Ref(r)) => r,
        Some(crate::heap::Valor::Int(n)) => n,
        Some(crate::heap::Valor::Double(d)) => d.to_bits() as i64,
        Some(crate::heap::Valor::Bool(b)) => i64::from(b),
        None => {
            lancar_type_error();
            0
        }
    }
}
