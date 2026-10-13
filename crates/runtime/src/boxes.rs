//! Boxes, cells, contexts, closures and records of the unified space (P2,
//! docs/NATIVO-ESPACO-UNIFICADO.md §2.5, §2.10 and §3.3).
//!
//! * `_Mint` (cid 3) and `_Double` (cid 4): `BRUTO` blocks of one word, the value
//!   at `b+16`. The form is canonical: an `int` that fits in a `Smi` never becomes
//!   a `_Mint`.
//! * `bool` (cid 5): only the runtime's two statics (`dartforge_false`,
//!   `dartforge_true`, state `PERMANENTE`); the handle is the address + 2.
//! * `_Celula` (14): `INSTANCIA` with one field, the map bit set by the write.
//! * `_Contexto` (13): `INSTANCIA` with the captures, each bit set by the write.
//! * `_Closure` (11): `INSTANCIA` with four fields — code (raw), context (`Ref`:
//!   the `_Contexto`, or the captured value in the "direct environment"), typed
//!   body (raw) and ABI (raw); map `0b10`.
//! * Positional `_Record` (12): `REFS`, word 0 is the number of fields (raw) and
//!   the following ones are the fields (`Ref`).
//!
//! The allocating functions root the `Ref`s they receive themselves (a runtime
//! root frame around the allocation): the caller only has to root the result
//! before the next allocation.

use crate::heap::{Field, Heap, Value};
use crate::layout::{DESLOCAMENTO_DO_HANDLE, Ref, cid, is_object, flags, smi};

/// The view of a `_Closure`: the code, the context (or the captured value in the
/// "direct environment"), the typed body and the ABI.
#[derive(Clone, Copy, Debug)]
pub struct ClosureRef {
    pub code: i64,
    pub contexto: Field,
    pub typed_one: i64,
    pub abi: i64,
}

/// Fields of a `_Closure` (the layout order, §2.5).
const CAMPOS_DA_CLOSURE: usize = 4;

impl Heap {
    /// Runs `f` with owning copies of `refs` in a temporary runtime frame (those
    /// that are not handles are ignored). The occurrences are released after the
    /// construction, when the result already holds its edges.
    fn with_rooted_refs<R>(&mut self, refs: impl IntoIterator<Item = Ref>, f: impl FnOnce(&mut Self) -> R) -> R {
        let frame = self.push_owning_frame(0);
        for r in refs {
            self.root(frame, r);
        }
        let out = f(self);
        self.pop_frame(frame);
        out
    }

    /// The class of `r` (1 for null, 2 for `Smi`; N4 panic for a dead handle).
    fn cid_of(&self, r: Ref) -> i32 {
        self.class(r)
    }

    /// The `int` `v` in a `Ref` position: a `Smi` if it fits, otherwise a new `_Mint`.
    pub fn box_int(&mut self, v: i64) -> Ref {
        match smi::de(v) {
            Some(r) => r,
            None => {
                let h = self.allocate(cid::MINT, 1, flags::BRUTO);
                self.words_mut(h)[0] = v;
                h
            }
        }
    }
    /// A new `_Double` with `v`.
    pub fn box_double(&mut self, v: f64) -> Ref {
        let h = self.allocate(cid::DOUBLE, 1, flags::BRUTO);
        self.words_mut(h)[0] = v.to_bits() as i64;
        h
    }
    /// One of the two static `bool` boxes.
    pub fn box_bool(v: bool) -> Ref {
        let block = if v {
            std::ptr::addr_of!(crate::heap::dartforge_true)
        } else {
            std::ptr::addr_of!(crate::heap::dartforge_false)
        };
        block as i64 + DESLOCAMENTO_DO_HANDLE
    }
    /// The value in a `Ref` position (boxes what needs boxing).
    pub fn to_ref(&mut self, v: Value) -> Ref {
        match v {
            Value::Ref(r) => r,
            Value::Int(i) => self.box_int(i),
            Value::Double(d) => self.box_double(d),
            Value::Bool(b) => Heap::box_bool(b),
        }
    }
    /// The value of a `Ref`, unboxed (`Smi`, `_Mint`, `_Double`, `bool`); the rest
    /// as `Value::Ref`.
    pub fn value(&self, r: Ref) -> Value {
        if smi::is_smi(r) {
            return Value::Int(smi::value(r));
        }
        if !is_object(r) {
            return Value::Ref(r);
        }
        match self.cid_of(r) {
            cid::MINT => Value::Int(self.words(r)[0]),
            cid::DOUBLE => Value::Double(f64::from_bits(self.words(r)[0] as u64)),
            cid::BOOL => Value::Bool(self.words(r)[0] != 0),
            _ => Value::Ref(r),
        }
    }
    /// The `int` of a `Smi` or `_Mint`.
    pub fn int_of(&self, r: Ref) -> Option<i64> {
        if smi::is_smi(r) {
            return Some(smi::value(r));
        }
        (is_object(r) && self.cid_of(r) == cid::MINT).then(|| self.words(r)[0])
    }
    /// The `double` of a `_Double`.
    pub fn double_of(&self, r: Ref) -> Option<f64> {
        (is_object(r) && self.cid_of(r) == cid::DOUBLE).then(|| f64::from_bits(self.words(r)[0] as u64))
    }
    /// The `bool` of one of the static boxes.
    pub fn bool_of(&self, r: Ref) -> Option<bool> {
        if r == Heap::box_bool(true) {
            Some(true)
        } else if r == Heap::box_bool(false) {
            Some(false)
        } else if is_object(r) && self.cid_of(r) == cid::BOOL {
            // The static of another copy of the runtime (the SDK DLL and the
            // executable, §4.10 9a): the value is in the body.
            Some(self.words(r)[0] != 0)
        } else {
            None
        }
    }
    /// `identical(a, b)` (§2.10): the same handle, or `_Mint`/`_Double` with the same value.
    pub fn identical_one(&self, a: Ref, b: Ref) -> bool {
        if a == b {
            return true;
        }
        if !is_object(a) || !is_object(b) {
            // `Smi` and null are only identical by handle (canonical form).
            return false;
        }
        let ca = self.cid_of(a);
        if ca != self.cid_of(b) {
            // The two static `bool`s of different copies of the runtime.
            return ca == cid::BOOL && self.bool_of(a) == self.bool_of(b);
        }
        match ca {
            // The bits: `identical(NaN, NaN)` is true and `0.0`/`-0.0` is not.
            cid::MINT | cid::DOUBLE => self.words(a)[0] == self.words(b)[0],
            cid::BOOL => self.bool_of(a) == self.bool_of(b),
            _ => false,
        }
    }
    /// A new `_Celula` with `value`.
    pub fn new_cell(&mut self, value: Field) -> Ref {
        let root = value.1.then_some(value.0);
        self.with_rooted_refs(root, |heap| {
            let h = heap.allocate_instance(cid::CELULA, 1);
            heap.set_field(h, 0, value.0, value.1);
            h
        })
    }
    /// The value of the `_Celula` `h`.
    pub fn cell(&self, h: Ref) -> Field {
        let o = self.object(h).expect("cell expected");
        debug_assert_eq!(o.class_id, i64::from(cid::CELULA), "cell expected");
        o.field(0)
    }
    /// Writes into the `_Celula` `h` (with a barrier).
    pub fn write_cell(&mut self, h: Ref, value: Field) {
        self.set_field(h, 0, value.0, value.1);
    }
    /// A new `_Contexto` with the captures.
    pub fn new_context(&mut self, captures: &[Field]) -> Ref {
        let roots = captures.iter().filter(|c| c.1).map(|c| c.0).collect::<Vec<_>>();
        self.with_rooted_refs(roots, |heap| {
            let h = heap.allocate_instance(cid::CONTEXTO, captures.len());
            for (i, &(bits, is_ref_kind)) in captures.iter().enumerate() {
                heap.set_field(h, i, bits, is_ref_kind);
            }
            h
        })
    }
    /// Capture `i` of the `_Contexto` `h`.
    pub fn capture(&self, h: Ref, i: usize) -> Field {
        let o = self.object(h).expect("context expected");
        debug_assert_eq!(o.class_id, i64::from(cid::CONTEXTO), "context expected");
        o.field(i)
    }
    /// A new `_Closure`.
    pub fn new_closure(&mut self, code: i64, contexto: Field, typed_one: i64, abi: i64) -> Ref {
        let root = contexto.1.then_some(contexto.0);
        self.with_rooted_refs(root, |heap| {
            let h = heap.allocate_instance(cid::CLOSURE, CAMPOS_DA_CLOSURE);
            heap.set_field(h, 0, code, false);
            heap.set_field(h, 1, contexto.0, contexto.1);
            heap.set_field(h, 2, typed_one, false);
            heap.set_field(h, 3, abi, false);
            h
        })
    }
    /// The view of the `_Closure` `h`; `None` if it is not a closure.
    pub fn closure(&self, h: Ref) -> Option<ClosureRef> {
        if !is_object(h) || self.cid_of(h) != cid::CLOSURE {
            return None;
        }
        let o = self.object(h)?;
        Some(ClosureRef { code: o.field(0).0, contexto: o.field(1), typed_one: o.field(2).0, abi: o.field(3).0 })
    }
    /// The canonical tear-off of the top-level function `code` (`tearoffs`
    /// table): a closure without context and without typed body, the same one
    /// on every call (`identical(f, f)`), a permanent root.
    pub fn tearoff(&mut self, code: i64) -> Ref {
        assert!(code >= 0, "invalid code ID");
        if let Some(h) = self.registered_tearoff(code) {
            return h;
        }
        let h = self.new_closure(code, (0, false), 0, 0);
        self.register_tearoff(code, h);
        h
    }
    /// A positional record (`_Record`, `REFS`) with the fields.
    pub fn new_record(&mut self, fields: &[Ref]) -> Ref {
        self.with_rooted_refs(fields.iter().copied(), |heap| {
            let h = heap.allocate(cid::RECORD, 1 + fields.len(), flags::REFS);
            // Freshly allocated (young, unpublished): write without a barrier.
            let body = heap.words_mut(h);
            body[0] = fields.len() as i64;
            body[1..=fields.len()].copy_from_slice(fields);
            h
        })
    }
    /// [`Heap::new_record`] with the edges counted by the ARC (`write_refs`): the
    /// fields stay rooted during the allocation.
    pub fn new_counted_record(&mut self, fields: &[Ref]) -> Ref {
        self.with_rooted_refs(fields.iter().copied(), |heap| {
            let h = heap.allocate(cid::RECORD, 1 + fields.len(), flags::REFS);
            heap.words_mut(h)[0] = fields.len() as i64;
            if !fields.is_empty() {
                heap.write_refs(h, 1, fields);
            }
            h
        })
    }
    /// The fields of the positional record `h`; `None` if it is not a `_Record`.
    pub fn record(&self, h: Ref) -> Option<&[i64]> {
        if !is_object(h) || self.cid_of(h) != cid::RECORD {
            return None;
        }
        let body = self.words(h);
        let n = usize::try_from(body[0]).expect("record shape");
        Some(&body[1..=n])
    }
}

#[cfg(test)]
mod tests_boxes {
    use super::*;

    #[test]
    fn bool_boxes_are_the_static_ones() {
        let v = Heap::box_bool(true);
        let f = Heap::box_bool(false);
        assert_ne!(v, f);
        assert!(is_object(v) && is_object(f));
        assert_eq!(v, Heap::box_bool(true));
    }
}
