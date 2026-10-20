//! Emission of boxes, cells, contexts, closures and records (P2,
//! docs/NATIVO-ESPACO-UNIFICADO.md §2.5, §3.5 and §4.4): scalar `Box`/`Unbox`,
//! `AllocCell`/`CellGet`/`CellSet`, `AllocEnv`/`EnvGet`, `AllocClosure`,
//! `AllocClosureTyped`, `TearOff`, `AllocRecord` and the
//! `@df.box_*`/`@df.unbox_*`/`@df.identical` helpers of the prelude.
//!
//! Everything is a block of the object space, allocated inline by `@df.allocate` (P0) and
//! read/written by offset from the handle (`h = block + 2`, so the
//! word `b + k` is at `h + k − 2`):
//!
//! * `_Mint`/`_Double` (`RAW`, w = 1): the value at `h+14`;
//! * `bool`: the statics of the active runtime, read from the `Context` (offsets
//!   360 and 368, `layout::context::VERDADEIRO`/`FALSO`; §4.10 item 9);
//! * `_Celula` (`INSTANCE`, 1 field): the value at `h+14`, the map bit (`h+6`)
//!   according to the written representation;
//! * `_Contexto` (`INSTANCE`, k fields): captures at `h+14+8i`, each one in the
//!   representation of the type of the variable (the same one `EnvGet` reads it with);
//! * `_Closure` (`INSTANCE`, 4 fields): code `h+14`, context `h+22` (`Ref`,
//!   the only map bit), typed body `h+30`, ABI `h+38`;
//! * positional `_Record` (`REFS`, w = 1 + k): k at `h+14`, fields at `h+22+8i`.
//!
//! A freshly allocated object is young: the writes of its creation have no barrier.

use super::LlvmEmitter;
use crate::hir::*;
use dartforge_runtime::layout::{self, cid, flags, state};
use std::fmt::Write;

#[cfg(test)]
/// The header (word 0) of a `_Mint`: `YOUNG | RAW << 8 | 1 << 16 | 3 << 32`.
const CAB_MINT: u64 = layout::header_word(state::YOUNG, flags::RAW, 1, cid::MINT);
/// That of a `_Double`.
#[cfg(test)]
const CAB_DOUBLE: u64 = layout::header_word(state::YOUNG, flags::RAW, 1, cid::DOUBLE);
/// That of a `_Celula` (1 field).
const CAB_CELL: u64 = layout::header_word(state::YOUNG, flags::INSTANCE, 1, cid::CELL);
/// That of a `_Closure` (4 fields).
const CAB_CLOSURE: u64 = layout::header_word(state::YOUNG, flags::INSTANCE, 4, cid::CLOSURE);

/// The P2 helpers (§3.5), printed in the prelude of each module by
/// `emit_runtime_decls`. The header constants are written out in
/// full (the text is `const`); the `helpers_with_layout_headers` test
/// checks that they are those of [`layout::header_word`].
///
/// * `@df.box_int`: a `Smi` in the 63-bit range; otherwise a new `_Mint`.
/// * `@df.unbox_int`: `Smi` → `ashr 1`; `_Mint` → the value; otherwise the
///   runtime, which throws `TypeError` (pending exception).
/// * `@df.box_double`/`@df.unbox_double`: likewise with `_Double`.
/// * `@df.num_to_double`: a `num` in a `Ref` as a `double` (`Smi` and `_Mint`
///   converted; the rest through `@df.unbox_double`): the right operand
///   of `d op= n` with `d` a `double` and `n` a `num`.
/// * `@df.box_bool`: a `select` between the statics; `@df.unbox_bool`
///   compares with them, and the rest goes to the runtime.
/// * `@df.identical`: `identical` (§2.10) — the same handle, or two `_Mint`/
///   `_Double` with the same bits.
pub const HELPERS: &str = "define internal i64 @df.box_int(i64 %v) alwaysinline {\n\
  %a = add i64 %v, 4611686018427387904\n\
  %ok = icmp ult i64 %a, -9223372036854775808\n\
  br i1 %ok, label %smi, label %heap\n\
smi:\n\
  %s = shl i64 %v, 1\n\
  %r = or i64 %s, 1\n\
  ret i64 %r\n\
heap:\n\
  %h = call i64 @df.allocate(i64 12884967937, i64 1)\n\
  %p = inttoptr i64 %h to ptr\n\
  %q = getelementptr inbounds i8, ptr %p, i64 14\n\
  store i64 %v, ptr %q, align 8\n\
  ret i64 %h\n\
}\n\
define internal i64 @df.unbox_int(i64 %r) alwaysinline {\n\
  %b = and i64 %r, 1\n\
  %e = icmp ne i64 %b, 0\n\
  br i1 %e, label %smi, label %obj\n\
smi:\n\
  %v = ashr i64 %r, 1\n\
  ret i64 %v\n\
obj:\n\
  %m = and i64 %r, -9223372036854775801\n\
  %o = icmp eq i64 %m, 2\n\
  br i1 %o, label %cls, label %lento\n\
cls:\n\
  %p = inttoptr i64 %r to ptr\n\
  %cp = getelementptr inbounds i8, ptr %p, i64 2\n\
  %c = load i32, ptr %cp, align 4, !invariant.load !{}\n\
  %mint = icmp eq i32 %c, 3\n\
  br i1 %mint, label %ler, label %lento\n\
ler:\n\
  %vp = getelementptr inbounds i8, ptr %p, i64 14\n\
  %x = load i64, ptr %vp, align 8\n\
  ret i64 %x\n\
lento:\n\
  %h = call i64 @dartforge_unbox_int(i64 %r)\n\
  ret i64 %h\n\
}\n\
define internal i64 @df.box_double(double %d) alwaysinline {\n\
  %h = call i64 @df.allocate(i64 17179935233, i64 1)\n\
  %p = inttoptr i64 %h to ptr\n\
  %q = getelementptr inbounds i8, ptr %p, i64 14\n\
  store double %d, ptr %q, align 8\n\
  ret i64 %h\n\
}\n\
define internal double @df.unbox_double(i64 %r) alwaysinline {\n\
  %m = and i64 %r, -9223372036854775801\n\
  %o = icmp eq i64 %m, 2\n\
  br i1 %o, label %cls, label %lento\n\
cls:\n\
  %p = inttoptr i64 %r to ptr\n\
  %cp = getelementptr inbounds i8, ptr %p, i64 2\n\
  %c = load i32, ptr %cp, align 4, !invariant.load !{}\n\
  %dbl = icmp eq i32 %c, 4\n\
  br i1 %dbl, label %ler, label %lento\n\
ler:\n\
  %vp = getelementptr inbounds i8, ptr %p, i64 14\n\
  %x = load double, ptr %vp, align 8\n\
  ret double %x\n\
lento:\n\
  %h = call double @dartforge_unbox_double(i64 %r)\n\
  ret double %h\n\
}\n\
define internal double @df.num_to_double(i64 %r) alwaysinline {\n\
  %b = and i64 %r, 1\n\
  %e = icmp ne i64 %b, 0\n\
  br i1 %e, label %smi, label %obj\n\
smi:\n\
  %v = ashr i64 %r, 1\n\
  %dv = sitofp i64 %v to double\n\
  ret double %dv\n\
obj:\n\
  %m = and i64 %r, -9223372036854775801\n\
  %o = icmp eq i64 %m, 2\n\
  br i1 %o, label %cls, label %dbl\n\
cls:\n\
  %p = inttoptr i64 %r to ptr\n\
  %cp = getelementptr inbounds i8, ptr %p, i64 2\n\
  %c = load i32, ptr %cp, align 4, !invariant.load !{}\n\
  %mint = icmp eq i32 %c, 3\n\
  br i1 %mint, label %ler, label %dbl\n\
ler:\n\
  %vp = getelementptr inbounds i8, ptr %p, i64 14\n\
  %x = load i64, ptr %vp, align 8\n\
  %dx = sitofp i64 %x to double\n\
  ret double %dx\n\
dbl:\n\
  %d = call double @df.unbox_double(i64 %r)\n\
  ret double %d\n\
}\n\
define internal i64 @df.box_bool(i1 %b) alwaysinline {\n\
  %ctx = call ptr @dartforge_context()\n\
  %o = select i1 %b, i64 360, i64 368\n\
  %p = getelementptr inbounds i8, ptr %ctx, i64 %o\n\
  %r = load i64, ptr %p, align 8, !invariant.load !{}\n\
  ret i64 %r\n\
}\n\
define internal i1 @df.unbox_bool(i64 %r) alwaysinline {\n\
  %ctx = call ptr @dartforge_context()\n\
  %vp = getelementptr inbounds i8, ptr %ctx, i64 360\n\
  %vh = load i64, ptr %vp, align 8, !invariant.load !{}\n\
  %v = icmp eq i64 %r, %vh\n\
  br i1 %v, label %sim, label %t\n\
sim:\n\
  ret i1 true\n\
t:\n\
  %fp = getelementptr inbounds i8, ptr %ctx, i64 368\n\
  %fh = load i64, ptr %fp, align 8, !invariant.load !{}\n\
  %f = icmp eq i64 %r, %fh\n\
  br i1 %f, label %nao, label %lento\n\
nao:\n\
  ret i1 false\n\
lento:\n\
  %u = call i8 @dartforge_unbox_bool(i64 %r)\n\
  %x = icmp ne i8 %u, 0\n\
  ret i1 %x\n\
}\n\
define internal i64 @df.bit_smi(i64 %x) alwaysinline {\n\
  %b = and i64 %x, 1\n\
  ret i64 %b\n\
}\n\
define internal i64 @df.smi_value(i64 %x) alwaysinline {\n\
  %v = ashr i64 %x, 1\n\
  ret i64 %v\n\
}\n\
define internal i1 @df.identical(i64 %a, i64 %b) alwaysinline {\n\
  %eq = icmp eq i64 %a, %b\n\
  br i1 %eq, label %sim, label %t1\n\
t1:\n\
  %ma = and i64 %a, -9223372036854775801\n\
  %oa = icmp eq i64 %ma, 2\n\
  %mb = and i64 %b, -9223372036854775801\n\
  %ob = icmp eq i64 %mb, 2\n\
  %ambos = and i1 %oa, %ob\n\
  br i1 %ambos, label %t2, label %nao\n\
t2:\n\
  %pa = inttoptr i64 %a to ptr\n\
  %pb = inttoptr i64 %b to ptr\n\
  %cpa = getelementptr inbounds i8, ptr %pa, i64 2\n\
  %cpb = getelementptr inbounds i8, ptr %pb, i64 2\n\
  %ca = load i32, ptr %cpa, align 4, !invariant.load !{}\n\
  %cb = load i32, ptr %cpb, align 4, !invariant.load !{}\n\
  %mesma = icmp eq i32 %ca, %cb\n\
  %c3 = sub i32 %ca, 3\n\
  %num = icmp ult i32 %c3, 2\n\
  %cmp = and i1 %mesma, %num\n\
  br i1 %cmp, label %t3, label %nao\n\
t3:\n\
  %va = getelementptr inbounds i8, ptr %pa, i64 14\n\
  %vb = getelementptr inbounds i8, ptr %pb, i64 14\n\
  %xa = load i64, ptr %va, align 8\n\
  %xb = load i64, ptr %vb, align 8\n\
  %r = icmp eq i64 %xa, %xb\n\
  ret i1 %r\n\
sim:\n\
  ret i1 true\n\
nao:\n\
  ret i1 false\n\
}\n";

/// The effects of each `@df.*` helper of this file called by
/// `CallRuntime`: `(name, allocates, throws)`. `externs::effects_of` consults them
/// before the extern table (a helper outside here is conservative). The
/// unboxings throw `TypeError` (which allocates) through the runtime.
pub(super) const HELPER_EFFECTS: &[(&str, bool, bool)] = &[
    ("df.box_int", true, false),
    ("df.box_double", true, false),
    ("df.box_bool", false, false),
    ("df.unbox_int", true, true),
    ("df.unbox_double", true, true),
    ("df.num_to_double", true, true),
    ("df.unbox_bool", true, true),
    ("df.identical", false, false),
    ("df.bit_smi", false, false),
    ("df.smi_value", false, false),
];

/// The per-module emission state of boxes and closures.
#[derive(Default)]
pub(super) struct BoxState {
    /// Counter of the local names of the instructions without a value (`CellSet`).
    next_item: u32,
}

impl BoxState {
    fn new_name(&mut self) -> u32 {
        let k = self.next_item;
        self.next_item += 1;
        k
    }
}

impl LlvmEmitter<'_> {
    /// `Box` of a scalar (`I64`, `F64`, `I1`/`I8`) into `%v<v>`.
    pub(super) fn emit_box(&mut self, v: u32, op: &Operand, from: Type) {
        match from {
            Type::F64 => {
                let only = self.coerce(op, Type::F64);
                writeln!(self.out, "  %v{v} = call i64 @df.box_double(double {only})").unwrap();
            }
            Type::I1 => {
                let only = self.coerce(op, Type::I1);
                writeln!(self.out, "  %v{v} = call i64 @df.box_bool(i1 {only})").unwrap();
            }
            Type::I8 => {
                let only = self.coerce(op, Type::I8);
                writeln!(self.out, "  %cb{v} = icmp ne i8 {only}, 0").unwrap();
                writeln!(self.out, "  %v{v} = call i64 @df.box_bool(i1 %cb{v})").unwrap();
            }
            _ => {
                let only = self.coerce(op, Type::I64);
                writeln!(self.out, "  %v{v} = call i64 @df.box_int(i64 {only})").unwrap();
            }
        }
    }

    /// `Unbox` to a scalar (`I64`, `F64`, `I1`); throws `TypeError` through the
    /// pending state.
    pub(super) fn emit_unbox(&mut self, v: u32, op: &Operand, to: Type) {
        let only = self.coerce(op, Type::Ref);
        match to {
            Type::F64 => {
                writeln!(
                    self.out,
                    "  %v{v} = call double @df.unbox_double(i64 {only})"
                )
                .unwrap();
            }
            Type::I1 => {
                writeln!(self.out, "  %v{v} = call i1 @df.unbox_bool(i64 {only})").unwrap();
            }
            _ => {
                writeln!(self.out, "  %v{v} = call i64 @df.unbox_int(i64 {only})").unwrap();
            }
        }
    }

    /// The inline allocation (`@df.allocate`) of a block of `w` words with
    /// header `hdr`, into `%v<v>`, and its pointer (the handle) into `%bp<v>`.
    /// Above `TLAB_N` words, `@df.allocate` goes to the runtime.
    fn allocate_block(&mut self, v: u32, hdr: u64, w: usize) {
        writeln!(
            self.out,
            "  %v{v} = call i64 @df.allocate(i64 {}, i64 {w})",
            hdr as i64
        )
        .unwrap();
        writeln!(self.out, "  %bp{v} = inttoptr i64 %v{v} to ptr").unwrap();
    }

    /// Writes `value` (IR text, `i64`) into the word at offset `off`
    /// (from the block) of object `%bp<v>`; `suffix` makes the name unique.
    fn store_word(&mut self, v: u32, suffix: &str, off: usize, value: &str) {
        let d = off as i64 - layout::HANDLE_OFFSET;
        writeln!(
            self.out,
            "  %bg{v}_{suffix} = getelementptr inbounds i8, ptr %bp{v}, i64 {d}"
        )
        .unwrap();
        writeln!(
            self.out,
            "  store i64 {value}, ptr %bg{v}_{suffix}, align 8"
        )
        .unwrap();
    }

    /// Writes the `map` (the first 32 reference bits) of object `%bp<v>`.
    fn store_map(&mut self, v: u32, map: u32) {
        if map == 0 {
            // Word 1 of the header comes zeroed from the allocation.
            return;
        }
        let d = layout::offsets::MAP as i64 - layout::HANDLE_OFFSET;
        writeln!(
            self.out,
            "  %bm{v} = getelementptr inbounds i8, ptr %bp{v}, i64 {d}"
        )
        .unwrap();
        writeln!(self.out, "  store i32 {}, ptr %bm{v}, align 4", map as i32).unwrap();
    }

    /// Are all the elements of the record `Ref`s (the lowering boxes them,
    /// `lower_positional_record`)? Otherwise, the slow path with the pairs.
    fn refs_only_record(&self, elements: &[(Operand, u8)]) -> bool {
        elements.iter().all(|(op, _)| self.type_of(op) == Type::Ref)
    }

    /// The `alloca` of the entry block of an `AllocRecord`: the
    /// `(bits, tag)` pairs of the slow path (an unboxed element), or the `Ref`s of a
    /// record too large for the TLAB.
    pub(super) fn record_buffer(&mut self, v: u32, elements: &[(Operand, u8)]) {
        if self.arc_owned_factories() {
            if !elements.is_empty() {
                writeln!(
                    self.out,
                    "  %rec_buf_{v} = alloca [{} x i64]",
                    elements.len()
                )
                .unwrap();
            }
        } else if !self.refs_only_record(elements) {
            writeln!(
                self.out,
                "  %rec_buf_{v} = alloca [{} x i64]",
                elements.len() * 2
            )
            .unwrap();
        } else if 1 + elements.len() > layout::TLAB_N {
            writeln!(
                self.out,
                "  %rec_buf_{v} = alloca [{} x i64]",
                elements.len()
            )
            .unwrap();
        }
    }

    /// The literal positional record (`_Record`, `REFS`).
    pub(super) fn emit_alloc_record(&mut self, v: u32, elements: &[(Operand, u8)]) {
        let n = elements.len();
        // Under prepared ARC, the Owned factory (the fields are already `Ref`s,
        // checked by the preparation), which counts the edges.
        if self.arc_owned_factories() {
            for (i, (elem, _)) in elements.iter().enumerate() {
                let s = self.coerce(elem, Type::Ref);
                writeln!(
                    self.out,
                    "  %rb{v}_{i} = getelementptr [{n} x i64], ptr %rec_buf_{v}, i64 0, i64 {i}"
                )
                .unwrap();
                writeln!(self.out, "  store i64 {s}, ptr %rb{v}_{i}").unwrap();
            }
            let buf = if n == 0 {
                "null".to_string()
            } else {
                format!("%rec_buf_{v}")
            };
            writeln!(
                self.out,
                "  %v{v} = call i64 @dartforge_arc_record_from_refs_owned_v1(ptr {buf}, i64 {n})"
            )
            .unwrap();
            return;
        }
        if !self.refs_only_record(elements) {
            // Slow path (an unboxed element, which the lowering has not
            // boxed yet): the runtime boxes with the right roots.
            let total = n * 2;
            for (i, (elem, _)) in elements.iter().enumerate() {
                let tag = self.tag_of(elem);
                let sop = self.coerce(elem, Type::I64);
                writeln!(
                    self.out,
                    "  %rb{v}_{i} = getelementptr [{total} x i64], ptr %rec_buf_{v}, i64 0, i64 {}",
                    i * 2
                )
                .unwrap();
                writeln!(self.out, "  store i64 {sop}, ptr %rb{v}_{i}").unwrap();
                writeln!(
                    self.out,
                    "  %rt{v}_{i} = getelementptr [{total} x i64], ptr %rec_buf_{v}, i64 0, i64 {}",
                    i * 2 + 1
                )
                .unwrap();
                writeln!(self.out, "  store i64 {tag}, ptr %rt{v}_{i}").unwrap();
            }
            writeln!(
                self.out,
                "  %v{v} = call i64 @dartforge_record_new(ptr %rec_buf_{v}, i64 {n})"
            )
            .unwrap();
            return;
        }
        let w = 1 + n;
        if w > layout::TLAB_N {
            for (i, (elem, _)) in elements.iter().enumerate() {
                let s = self.coerce(elem, Type::Ref);
                writeln!(
                    self.out,
                    "  %rb{v}_{i} = getelementptr [{n} x i64], ptr %rec_buf_{v}, i64 0, i64 {i}"
                )
                .unwrap();
                writeln!(self.out, "  store i64 {s}, ptr %rb{v}_{i}").unwrap();
            }
            writeln!(
                self.out,
                "  %v{v} = call i64 @dartforge_record_from_refs(ptr %rec_buf_{v}, i64 {n})"
            )
            .unwrap();
            return;
        }
        let hdr = layout::header_word(state::YOUNG, flags::REFS, w, cid::RECORD);
        let values: Vec<String> = elements
            .iter()
            .map(|(e, _)| self.coerce(e, Type::Ref))
            .collect();
        self.allocate_block(v, hdr, w);
        self.store_word(v, "n", layout::offsets::RECORD_SHAPE, &n.to_string());
        for (i, s) in values.iter().enumerate() {
            self.store_word(v, &i.to_string(), layout::offsets::RECORD_FIELDS + 8 * i, s);
        }
        if n > 0 {
            self.count_initial_in_arc(v);
        }
    }

    /// The cell of a mutable capture: the value in the representation of the local.
    pub(super) fn emit_alloc_cell(&mut self, v: u32, value: &Operand) {
        let is_ref = self.type_of(value) == Type::Ref;
        let s = self.coerce(value, Type::I64);
        self.allocate_block(v, CAB_CELL, layout::instance_words(1));
        self.store_word(v, "c", layout::offsets::BODY, &s);
        self.store_map(v, u32::from(is_ref));
        if is_ref {
            self.count_initial_in_arc(v);
        }
    }

    /// Under ARC, the references written inline into the freshly allocated object
    /// `%v<v>` start counting (`dartforge_arc_initial`): the generated code does not
    /// go through the runtime for these writes.
    pub(super) fn count_initial_in_arc(&mut self, v: u32) {
        if self.module.arc_memory {
            writeln!(self.out, "  call void @dartforge_arc_initial(i64 %v{v})").unwrap();
        }
    }

    /// The read of a cell in representation `ty` (the one it was written with).
    pub(super) fn emit_cell_get(&mut self, v: u32, cell: &Operand, ty: Type) {
        let c = self.coerce(cell, Type::Ref);
        let d = layout::offsets::BODY as i64 - layout::HANDLE_OFFSET;
        writeln!(self.out, "  %cgp{v} = inttoptr i64 {c} to ptr").unwrap();
        writeln!(
            self.out,
            "  %cgg{v} = getelementptr inbounds i8, ptr %cgp{v}, i64 {d}"
        )
        .unwrap();
        writeln!(self.out, "  %u{v} = load i64, ptr %cgg{v}, align 8").unwrap();
        self.bits_to_repr(v, &format!("%u{v}"), ty);
    }

    /// The write into a cell: the value, the map bit by representation and,
    /// for a `Ref`, the barrier (`@df.barrier`).
    pub(super) fn emit_cell_set(&mut self, cell: &Operand, value: &Operand) {
        let k = self.boxes.new_name();
        let is_ref = self.type_of(value) == Type::Ref;
        let c = self.coerce(cell, Type::Ref);
        let s = self.coerce(value, Type::I64);
        // Under ARC the write goes to the runtime, which counts the replacement (field 0 of the
        // cell, a one-field `INSTANCE`).
        if self.module.arc_memory {
            writeln!(
                self.out,
                "  call void @dartforge_object_set(i64 {c}, i64 0, i64 {s}, i8 {})",
                u8::from(is_ref)
            )
            .unwrap();
            return;
        }
        let d = layout::offsets::BODY as i64 - layout::HANDLE_OFFSET;
        let dm = layout::offsets::MAP as i64 - layout::HANDLE_OFFSET;
        writeln!(self.out, "  %csp{k} = inttoptr i64 {c} to ptr").unwrap();
        writeln!(
            self.out,
            "  %csg{k} = getelementptr inbounds i8, ptr %csp{k}, i64 {d}"
        )
        .unwrap();
        writeln!(self.out, "  store i64 {s}, ptr %csg{k}, align 8").unwrap();
        writeln!(
            self.out,
            "  %csm{k} = getelementptr inbounds i8, ptr %csp{k}, i64 {dm}"
        )
        .unwrap();
        writeln!(
            self.out,
            "  store i32 {}, ptr %csm{k}, align 4",
            u8::from(is_ref)
        )
        .unwrap();
        let constant = matches!(
            value,
            Operand::Constant(
                Constant::Null | Constant::Int(_) | Constant::Bool(_) | Constant::Double(_)
            )
        );
        if is_ref && !constant {
            writeln!(self.out, "  call void @df.barrier(i64 {c}, i64 {s})").unwrap();
        }
    }

    /// The `alloca` of the entry block of an `AllocEnv`: none (the captures
    /// go directly into the context block).
    pub(super) fn env_buffer(&mut self, v: u32, values: &[Operand]) {
        let _ = (v, values);
    }

    /// The context (`_Contexto`) of the captures of a closure: each capture in the
    /// representation of its operand, with the map bit set for a `Ref`. Without
    /// captures, null (whoever captures nothing never reads the context).
    pub(super) fn emit_alloc_env(&mut self, v: u32, values: &[Operand]) {
        let n = values.len();
        if n == 0 {
            writeln!(self.out, "  %v{v} = add i64 0, 0").unwrap();
            return;
        }
        let refs: Vec<bool> = values
            .iter()
            .map(|val| self.type_of(val) == Type::Ref)
            .collect();
        let bits: Vec<String> = values
            .iter()
            .map(|val| self.coerce(val, Type::I64))
            .collect();
        let w = layout::instance_words(n);
        if w <= layout::TLAB_N {
            let hdr = layout::header_word(state::YOUNG, flags::INSTANCE, n, cid::CONTEXT);
            self.allocate_block(v, hdr, w);
        } else {
            // A large context: the runtime (the `n` of an `INSTANCE` is the number
            // of fields, not the words of the body).
            writeln!(
                self.out,
                "  %v{v} = call i64 @dartforge_object_new(i64 {}, i64 {n})",
                cid::CONTEXT
            )
            .unwrap();
            writeln!(self.out, "  %bp{v} = inttoptr i64 %v{v} to ptr").unwrap();
        }
        for (i, s) in bits.iter().enumerate() {
            self.store_word(v, &i.to_string(), layout::offsets::BODY + 8 * i, s);
        }
        let map = refs
            .iter()
            .take(32)
            .enumerate()
            .fold(0u32, |m, (i, &r)| m | (u32::from(r) << i));
        self.store_map(v, map);
        let has_ref = refs.iter().any(|&r| r);
        // The extension of the map (fields 32..), in the words after the fields.
        for j in 0..layout::map_words(n) {
            let word = refs
                .iter()
                .enumerate()
                .skip(32 + 64 * j)
                .take(64)
                .fold(0u64, |m, (i, &r)| m | (u64::from(r) << ((i - 32) % 64)));
            if word != 0 {
                let off = layout::offsets::BODY + 8 * (layout::capacity(n) + j);
                self.store_word(v, &format!("m{j}"), off, &(word as i64).to_string());
            }
        }
        if has_ref {
            self.count_initial_in_arc(v);
        }
    }

    /// Capture `index` of the context, in representation `ty` — the same one it
    /// was written with (the lowering guarantees it; §4.4 and risk 12).
    pub(super) fn emit_env_get(&mut self, v: u32, env: &Operand, index: usize, ty: Type) {
        let e = self.coerce(env, Type::Ref);
        let d = (layout::offsets::BODY + 8 * index) as i64 - layout::HANDLE_OFFSET;
        writeln!(self.out, "  %ep{v} = inttoptr i64 {e} to ptr").unwrap();
        writeln!(
            self.out,
            "  %eg{v} = getelementptr inbounds i8, ptr %ep{v}, i64 {d}"
        )
        .unwrap();
        writeln!(self.out, "  %u{v} = load i64, ptr %eg{v}, align 8").unwrap();
        self.bits_to_repr(v, &format!("%u{v}"), ty);
    }

    /// An inline `_Closure`: code, context (`Ref`), typed body and ABI.
    fn emit_inline_closure(&mut self, v: u32, code: &str, env: &Operand, typed: &str, abi: i64) {
        let e = self.coerce(env, Type::Ref);
        self.allocate_block(v, CAB_CLOSURE, layout::instance_words(4));
        self.store_word(v, "k", layout::offsets::CLOSURE_CODE, code);
        self.store_word(v, "e", layout::offsets::CLOSURE_CONTEXT, &e);
        self.store_word(v, "t", layout::offsets::CLOSURE_TYPED, typed);
        self.store_word(v, "a", layout::offsets::CLOSURE_ABI, &abi.to_string());
        // Only the context is a reference.
        self.store_map(v, 0b10);
        self.count_initial_in_arc(v);
    }

    /// The closure with code `code_symbol` (the uniform entry) over `env`.
    pub(super) fn emit_alloc_closure(&mut self, v: u32, code_symbol: &str, env: &Operand) {
        // The code of a closure is the address of the uniform entry: it holds across
        // modules (a closure created in the source SDK is called in the program) and
        // does not depend on the order of anything.
        self.annotate_external(code_symbol, Type::Ref, &[Type::Ref, Type::Ptr, Type::Ptr]);
        let code = format!("ptrtoint (ptr @{code_symbol} to i64)");
        self.emit_inline_closure(v, &code, env, "0", 0);
    }

    /// The closure with a typed body (`AllocClosureTyped`). With `direct`, `env`
    /// is the captured value itself (or the `this`) in place of the context: the same
    /// `Ref` field.
    pub(super) fn emit_alloc_typed_closure(
        &mut self,
        v: u32,
        code_symbol: &str,
        env: &Operand,
        typed: &str,
        abi: i64,
        direct: bool,
    ) {
        let _ = direct;
        self.annotate_external(code_symbol, Type::Ref, &[Type::Ref, Type::Ptr, Type::Ptr]);
        let code = format!("ptrtoint (ptr @{code_symbol} to i64)");
        let body = format!("ptrtoint (ptr @{typed} to i64)");
        self.emit_inline_closure(v, &code, env, &body, abi);
    }

    /// The canonical tear-off of the uniform entry function `code_symbol` (the
    /// runtime keeps the single handle, `Heap::tearoff`).
    pub(super) fn emit_tearoff(&mut self, v: u32, code_symbol: &str) {
        self.annotate_external(code_symbol, Type::Ref, &[Type::Ref, Type::Ptr, Type::Ptr]);
        writeln!(
            self.out,
            "  %v{v} = call i64 @dartforge_tearoff(i64 ptrtoint (ptr @{code_symbol} to i64))"
        )
        .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers_with_layout_headers() {
        assert!(HELPERS.contains(&format!("@df.allocate(i64 {}, i64 1)", CAB_MINT as i64)));
        assert!(HELPERS.contains(&format!("@df.allocate(i64 {}, i64 1)", CAB_DOUBLE as i64)));
        // The mask of `e_objeto` (7 | MIN) and the offsets of the value and of the class.
        assert!(HELPERS.contains(&format!("and i64 %r, {}", 7 | i64::MIN)));
        assert_eq!(layout::offsets::VALUE as i64 - layout::HANDLE_OFFSET, 14);
        assert_eq!(layout::offsets::CLASS as i64 - layout::HANDLE_OFFSET, 2);
        assert_eq!((cid::MINT, cid::DOUBLE), (3, 4));
        for (name, _, _) in HELPER_EFFECTS {
            assert!(
                HELPERS.contains(&format!("@{name}(")),
                "{name} without a definition"
            );
        }
        assert_eq!(CAB_CELL >> 32, cid::CELL as u64);
        assert_eq!(CAB_CLOSURE >> 16 & 0xFFFF, 4);
    }
}
