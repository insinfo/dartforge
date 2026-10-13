//! Caixas, células, contextos, closures e records do espaço unificado (P2,
//! docs/NATIVO-ESPACO-UNIFICADO.md §2.5, §2.10 e §3.3).
//!
//! * `_Mint` (cid 3) e `_Double` (cid 4): blocos `BRUTO` de uma palavra, o valor
//!   em `b+16`. A forma é canônica: um `int` que cabe no `Smi` nunca vira `_Mint`.
//! * `bool` (cid 5): só as duas estáticas do runtime (`dartforge_falso`,
//!   `dartforge_verdadeiro`, estado `PERMANENTE`); o handle é o endereço + 2.
//! * `_Celula` (14): `INSTANCIA` de um campo, o bit do mapa pela gravação.
//! * `_Contexto` (13): `INSTANCIA` com as capturas, cada bit pela gravação.
//! * `_Closure` (11): `INSTANCIA` de quatro campos — código (bruto), contexto
//!   (`Ref`: o `_Contexto`, ou o valor capturado no "ambiente direto"), corpo
//!   tipado (bruto) e ABI (bruta); mapa `0b10`.
//! * `_Record` posicional (12): `REFS`, a palavra 0 é o número de campos (bruto)
//!   e as seguintes os campos (`Ref`).
//!
//! As funções que alocam enraízam elas mesmas os `Ref` que recebem (um quadro
//! de raízes do runtime em volta da alocação): quem chama só precisa enraizar o
//! resultado antes da próxima alocação.

use crate::heap::{Field, Heap, Value};
use crate::layout::{DESLOCAMENTO_DO_HANDLE, Ref, cid, is_object, flags, smi};

/// A vista de uma `_Closure`: o código, o contexto (ou o valor capturado no
/// "ambiente direto"), o corpo tipado e a ABI.
#[derive(Clone, Copy, Debug)]
pub struct ClosureRef {
    pub code: i64,
    pub contexto: Field,
    pub typed_one: i64,
    pub abi: i64,
}

/// Campos de uma `_Closure` (a ordem do layout, §2.5).
const CAMPOS_DA_CLOSURE: usize = 4;

impl Heap {
    /// Executa `f` com cópias proprietárias dos `refs` num quadro temporário
    /// do runtime (os que não são handle são ignorados). As ocorrências são
    /// soltadas após a construção, quando o resultado já guarda suas arestas.
    fn with_rooted_refs<R>(&mut self, refs: impl IntoIterator<Item = Ref>, f: impl FnOnce(&mut Self) -> R) -> R {
        let frame = self.push_owning_frame(0);
        for r in refs {
            self.root(frame, r);
        }
        let out = f(self);
        self.pop_frame(frame);
        out
    }

    /// A classe de `r` (1 para null, 2 para `Smi`; pânico N4 para handle morto).
    fn cid_of(&self, r: Ref) -> i32 {
        self.class(r)
    }

    /// O `int` `v` em posição `Ref`: `Smi` se cabe, senão um `_Mint` novo.
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
    /// Um `_Double` novo com `v`.
    pub fn box_double(&mut self, v: f64) -> Ref {
        let h = self.allocate(cid::DOUBLE, 1, flags::BRUTO);
        self.words_mut(h)[0] = v.to_bits() as i64;
        h
    }
    /// Uma das duas caixas estáticas de `bool`.
    pub fn box_bool(v: bool) -> Ref {
        let block = if v {
            std::ptr::addr_of!(crate::heap::dartforge_verdadeiro)
        } else {
            std::ptr::addr_of!(crate::heap::dartforge_falso)
        };
        block as i64 + DESLOCAMENTO_DO_HANDLE
    }
    /// O valor em posição `Ref` (encaixota o que precisa).
    pub fn to_ref(&mut self, v: Value) -> Ref {
        match v {
            Value::Ref(r) => r,
            Value::Int(i) => self.box_int(i),
            Value::Double(d) => self.box_double(d),
            Value::Bool(b) => Heap::box_bool(b),
        }
    }
    /// O valor de um `Ref`, desencaixotado (`Smi`, `_Mint`, `_Double`, `bool`); o
    /// resto como `Value::Ref`.
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
    /// O `int` de um `Smi` ou `_Mint`.
    pub fn int_of(&self, r: Ref) -> Option<i64> {
        if smi::is_smi(r) {
            return Some(smi::value(r));
        }
        (is_object(r) && self.cid_of(r) == cid::MINT).then(|| self.words(r)[0])
    }
    /// O `double` de um `_Double`.
    pub fn double_of(&self, r: Ref) -> Option<f64> {
        (is_object(r) && self.cid_of(r) == cid::DOUBLE).then(|| f64::from_bits(self.words(r)[0] as u64))
    }
    /// O `bool` de uma das caixas estáticas.
    pub fn bool_of(&self, r: Ref) -> Option<bool> {
        if r == Heap::box_bool(true) {
            Some(true)
        } else if r == Heap::box_bool(false) {
            Some(false)
        } else if is_object(r) && self.cid_of(r) == cid::BOOL {
            // A estática de outra cópia do runtime (a DLL do SDK e o
            // executável, §4.10 9a): o valor está no corpo.
            Some(self.words(r)[0] != 0)
        } else {
            None
        }
    }
    /// `identical(a, b)` (§2.10): o mesmo handle, ou `_Mint`/`_Double` de mesmo valor.
    pub fn identical_one(&self, a: Ref, b: Ref) -> bool {
        if a == b {
            return true;
        }
        if !is_object(a) || !is_object(b) {
            // `Smi` e null só são idênticos pelo handle (forma canônica).
            return false;
        }
        let ca = self.cid_of(a);
        if ca != self.cid_of(b) {
            // As duas estáticas de `bool` de cópias diferentes do runtime.
            return ca == cid::BOOL && self.bool_of(a) == self.bool_of(b);
        }
        match ca {
            // Os bits: `identical(NaN, NaN)` é verdadeiro e `0.0`/`-0.0` não.
            cid::MINT | cid::DOUBLE => self.words(a)[0] == self.words(b)[0],
            cid::BOOL => self.bool_of(a) == self.bool_of(b),
            _ => false,
        }
    }
    /// Uma `_Celula` nova com `value`.
    pub fn new_cell(&mut self, value: Field) -> Ref {
        let root = value.1.then_some(value.0);
        self.with_rooted_refs(root, |heap| {
            let h = heap.allocate_instance(cid::CELULA, 1);
            heap.set_field(h, 0, value.0, value.1);
            h
        })
    }
    /// O valor da `_Celula` `h`.
    pub fn cell(&self, h: Ref) -> Field {
        let o = self.object(h).expect("célula esperada");
        debug_assert_eq!(o.class_id, i64::from(cid::CELULA), "célula esperada");
        o.field(0)
    }
    /// Grava na `_Celula` `h` (com barreira).
    pub fn write_cell(&mut self, h: Ref, value: Field) {
        self.set_field(h, 0, value.0, value.1);
    }
    /// Um `_Contexto` novo com as capturas.
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
    /// A captura `i` do `_Contexto` `h`.
    pub fn capture(&self, h: Ref, i: usize) -> Field {
        let o = self.object(h).expect("contexto esperado");
        debug_assert_eq!(o.class_id, i64::from(cid::CONTEXTO), "contexto esperado");
        o.field(i)
    }
    /// Uma `_Closure` nova.
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
    /// A vista da `_Closure` `h`; `None` se não é closure.
    pub fn closure(&self, h: Ref) -> Option<ClosureRef> {
        if !is_object(h) || self.cid_of(h) != cid::CLOSURE {
            return None;
        }
        let o = self.object(h)?;
        Some(ClosureRef { code: o.field(0).0, contexto: o.field(1), typed_one: o.field(2).0, abi: o.field(3).0 })
    }
    /// O tear-off canônico da função de topo `code` (tabela `tearoffs`): uma
    /// closure sem contexto e sem corpo tipado, a mesma a cada chamada
    /// (`identical(f, f)`), raiz permanente.
    pub fn tearoff(&mut self, code: i64) -> Ref {
        assert!(code >= 0, "ID de código inválido");
        if let Some(h) = self.registered_tearoff(code) {
            return h;
        }
        let h = self.new_closure(code, (0, false), 0, 0);
        self.register_tearoff(code, h);
        h
    }
    /// Um record posicional (`_Record`, `REFS`) com os campos.
    pub fn new_record(&mut self, fields: &[Ref]) -> Ref {
        self.with_rooted_refs(fields.iter().copied(), |heap| {
            let h = heap.allocate(cid::RECORD, 1 + fields.len(), flags::REFS);
            // Recém-alocado (jovem, sem publicar): gravação sem barreira.
            let body = heap.words_mut(h);
            body[0] = fields.len() as i64;
            body[1..=fields.len()].copy_from_slice(fields);
            h
        })
    }
    /// [`Heap::new_record`] com as arestas contadas pelo ARC (`write_refs`):
    /// os campos ficam enraizados durante a alocação.
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
    /// Os campos do record posicional `h`; `None` se não é `_Record`.
    pub fn record(&self, h: Ref) -> Option<&[i64]> {
        if !is_object(h) || self.cid_of(h) != cid::RECORD {
            return None;
        }
        let body = self.words(h);
        let n = usize::try_from(body[0]).expect("forma de record");
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
