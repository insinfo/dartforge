//! Emission of core lists (P3, docs/NATIVO-ESPACO-UNIFICADO.md §3.5 and
//! §4.5): the list literal (`AllocList`) and the `@df.list_*` helpers of the
//! prelude.
//!
//! The literal goes to the runtime as words (`dartforge_list_new`), in the shape
//! the elements ask for: all `Ref` → general; all unboxed `int`, all `double` or all
//! `bool` → the compact shape of that scalar (the reified `E`, written
//! right after by `rti_definir`, confirms or undoes it: `Heap::lista_ajustar_forma`);
//! mixed → general, with a tag vector that tells the runtime which words
//! to box. The generated code does not box: a box made before the list
//! would have no root at the next allocation.
//!
//! The helpers read the block through the offsets of `layout::off` (checked
//! below, at compile time, against the numbers in the IR text). None
//! allocates or throws ([`HELPER_EFFECTS`]).

use super::LlvmEmitter;
use crate::hir::*;
use dartforge_runtime::layout::{self, cid, desl, flags};
use std::fmt::Write;

/// The `@df.list_*` helpers (§3.5), printed in the prelude of each module by
/// `emit_runtime_decls`. `%l` is a core list (`_List`, `_ImmutableList` or
/// `_GrowableList`); `%a`, a storage (`_List`/`_ImmutableList`), what
/// `@df.list_storage` returns.
///
/// * `@df.list_len(l)`: the length, `[l+14]` in all three classes;
/// * `@df.list_storage(l)`: cid 10 → `[l+22]`, otherwise `l`;
/// * `@df.list_element(a, i)`: the address `a + 22 + 8i`;
/// * `@df.list_shape(a)`: `[a−1] & 0x66` (`FORMA` and `ELEMENTO`: 0x04 general,
///   0x22 `int`, 0x42 `double`, 0x62 `bool`);
/// * `@df.list_capacity(l)`: the length of the storage;
/// * `@df.list_read(a, i)`: word `i` (a `Ref` in the general shape);
/// * `@df.list_write_ref(a, i, v)`: writes the `Ref` and applies the element
///   barrier (`@df.element_barrier`, with cards);
/// * `@df.list_write_len(l, n)`: the length of a `_GrowableList`, without a
///   barrier (it is not a reference; the VM's `_setLength`);
/// * `@df.list_write_data(l, d)`: replaces the storage of a `_GrowableList`
///   with the barrier when `d` has the shape of the current one (returns 1); otherwise it does not write
///   and returns 0 (the native gives the shape to the new one).
pub const HELPERS: &str = "define internal i64 @df.list_len(i64 %l) alwaysinline {\n\
  %p = inttoptr i64 %l to ptr\n\
  %q = getelementptr inbounds i8, ptr %p, i64 14\n\
  %n = load i64, ptr %q, align 8\n\
  ret i64 %n\n\
}\n\
define internal i64 @df.list_storage(i64 %l) alwaysinline {\n\
entrada:\n\
  %p = inttoptr i64 %l to ptr\n\
  %cp = getelementptr inbounds i8, ptr %p, i64 2\n\
  %c = load i32, ptr %cp, align 4, !invariant.load !{}\n\
  %e = icmp eq i32 %c, 10\n\
  br i1 %e, label %exp, label %fim\n\
exp:\n\
  %dp = getelementptr inbounds i8, ptr %p, i64 22\n\
  %d = load i64, ptr %dp, align 8\n\
  br label %fim\n\
fim:\n\
  %r = phi i64 [ %d, %exp ], [ %l, %entrada ]\n\
  ret i64 %r\n\
}\n\
define internal ptr @df.list_element(i64 %a, i64 %i) alwaysinline {\n\
  %p = inttoptr i64 %a to ptr\n\
  %b = getelementptr inbounds i8, ptr %p, i64 22\n\
  %e = getelementptr inbounds i64, ptr %b, i64 %i\n\
  ret ptr %e\n\
}\n\
define internal i8 @df.list_shape(i64 %a) alwaysinline {\n\
  %p = inttoptr i64 %a to ptr\n\
  %fp = getelementptr inbounds i8, ptr %p, i64 -1\n\
  %f = load i8, ptr %fp, align 1\n\
  %r = and i8 %f, 102\n\
  ret i8 %r\n\
}\n\
define internal i64 @df.list_capacity(i64 %l) alwaysinline {\n\
  %a = call i64 @df.list_storage(i64 %l)\n\
  %n = call i64 @df.list_len(i64 %a)\n\
  ret i64 %n\n\
}\n\
define internal i64 @df.list_read(i64 %a, i64 %i) alwaysinline {\n\
  %e = call ptr @df.list_element(i64 %a, i64 %i)\n\
  %v = load i64, ptr %e, align 8\n\
  ret i64 %v\n\
}\n\
define internal void @df.list_write_ref(i64 %a, i64 %i, i64 %v) alwaysinline {\n\
  %e = call ptr @df.list_element(i64 %a, i64 %i)\n\
  store i64 %v, ptr %e, align 8\n\
  call void @df.element_barrier(i64 %a, i64 %i, i64 %v)\n\
  ret void\n\
}\n\
define internal void @df.list_write_len(i64 %l, i64 %n) alwaysinline {\n\
  %p = inttoptr i64 %l to ptr\n\
  %q = getelementptr inbounds i8, ptr %p, i64 14\n\
  store i64 %n, ptr %q, align 8\n\
  ret void\n\
}\n\
define internal i64 @df.list_write_data(i64 %l, i64 %d) alwaysinline {\n\
entrada:\n\
  %a = call i64 @df.list_storage(i64 %l)\n\
  %fa = call i8 @df.list_shape(i64 %a)\n\
  %fd = call i8 @df.list_shape(i64 %d)\n\
  %igual = icmp eq i8 %fa, %fd\n\
  br i1 %igual, label %grava, label %fim\n\
grava:\n\
  %p = inttoptr i64 %l to ptr\n\
  %q = getelementptr inbounds i8, ptr %p, i64 22\n\
  store i64 %d, ptr %q, align 8\n\
  call void @df.barrier(i64 %l, i64 %d)\n\
  br label %fim\n\
fim:\n\
  %r = phi i64 [ 1, %grava ], [ 0, %entrada ]\n\
  ret i64 %r\n\
}\n";

// The numbers in the text of `HELPERS` are those of the layout contract (relative to the
// handle, block + 2).
const _: () = {
    let h = layout::DESLOCAMENTO_DO_HANDLE as usize;
    assert!(desl::COMPRIMENTO - h == 14);
    assert!(desl::EXPANSIVEL_DADOS - h == 22);
    assert!(desl::ELEMENTOS - h == 22);
    assert!(desl::CLASSE - h == 2);
    assert!(desl::FLAGS as i64 - h as i64 == -1);
    assert!(cid::GROWABLE_LIST == 10);
    assert!(flags::FORMA | flags::ELEMENTO == 102);
};

/// The helpers of the module: under ARC (`Module::arc_memory`), the element write
/// and the storage replacement of the `_GrowableList` go through the
/// runtime, which counts the replacement and applies the barrier: the element through the word
/// of the `REFS` body (`dartforge_arc_write_ref`), the storage through field
/// 1 of the `INSTANCIA` (`dartforge_object_set`).
pub(super) fn helpers(arc: bool) -> std::borrow::Cow<'static, str> {
    if !arc {
        return std::borrow::Cow::Borrowed(HELPERS);
    }
    let element = "%e = call ptr @df.list_element(i64 %a, i64 %i)\nstore i64 %v, ptr %e, align 8\ncall void @df.element_barrier(i64 %a, i64 %i, i64 %v)\n";
    let arc_element = "%p = add i64 %i, 1\ncall void @dartforge_arc_write_ref(i64 %a, i64 %p, i64 %v)\n";
    let data = "%p = inttoptr i64 %l to ptr\n%q = getelementptr inbounds i8, ptr %p, i64 22\nstore i64 %d, ptr %q, align 8\ncall void @df.barrier(i64 %l, i64 %d)\n";
    let arc_data = "call void @dartforge_object_set(i64 %l, i64 1, i64 %d, i8 1)\n";
    assert!(HELPERS.matches(element).count() == 1 && HELPERS.matches(data).count() == 1);
    std::borrow::Cow::Owned(HELPERS.replace(element, arc_element).replace(data, arc_data))
}

/// The effects of each `@df.*` helper of this file called by
/// `CallRuntime`: `(name, allocates, throws)`. `externs::effects_of` consults them
/// before the extern table (a helper outside here is conservative).
pub(super) const HELPER_EFFECTS: &[(&str, bool, bool)] = &[
    ("df.list_len", false, false),
    ("df.list_storage", false, false),
    ("df.list_shape", false, false),
    ("df.list_capacity", false, false),
    ("df.list_read", false, false),
    ("df.list_write_ref", false, false),
    ("df.list_write_len", false, false),
    ("df.list_write_data", false, false),
];

/// The per-module emission state of lists.
#[derive(Default)]
pub(super) struct ListState {}

/// The ABI tag of a word of the mixed literal (that of `LlvmEmitter::tag_de`): 1
/// `int`, 2 `bool`, 3 `Ref`, 4 bits of a `double`.
const TAG_REF: u8 = 3;

impl LlvmEmitter<'_> {
    /// The `alloca`s of the entry block of an `AllocList`: the words and the tags
    /// (used only in the mixed literal; LLVM removes the leftover).
    pub(super) fn list_buffer(&mut self, v: u32, elements: &[(Operand, u8)]) {
        let n = elements.len().max(1);
        writeln!(self.out, "  %list_buf_{v} = alloca [{n} x i64], align 8").unwrap();
        writeln!(self.out, "  %list_tags_{v} = alloca [{n} x i8], align 1").unwrap();
    }

    /// The literal list: a `_GrowableList` of capacity `n` with the words
    /// of the elements (`dartforge_list_new(words, n, cid, shape)`).
    pub(super) fn emit_alloc_list(&mut self, v: u32, elements: &[(Operand, u8)]) {
        let n = elements.len();
        let width = n.max(1);
        let tags: Vec<u8> = elements.iter().map(|(op, _)| self.tag_de(op)).collect();
        // The shape by the code of `lists::Elemento` (0 general, 1 int, 2 double,
        // 3 bool); `None`: mixed.
        let shape: Option<i64> = match tags.first() {
            None => Some(0),
            Some(&t) if tags.iter().all(|&x| x == t) => Some(match t {
                1 => 1,
                4 => 2,
                2 => 3,
                _ => 0,
            }),
            _ => None,
        };
        for (k, (elem, _)) in elements.iter().enumerate() {
            // The word in the representation of the element: the `Ref`, the `i64`, the bits
            // of the `double` (`bitcast`) or 0/1 (`zext`).
            let word = if tags[k] == TAG_REF { self.coerce(elem, Type::Ref) } else { self.coerce(elem, Type::I64) };
            writeln!(self.out, "  %lp{v}_{k} = getelementptr [{width} x i64], ptr %list_buf_{v}, i64 0, i64 {k}").unwrap();
            writeln!(self.out, "  store i64 {word}, ptr %lp{v}_{k}, align 8").unwrap();
            if shape.is_none() {
                writeln!(self.out, "  %lt{v}_{k} = getelementptr [{width} x i8], ptr %list_tags_{v}, i64 0, i64 {k}").unwrap();
                writeln!(self.out, "  store i8 {}, ptr %lt{v}_{k}, align 1", tags[k]).unwrap();
            }
        }
        let shape = match shape {
            Some(f) => f.to_string(),
            None => {
                writeln!(self.out, "  %ltp{v} = ptrtoint ptr %list_tags_{v} to i64").unwrap();
                format!("%ltp{v}")
            }
        };
        writeln!(
            self.out,
            "  %v{v} = call i64 @{}(ptr %list_buf_{v}, i64 {n}, i64 {}, i64 {shape})",
            if self.arc_owned_factories() { "dartforge_arc_list_new_owned_v1" } else { "dartforge_list_new" },
            cid::GROWABLE_LIST
        )
        .unwrap();
    }
}
