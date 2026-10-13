// Native runtime: cells, contexts and closures (unified space, P2:
// docs/NATIVO-ESPACO-UNIFICADO.md §2.5 and `boxes.rs`).
//
// Generated code creates and reads cells, contexts and closures inline
// (`llvm/caixas_ir.rs`). The functions below remain for the ABI: the creation
// ones as the slow path, and the reading ones for the runtime and for whatever
// still declares them (`llvm/externs.rs`). The `(bits, tag)` pair ABI (tags 1
// int, 2 bool, 3 reference, 4 double) of the old externs becomes a `Field`: the
// word in the written representation and whether it is a reference.

/// The `Field` of a `(bits, tag)` pair of the old ABI: only tag 3 is a reference;
/// `bool` is 0/1 and `double`, its bits.
fn campo_de_par(bits: i64, tag: u8) -> crate::heap::Field {
    match tag {
        1 | 4 => (bits, false),
        2 => (i64::from(bits != 0), false),
        3 => (bits, true),
        _ => panic!("invalid value tag"),
    }
}

/// The typed body of `h` if it is a closure with ABI `abi` (the one the caller
/// expects), otherwise 0 — and the caller goes through the uniform entry. Only
/// reads the heap.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_closure_tipada(h: i64, abi: i64) -> i64 {
    heap_without_borrow(|heap| match heap.closure(h) {
        Some(c) if c.abi == abi && abi != 0 => c.typed_one,
        _ => 0,
    })
}

/// Returns the canonical tear-off of a top-level function (always the same handle).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_tearoff(code_id: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().tearoff(code_id))
}

/// [`dartforge_tearoff`] under ARC: the canonical tear-off with an owner of the
/// code, independent of the registration that keeps it. May allocate the first
/// time; neither throws nor runs Dart.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let a = dartforge_arc_tearoff_owned_v1(77);
/// assert_eq!(a, dartforge_arc_tearoff_owned_v1(77));
/// dartforge_arc_release(a);
/// dartforge_arc_release(a);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_tearoff_owned_v1(code_id: i64) -> i64 {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let h = heap.tearoff(code_id);
        heap.retain_code_owner(h);
        h
    })
}

/// Queries the symbolic code of a closure for indirect dispatch.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_closure_code(handle: i64) -> i64 {
    HEAP.with(|heap| heap.borrow().closure(handle).expect("closure esperada").code)
}

/// The four fields of nobody (`abi` 0): what [`dartforge_closure_cabecalho`]
/// returns for something that is not a closure.
static CLOSURE_NONE: [i64; 4] = [0; 4];

/// The address of the four fields of closure `h` (code, context, typed body, ABI
/// — the order of the old `CabecalhoDeClosure`), or those of [`CLOSURE_NONE`] for
/// something that is not a closure. Generated code reads the block directly
/// (`lower/closures.rs`); this remains for the ABI.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_closure_cabecalho(h: i64) -> i64 {
    heap_without_borrow(|heap| match heap.closure(h) {
        Some(_) => h + (crate::layout::desl::CORPO as i64 - crate::layout::DESLOCAMENTO_DO_HANDLE),
        None => CLOSURE_NONE.as_ptr() as i64,
    })
}

// --- P1: uniform closure convention (docs/NATIVO-PLANO.md §7.4) ------------

/// Reads a mutable capture as a reference. The word of a scalar does not tell the
/// type: it comes out as `int` (generated code reads the cell in the local's
/// representation and never calls this).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_cell_get_ref(handle: i64) -> i64 {
    let (bits, is_ref_kind) = HEAP.with(|heap| heap.borrow().cell(handle));
    if is_ref_kind { bits } else { HEAP.with(|heap| heap.borrow_mut().box_int(bits)) }
}

/// The address of a closure's uniform entry (the code written in it). A value
/// that is not a closure (null, or another object called as a function) leaves a
/// pending `NoSuchMethodError` and returns 0, which the call site replaces with
/// the `@df_clo_invalido` entry.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_closure_entry(handle: i64) -> i64 {
    let code = HEAP.with(|heap| heap.borrow().closure(handle).map(|c| c.code));
    // An object of a class with `call` (callable class): the method's entry in
    // the class table has the same convention (receiver, args, desc).
    code.or_else(|| class_method(dartforge_value_class(handle), name_hash("c:call")).map(|f| f as i64))
        .unwrap_or_else(|| {
            dartforge_nsm_chamada();
            0
        })
}

/// Throws the `NoSuchMethodError` of a function-value call that does not match
/// (a value that is not a function, or wrong arity/names).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nsm_chamada() {
    let name = HEAP.with(|heap| heap.borrow_mut().allocate_str("call"));
    let error = with_roots(&[name], || dartforge_no_such_method_error_new(name));
    with_roots(&[error], || dartforge_exception_throw(error, 3));
}

/// The RTI tuple of a generic closure's body: the `n_outside` type arguments of
/// its creator (`from_outside`, `0` with none; whatever is missing is `dynamic`)
/// followed by its own — those of the call (`own`, the tuple in the slot after
/// the arguments) or, when the call passed none (`0`), the defaults (`pattern`,
/// the instantiation to bounds).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_rti_tupla_juntar(from_outside: i64, n_outside: i64, own: i64, pattern: i64) -> i64 {
    let mut v = tuple_arguments(from_outside);
    v.resize(usize::try_from(n_outside).unwrap_or(0), 0);
    v.extend(tuple_arguments(if own > 0 { own } else { pattern }));
    RTI.with(|u| u.borrow_mut().intern(Type::Tuple(v)))
}

/// Checks a call descriptor `[n_pos, n_nom, hash…]` against the signature
/// `[n_required, n_pos, n_nom, hash…, required…]` of the called function:
/// positionals between the required count and the total, every named argument
/// passed exists, every `required` named argument was passed.
///
/// # Safety
/// Both pointers are constant vectors emitted by the compiler, with the size
/// their own headers state.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_args_casam(desc: *const i64, sig: *const i64) -> u8 {
    // SAFETY: headers of the emitter's constant vectors.
    let (npos, nnom) = unsafe { (*desc, *desc.add(1)) };
    let (required_count, total, snom) = unsafe { (*sig, *sig.add(1), *sig.add(2)) };
    if npos < required_count || npos > total {
        return 0;
    }
    let nnom = usize::try_from(nnom).unwrap_or(0);
    let snom = usize::try_from(snom).unwrap_or(0);
    // SAFETY: the sizes come from the headers.
    let passed = unsafe { std::slice::from_raw_parts(desc.add(2), nnom) };
    let name_list = unsafe { std::slice::from_raw_parts(sig.add(3), snom) };
    let required = unsafe { std::slice::from_raw_parts(sig.add(3 + snom), snom) };
    if passed.iter().any(|h| !name_list.contains(h)) {
        return 0;
    }
    for (h, req) in name_list.iter().zip(required) {
        if *req != 0 && !passed.contains(h) {
            return 0;
        }
    }
    1
}

/// The position (among the named ones) of the argument named `hash` in the
/// descriptor, or -1 if it was not passed.
///
/// # Safety
/// `desc` is a constant descriptor emitted by the compiler.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arg_indice(desc: *const i64, hash: i64) -> i64 {
    // SAFETY: header and names of the constant descriptor.
    let nnom = usize::try_from(unsafe { *desc.add(1) }).unwrap_or(0);
    let passed = unsafe { std::slice::from_raw_parts(desc.add(2), nnom) };
    passed
        .iter()
        .position(|h| *h == hash)
        .map_or(-1, |p| p as i64)
}

/// The length of a core list, or 0 for whatever is not a list.
fn list_length(h: i64) -> usize {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        if heap.e_lista(h) { heap.list_len(h) } else { 0 }
    })
}

/// The VM's `Function._apply` receives `[function, positional…, named…]` and the
/// names in a second list, already produced by the Dart patch of `Function.apply`.
/// Rebuilds the uniform closure ABI, including the names descriptor.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Function_apply(arguments: i64, names: i64) -> i64 {
    with_roots(&[arguments, names], || {
        let count = list_length(arguments);
        let named = list_length(names);
        if count == 0 || named >= count {
            dartforge_nsm_chamada();
            return 0;
        }
        // The elements in a `Ref` position (the lists are the patch's general
        // ones; the box, if any, stays alive in the list itself).
        let (function, args, name_list) = HEAP.with(|heap| {
            let mut heap = heap.borrow_mut();
            let function = heap.list_get_ref(arguments, 0);
            // The slot after the arguments is the type tuple's (none).
            let args: Vec<i64> = (1..count).map(|i| heap.list_get_ref(arguments, i)).chain(std::iter::once(0)).collect();
            let name_list: Vec<Option<String>> = (0..named)
                .map(|i| {
                    let v = heap.list_get_ref(names, i);
                    heap.text(v).map(|t| t.para_string())
                })
                .collect();
            (function, args, name_list)
        });
        let mut desc = vec![(count - 1 - named) as i64, named as i64];
        for name in name_list {
            let Some(name) = name else {
                dartforge_nsm_chamada();
                return 0;
            };
            // The same 64-bit FNV-1a as the descriptor emitted in `lower/closures.rs`.
            let hash = name.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
                (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
            });
            desc.push(hash as i64);
        }
        with_roots(&args, || {
            let code = dartforge_closure_entry(function);
            if code == 0 { return 0; }
            // SAFETY: `dartforge_closure_entry` returns the address of a uniform
            // entry generated with the signature (closure, arguments, descriptor).
            // Through the port (`exceptions_tables.rs`): both pointers pass as
            // words, the same argument passing.
            dart_r3(code as usize, function, args.as_ptr() as i64, desc.as_ptr() as i64)
        })
    })
}

#[cfg(test)]
mod function_apply_tests {
    use super::*;

    extern "C" fn entry(_closure: i64, args: *const i64, desc: *const i64) -> i64 {
        // The Dart patch supplies 1 positional and two named ones in the order c,b.
        dartforge_gc_collect();
        let matches = unsafe {
            *desc == 1 && *desc.add(1) == 2
                && *desc.add(2) == hash("c") && *desc.add(3) == hash("b")
                && *args.add(1) == 0 && *args.add(2) == 0
        };
        let value_alive = HEAP.with(|heap| heap.borrow().text(unsafe { *args }).is_some_and(|t| t.para_string() == "valor"));
        i64::from(matches && value_alive)
    }

    fn hash(name: &str) -> i64 {
        name.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
        }) as i64
    }

    #[test]
    fn function_apply_forwards_named_descriptor_to_uniform_entry() {
        use crate::heap::Value;
        use crate::lists::Element;
        let function = dartforge_tearoff(entry as usize as i64);
        with_roots(&[function], || {
            let c = HEAP.with(|heap| heap.borrow_mut().allocate_str("c"));
            with_roots(&[c], || {
                let b = HEAP.with(|heap| heap.borrow_mut().allocate_str("b"));
                with_roots(&[b], || {
                    let value = HEAP.with(|heap| heap.borrow_mut().allocate_str("valor"));
                    with_roots(&[value], || {
                        let arguments = HEAP.with(|heap| {
                            let mut heap = heap.borrow_mut();
                            let l = heap.new_list(crate::layout::cid::LIST, 4, Element::General);
                            heap.list_set(l, 0, Value::Ref(function));
                            heap.list_set(l, 1, Value::Ref(value));
                            l
                        });
                        with_roots(&[arguments], || {
                            let names = HEAP.with(|heap| {
                                let mut heap = heap.borrow_mut();
                                let l = heap.new_list(crate::layout::cid::LIST, 2, Element::General);
                                heap.list_set(l, 0, Value::Ref(c));
                                heap.list_set(l, 1, Value::Ref(b));
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
