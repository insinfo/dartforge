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

fn operator_name(op: i64) -> &'static str {
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

fn read_num(h: i64) -> Option<Num> {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        // `Smi` ou `_Mint` (R10); senão o `_Double`.
        match heap.value(h) {
            crate::heap::Value::Int(i) => Some(Num::I(i)),
            crate::heap::Value::Double(d) => Some(Num::D(d)),
            _ => None,
        }
    })
}

fn e_string(h: i64) -> bool {
    HEAP.with(|heap| heap.borrow().is_string(h))
}

fn box_int(v: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().box_int(v))
}

fn box_double(v: f64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().box_double(v))
}

fn box_bool(v: bool) -> i64 {
    Heap::box_bool(v)
}

/// `NoSuchMethodError` do operador `op` (receptor sem o operador).
fn nsm_operator(op: i64) -> i64 {
    let name = HEAP.with(|heap| heap.borrow_mut().allocate_str(operator_name(op)));
    let error = with_roots(&[name], || dartforge_no_such_method_error_new(name));
    with_roots(&[error], || dartforge_exception_throw(error, 3));
    0
}

fn division_by_zero() -> i64 {
    let msg = HEAP.with(|heap| heap.borrow_mut().allocate_str("IntegerDivisionByZeroException"));
    let error = with_roots(&[msg], || dartforge_unsupported_error_new(msg));
    with_roots(&[error], || dartforge_exception_throw(error, 3));
    0
}

/// `a + b` de duas listas do núcleo: uma `_GrowableList` nova com os
/// elementos das duas, na forma delas quando é a mesma, e o tipo da da
/// esquerda (`List.+`). `None` se algum lado não é lista.
fn concat_lists(a: i64, b: i64) -> Option<i64> {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        if !heap.e_lista(a) || !heap.e_lista(b) {
            return None;
        }
        // Cópias proprietárias de `a`, `b` e da saída protegem a alocação
        // e as caixas quando as formas dos elementos são diferentes.
        let frame = heap.push_owning_frame(0);
        heap.root(frame, a);
        heap.root(frame, b);
        let (len_a, len_b) = (heap.list_len(a), heap.list_len(b));
        let (shape_a, shape_b) = (heap.list_shape(a), heap.list_shape(b));
        let shape = if shape_a == shape_b { shape_a } else { crate::lists::Element::General };
        let new_area = heap.new_growable(len_a + len_b, len_a + len_b, shape);
        heap.root(frame, new_area);
        for i in 0..len_a {
            let v = heap.list_get(a, i);
            heap.list_set(new_area, i, v);
        }
        for i in 0..len_b {
            let v = heap.list_get(b, i);
            heap.list_set(new_area, len_a + i, v);
        }
        let tipo = heap.metadata(a);
        if tipo != 0 {
            heap.set_metadata(new_area, tipo);
        }
        heap.pop_frame(frame);
        Some(new_area)
    })
}

/// Resto euclidiano do Dart (`int.%`): nunca negativo.
fn int_remainder(a: i64, b: i64) -> i64 {
    let r = a.wrapping_rem(b);
    if r < 0 { if b < 0 { r - b } else { r + b } } else { r }
}

fn double_remainder(a: f64, b: f64) -> f64 {
    let r = a % b;
    if r < 0.0 { r + b.abs() } else { r }
}

/// `a op b` sobre referências.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_dyn_op(op: i64, a: i64, b: i64) -> i64 {
    if op == OP_ADD && e_string(a) {
        if !e_string(b) {
            let e = dartforge_type_error_new();
            with_roots(&[e], || dartforge_exception_throw(e, 3));
            return 0;
        }
        return dartforge_string_concat(a, b);
    }
    if op == OP_MUL && e_string(a) {
        return match read_num(b) {
            Some(Num::I(n)) => dartforge_string_repeat(a, n),
            _ => nsm_operator(op),
        };
    }
    if op == OP_ADD
        && let Some(list) = concat_lists(a, b)
    {
        // `List.+` devolve uma lista nova e expansível.
        return list;
    }
    let (Some(x), Some(y)) = (read_num(a), read_num(b)) else {
        return nsm_operator(op);
    };
    match (x, y) {
        (Num::I(x), Num::I(y)) => match op {
            OP_ADD => box_int(x.wrapping_add(y)),
            OP_SUB => box_int(x.wrapping_sub(y)),
            OP_MUL => box_int(x.wrapping_mul(y)),
            OP_DIV => box_double(x as f64 / y as f64),
            OP_TDIV => {
                if y == 0 {
                    division_by_zero()
                } else {
                    box_int(x.wrapping_div(y))
                }
            }
            OP_REM => {
                if y == 0 {
                    division_by_zero()
                } else {
                    box_int(int_remainder(x, y))
                }
            }
            OP_LT => box_bool(x < y),
            OP_LE => box_bool(x <= y),
            OP_GT => box_bool(x > y),
            OP_GE => box_bool(x >= y),
            OP_AND => box_int(x & y),
            OP_OR => box_int(x | y),
            OP_XOR => box_int(x ^ y),
            OP_SHL => box_int(if y >= 64 { 0 } else { x.wrapping_shl(y as u32) }),
            OP_SHR => box_int(if y >= 64 { x >> 63 } else { x >> y }),
            OP_USHR => box_int(if y >= 64 { 0 } else { ((x as u64) >> y) as i64 }),
            _ => nsm_operator(op),
        },
        (x, y) => {
            let f = |n: Num| match n {
                Num::I(i) => i as f64,
                Num::D(d) => d,
            };
            let (x, y) = (f(x), f(y));
            match op {
                OP_ADD => box_double(x + y),
                OP_SUB => box_double(x - y),
                OP_MUL => box_double(x * y),
                OP_DIV => box_double(x / y),
                OP_TDIV => {
                    let q = (x / y).trunc();
                    if q.is_finite() {
                        box_int(q as i64)
                    } else {
                        division_by_zero()
                    }
                }
                OP_REM => box_double(double_remainder(x, y)),
                OP_LT => box_bool(x < y),
                OP_LE => box_bool(x <= y),
                OP_GT => box_bool(x > y),
                OP_GE => box_bool(x >= y),
                _ => nsm_operator(op),
            }
        }
    }
}

/// O elemento `i` de uma lista do núcleo, para o `for-in` e o espalhamento;
/// `None` para outro valor ou índice fora.
fn iterable_element(h: i64, i: i64) -> Option<crate::heap::Value> {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let i = usize::try_from(i).ok()?;
        (heap.e_lista(h) && i < heap.list_len(h)).then(|| heap.list_get(h, i))
    })
}

/// `iterable_element` como referência (escalar encaixotado); outro valor
/// lança `TypeError`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_iteravel_get_ref(h: i64, i: i64) -> i64 {
    match iterable_element(h, i) {
        Some(v) => HEAP.with(|heap| heap.borrow_mut().to_ref(v)),
        None => {
            throw_type_error();
            0
        }
    }
}

/// `iterable_element` pelos bits (a representação escalar do destino).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_iteravel_get_bits(h: i64, i: i64) -> i64 {
    match iterable_element(h, i) {
        Some(crate::heap::Value::Ref(r)) => r,
        Some(crate::heap::Value::Int(n)) => n,
        Some(crate::heap::Value::Double(d)) => d.to_bits() as i64,
        Some(crate::heap::Value::Bool(b)) => i64::from(b),
        None => {
            throw_type_error();
            0
        }
    }
}
