# Managed captures and lists

This Rust API is runtime infrastructure. **It does not yet implement lowering of Dart
closures or lists to LLVM**, indirect closure calls, function signatures or generic type
checks. The existing scalar C ABI is unchanged.

`TaggedValue` holds `bits: i64` and `is_ref: bool`. Only the reference tag makes the GC
follow the bits as a handle; zero is the null reference. Scalars whose bits equal a handle
do not keep objects alive. Handles identify live objects and may be reused after a
collection; they must not be kept outside the root protocol.

A reference is `0` (null), an **even** handle (`(index + 1) << 1`) or an **odd** `Smi`
— `(v << 1) | 1`, a 63-bit `int` without allocation (R10, docs/NATIVO-PLANO.md §6.2;
module `heap::smi`). The GC never follows a `Smi`, even when it is tagged as a reference;
the collections normalize it to the scalar.

| Structure | Contract |
| --- | --- |
| `Cell` | Mutable capture. Different environments may share the same cell and observe its changes. |
| `Environment` | Ordered, immutable captures. Mutable captured values are references to cells. |
| `Closure` | New object with its own identity, a symbolic code ID and a reference to an environment. Two creations with the same code remain distinct. Does not run code. |
| `List` | Growable sequence of tagged values. Read/replace by index and append; capacity and length are distinct. |

The `create_cell`, `create_environment`, `create_closure` and `create_list` APIs internally
protect the references they receive during their allocations. The caller must supply live
handles and root the result before the next operation that may collect. `list_push`
protects the list itself during a collection caused by growth; after the operation the
caller remains responsible for its root. `cell_set` and `list_set` neither allocate nor
collect.

Frames may end while closures remain alive: the closure → environment → cell chain
preserves the escaped capture. Iterative tracing also follows references in lists and
reclaims unreachable cycles, including cycles involving closures. The capacity of list and
environment buffers counts toward the byte estimate; that estimate is not RSS and does not
measure all allocator metadata.

Invalid handle types and indices cause a `panic` in the current Rust API. This is an
internal compiler/runtime contract, not an implementation of Dart exceptions. There are no
fixed/unmodifiable lists, iterators, removal, growth through the length setter, execution
of code by `code_id` or public FFI API for these structures yet.

References consulted in Dart SDK **3.6.2**, commit
`b0cc5495e0f5e8ae150825a5352e708cb49e65ff`:

- `runtime/vm/object.h`: separation of `Context`, `Closure` and collection objects.
- `sdk/lib/core/list.dart`: distinction between fixed and growable lists and indexed access.
- `runtime/vm/compiler/backend/flow_graph_compiler.cc`: collector reference maps.

These sources guide the contract; no SDK code or binary layout was copied. The tests use a
collection before every allocation to verify escape, cell aliasing, identity, cycles and
tag precision.
