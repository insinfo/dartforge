// Replaces `_internal/vm/lib/print_patch.dart` (overlay `native_sdk/`, P5c).
// On the VM, `printToConsole` calls the `_printClosure` closure that the
// embedder (`dart:io`/`builtin`) installs; here it is a runtime native that
// writes the line and the line terminator, like the VM's `print`.
//
// The `_dartforge*` functions are the errors the DartForge runtime throws
// (index out of range…) built as the source's Dart objects: the
// `dart:_internal` registration hands their addresses to the runtime
// (`dartforge_register_helper`, `crates/runtime/src/natives_lists.rs`).

part of "internal_patch.dart";

@patch
@pragma("vm:external-name", "DartForge_print")
external void printToConsole(String line);

@pragma("vm:entry-point")
Object _dartforgeIndexError(int index, Object? target, int size) =>
    new IndexError.withLength(index, size, indexable: target, name: "index");

@pragma("vm:entry-point")
Object _dartforgeLateError(String nameArg, int code) {
  if (code == 0) return new LateError.fieldNI(nameArg);
  if (code == 1) return new LateError.fieldAI(nameArg);
  if (code == 2) return new LateError.localNI(nameArg);
  if (code == 3) return new LateError.localAI(nameArg);
  if (code == 4) return new LateError.fieldADI(nameArg);
  return new LateError.localADI(nameArg);
}

@pragma("vm:entry-point")
Object _dartforgeStackOverflowError() => const StackOverflowError();
