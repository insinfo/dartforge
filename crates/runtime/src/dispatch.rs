// Native runtime: operators on `dynamic`/`num`/`Object` (P2).
//
// STOPGAP, marked for removal in P5: with the SDK from source, `a + b` on `num`
// is the call of the `+` selector on the source's own
// `_IntegerImplementation`/`_Double`. Until then, an operator whose receiver is
// not a program object (lowering dispatches the operators declared in user
// classes first) lands here, with the VM semantics: `/` is always `double`, `~/`
// truncates, `%` is the Euclidean remainder (never negative), `int` with `double`
// promotes, `String + String` concatenates and `String * int` repeats. The result
// is always a reference (boxed scalar).

/// Operator codes (the same number in lowering, `operators.rs`).
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

/// A numeric operand, read from the box.
#[derive(Clone, Copy)]
enum Num {
    I(i64),
    D(f64),
}

fn read_num(h: i64) -> Option<Num> {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        // `Smi` or `_Mint` (R10); otherwise the `_Double`.
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

/// `NoSuchMethodError` of operator `op` (receiver without the operator).
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

/// `a + b` of two core lists: a new `_GrowableList` with the elements of both,
/// in their shape when it is the same, and the type of the left one (`List.+`).
/// `None` if either side is not a list.
fn concat_lists(a: i64, b: i64) -> Option<i64> {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        if !heap.is_list(a) || !heap.is_list(b) {
            return None;
        }
        // Owning copies of `a`, `b` and of the output protect the allocation
        // and the boxes when the element shapes differ.
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
        let ty = heap.metadata(a);
        if ty != 0 {
            heap.set_metadata(new_area, ty);
        }
        heap.pop_frame(frame);
        Some(new_area)
    })
}

/// Dart's Euclidean remainder (`int.%`): never negative.
fn int_remainder(a: i64, b: i64) -> i64 {
    let r = a.wrapping_rem(b);
    if r < 0 { if b < 0 { r - b } else { r + b } } else { r }
}

fn double_remainder(a: f64, b: f64) -> f64 {
    let r = a % b;
    if r < 0.0 { r + b.abs() } else { r }
}

/// `a op b` on references.
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
        // `List.+` returns a new growable list.
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

/// Element `i` of a core list, for `for-in` and spreading; `None` for another
/// value or an out-of-range index.
fn iterable_element(h: i64, i: i64) -> Option<crate::heap::Value> {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let i = usize::try_from(i).ok()?;
        (heap.is_list(h) && i < heap.list_len(h)).then(|| heap.list_get(h, i))
    })
}

/// `iterable_element` as a reference (boxed scalar); any other value throws
/// `TypeError`.
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

/// `iterable_element` through the bits (the destination's scalar representation).
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
