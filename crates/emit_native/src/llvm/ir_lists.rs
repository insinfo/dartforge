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
use dartforge_runtime::layout::{self, cid, flags, offsets};
use std::fmt::Write;

/// The `@df.list_*` helpers (§3.5), printed in the prelude of each module by
/// `emit_runtime_decls`. `%l` is a core list (`_List`, `_ImmutableList` or
/// `_GrowableList`); `%a`, a storage (`_List`/`_ImmutableList`), what
/// `@df.list_storage` returns.
///
/// * `@df.list_len(l)`: the length, `[l+14]` in all three classes;
/// * `@df.list_storage(l)`: cid 10 → `[l+22]`, otherwise `l`;
/// * `@df.list_element(a, i)`: the address `a + 22 + 8i`;
/// * `@df.list_shape(a)`: `[a−1] & 0x66` (`FORMAT` and `ELEMENT`: 0x04 general,
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
define internal i64 @df.list_grow(i64 %l, i64 %cab) noinline {\n\
entry:\n\
  %a = call i64 @df.list_storage(i64 %l)\n\
  %cap = call i64 @df.list_len(i64 %a)\n\
  %c2 = shl i64 %cap, 1\n\
  %nc = or i64 %c2, 3\n\
  %fits = icmp ule i64 %nc, 1024\n\
  br i1 %fits, label %grow, label %large\n\
large:\n\
  ret i64 0\n\
grow:\n\
  %w = add i64 %nc, 1\n\
  %ws = shl i64 %w, 16\n\
  %ch = or i64 %cab, %ws\n\
  %n = call i64 @df.allocate(i64 %ch, i64 %w)\n\
  %np = inttoptr i64 %n to ptr\n\
  %ncp = getelementptr inbounds i8, ptr %np, i64 14\n\
  store i64 %nc, ptr %ncp, align 8\n\
  %len = call i64 @df.list_len(i64 %l)\n\
  %a2 = call i64 @df.list_storage(i64 %l)\n\
  %ap = inttoptr i64 %a2 to ptr\n\
  %src = getelementptr inbounds i8, ptr %ap, i64 22\n\
  %dst = getelementptr inbounds i8, ptr %np, i64 22\n\
  br label %loop\n\
loop:\n\
  %i = phi i64 [ 0, %grow ], [ %i1, %copy ]\n\
  %end = icmp uge i64 %i, %len\n\
  br i1 %end, label %done, label %copy\n\
copy:\n\
  %sp = getelementptr inbounds i64, ptr %src, i64 %i\n\
  %x = load i64, ptr %sp, align 8\n\
  %dp = getelementptr inbounds i64, ptr %dst, i64 %i\n\
  store i64 %x, ptr %dp, align 8\n\
  %i1 = add i64 %i, 1\n\
  br label %loop\n\
done:\n\
  %lp = inttoptr i64 %l to ptr\n\
  %q = getelementptr inbounds i8, ptr %lp, i64 22\n\
  store i64 %n, ptr %q, align 8\n\
  call void @df.barrier(i64 %l, i64 %n)\n\
  ret i64 %n\n\
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
    let h = layout::HANDLE_OFFSET as usize;
    assert!(offsets::LENGTH - h == 14);
    assert!(offsets::GROWABLE_DATA - h == 22);
    assert!(offsets::ELEMENTS - h == 22);
    assert!(offsets::CLASS - h == 2);
    assert!(offsets::FLAGS as i64 - h as i64 == -1);
    assert!(cid::GROWABLE_LIST == 10);
    assert!(flags::FORMAT | flags::ELEMENT == 102);
};

/// The helpers of the module: under ARC (`Module::arc_memory`), the element write
/// and the storage replacement of the `_GrowableList` go through the
/// runtime, which counts the replacement and applies the barrier: the element through the word
/// of the `REFS` body (`dartforge_arc_write_ref`), the storage through field
/// 1 of the `INSTANCE` (`dartforge_object_set`). Under the nursery
/// (`Module::arc_nursery_writes`) only when the list or storage is not young: a
/// young one is written inline, without barrier or count ([`nursery_helpers`]).
pub(super) fn helpers(arc: bool, nursery: bool) -> std::borrow::Cow<'static, str> {
    if !arc {
        return std::borrow::Cow::Borrowed(HELPERS);
    }
    if nursery {
        return std::borrow::Cow::Owned(nursery_helpers());
    }
    let element = "%e = call ptr @df.list_element(i64 %a, i64 %i)\nstore i64 %v, ptr %e, align 8\ncall void @df.element_barrier(i64 %a, i64 %i, i64 %v)\n";
    let arc_element =
        "%p = add i64 %i, 1\ncall void @dartforge_arc_write_ref(i64 %a, i64 %p, i64 %v)\n";
    let data = "%p = inttoptr i64 %l to ptr\n%q = getelementptr inbounds i8, ptr %p, i64 22\nstore i64 %d, ptr %q, align 8\ncall void @df.barrier(i64 %l, i64 %d)\n";
    let arc_data = "call void @dartforge_object_set(i64 %l, i64 1, i64 %d, i8 1)\n";
    assert!(HELPERS.matches(element).count() == 1 && HELPERS.matches(data).count() == 1);
    std::borrow::Cow::Owned(
        HELPERS
            .replace(element, arc_element)
            .replace(data, arc_data),
    )
}

/// The tracing helpers with the stores of the ARC nursery: the element of a
/// storage, the storage of a `_GrowableList` (also the one `@df.list_grow`
/// installs) come out inline when the owner is young (state `YOUNG`, never
/// remembered); otherwise through the runtime, as in [`helpers`] under ARC.
fn nursery_helpers() -> String {
    let young = |o: &str, k: &str| {
        format!(
            "%{k}p = inttoptr i64 %{o} to ptr\n%{k}s = getelementptr inbounds i8, ptr %{k}p, i64 {}\n%{k}e = load i8, ptr %{k}s, align 1\n%{k}y = icmp eq i8 %{k}e, {}\nbr i1 %{k}y, label %{k}.young, label %{k}.old\n",
            layout::offsets::STATE as i64 - layout::HANDLE_OFFSET,
            layout::state::YOUNG
        )
    };
    let element = "%e = call ptr @df.list_element(i64 %a, i64 %i)\nstore i64 %v, ptr %e, align 8\ncall void @df.element_barrier(i64 %a, i64 %i, i64 %v)\n";
    let nursery_element = format!(
        "{}ny.old:\n%p = add i64 %i, 1\ncall void @dartforge_arc_write_ref(i64 %a, i64 %p, i64 %v)\nret void\nny.young:\n%e = call ptr @df.list_element(i64 %a, i64 %i)\nstore i64 %v, ptr %e, align 8\n",
        young("a", "ny")
    );
    let data = "%p = inttoptr i64 %l to ptr\n%q = getelementptr inbounds i8, ptr %p, i64 22\nstore i64 %d, ptr %q, align 8\ncall void @df.barrier(i64 %l, i64 %d)\nbr label %fim\nfim:\n%r = phi i64 [ 1, %grava ], [ 0, %entrada ]\n";
    let nursery_data = format!(
        "{}ny.old:\ncall void @dartforge_object_set(i64 %l, i64 1, i64 %d, i8 1)\nbr label %fim\nny.young:\n%q = getelementptr inbounds i8, ptr %nyp, i64 22\nstore i64 %d, ptr %q, align 8\nbr label %fim\nfim:\n%r = phi i64 [ 1, %ny.old ], [ 1, %ny.young ], [ 0, %entrada ]\n",
        young("l", "ny")
    );
    let grown = "%lp = inttoptr i64 %l to ptr\n%q = getelementptr inbounds i8, ptr %lp, i64 22\nstore i64 %n, ptr %q, align 8\ncall void @df.barrier(i64 %l, i64 %n)\n";
    let nursery_grown = format!(
        "{}ny.old:\ncall void @dartforge_object_set(i64 %l, i64 1, i64 %n, i8 1)\nret i64 %n\nny.young:\n%q = getelementptr inbounds i8, ptr %nyp, i64 22\nstore i64 %n, ptr %q, align 8\n",
        young("l", "ny")
    );
    assert!(
        HELPERS.matches(element).count() == 1
            && HELPERS.matches(data).count() == 1
            && HELPERS.matches(grown).count() == 1
    );
    HELPERS
        .replace(element, &nursery_element)
        .replace(data, &nursery_data)
        .replace(grown, &nursery_grown)
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
    // Allocates the new storage (`df.allocate`, which may collect).
    ("df.list_grow", true, false),
];

/// The per-module emission state of lists.
#[derive(Default)]
pub(super) struct ListState {}

/// The ABI tag of a word of the mixed literal (that of `LlvmEmitter::tag_of`): 1
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
        let tags: Vec<u8> = elements.iter().map(|(op, _)| self.tag_of(op)).collect();
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
            let word = if tags[k] == TAG_REF {
                self.coerce(elem, Type::Ref)
            } else {
                self.coerce(elem, Type::I64)
            };
            writeln!(
                self.out,
                "  %lp{v}_{k} = getelementptr [{width} x i64], ptr %list_buf_{v}, i64 0, i64 {k}"
            )
            .unwrap();
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
            if self.arc_owned_factories() {
                "dartforge_arc_list_new_owned_v1"
            } else {
                "dartforge_list_new"
            },
            cid::GROWABLE_LIST
        )
        .unwrap();
    }
}
