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
    HEAP.with(|heap| match heap.borrow().try_get(h) {
        Some(Value::BoxedInt(i)) if h != 0 => Some(Num::I(*i)),
        Some(Value::BoxedDouble(d)) if h != 0 => Some(Num::D(*d)),
        _ => None,
    })
}

fn e_string(h: i64) -> bool {
    h != 0
        && HEAP.with(|heap| {
            matches!(
                heap.borrow().try_get(h),
                Some(Value::String(_) | Value::RawString(_))
            )
        })
}

fn caixa_int(v: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::BoxedInt(v)))
}

fn caixa_double(v: f64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::BoxedDouble(v)))
}

fn caixa_bool(v: bool) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().caixa_bool(v))
}

/// `NoSuchMethodError` do operador `op` (receptor sem o operador).
fn nsm_operador(op: i64) -> i64 {
    let nome = HEAP.with(|heap| {
        heap.borrow_mut()
            .allocate(Value::String(nome_do_operador(op).to_string()))
    });
    let erro = com_raizes(&[nome], || dartforge_no_such_method_error_new(nome));
    com_raizes(&[erro], || dartforge_exception_throw(erro, 3));
    0
}

fn divisao_por_zero() -> i64 {
    let msg = HEAP.with(|heap| {
        heap.borrow_mut()
            .allocate(Value::String("IntegerDivisionByZeroException".to_string()))
    });
    let erro = com_raizes(&[msg], || dartforge_unsupported_error_new(msg));
    com_raizes(&[erro], || dartforge_exception_throw(erro, 3));
    0
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
        return HEAP.with(|heap| heap.borrow_mut().string_concat(a, b));
    }
    if op == OP_MUL && e_string(a) {
        return match ler_num(b) {
            Some(Num::I(n)) => dartforge_string_repeat(a, n),
            _ => nsm_operador(op),
        };
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

/// `-a` e `~a` sobre uma referência.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_dyn_unario(op: i64, a: i64) -> i64 {
    match (op, ler_num(a)) {
        (OP_NEG, Some(Num::I(i))) => caixa_int(i.wrapping_neg()),
        (OP_NEG, Some(Num::D(d))) => caixa_double(-d),
        (OP_NOT, Some(Num::I(i))) => caixa_int(!i),
        _ => nsm_operador(op),
    }
}

/// Comprimento de um record posicional do runtime.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_record_len(h: i64) -> i64 {
    HEAP.with(|heap| match heap.borrow().try_get(h) {
        Some(Value::Record(v)) if h != 0 => v.len() as i64,
        _ => -1,
    })
}

/// Campo posicional `i` de um record, como referência (escalar
/// encaixotado, R5).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_record_get_ref(h: i64, i: i64) -> i64 {
    let v = HEAP.with(|heap| match heap.borrow().get(h) {
        Value::Record(v) => v[usize::try_from(i).expect("índice de record")],
        _ => panic!("record esperado"),
    });
    valor_como_ref(v)
}
