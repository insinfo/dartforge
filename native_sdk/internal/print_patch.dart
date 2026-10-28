// Replaces `_internal/vm/lib/print_patch.dart` (overlay `native_sdk/`, P5c).
// On the VM, `printToConsole` calls the `_printClosure` closure that the
// embedder (`dart:io`/`builtin`) installs; here it is a runtime native that
// writes the line and the line terminator, like the VM's `print`.
//
// The `_dartforge*` functions are the errors the DartForge runtime throws
// (index out of range…) built as the source's Dart objects: the
// `dart:_internal` registration hands their addresses to the runtime
// (`dartforge_register_helper`, `crates/runtime/src/natives_lists.rs`).
//
// The `textBuffer*` functions are the text storage shared by `StringBuffer`
// (`core/string_buffer_patch.dart`) and the JSON stringifier that owns its
// output (`convert/json.dart`).

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

// ─── Text buffers ───────────────────────────────────────────────────────────
//
// The text written so far is the first `units` code units of a string that is
// never published: a `_OneByteString` until a unit above 0xFF arrives, then a
// `_TwoByteString`, so `toString` yields the same canonical form as the VM's
// `StringBuffer`. The owner keeps the buffer and the count in its own fields
// and copies a string into the room left inline ([copyStringUnits], one
// `memcpy`); these functions are the rest: growing, widening and the result.

/// The capacity of a buffer's first string, in code units (the first nine words
/// fit the generated code's inline allocation).
const int _textBufferFirstCapacity = 64;

/// Copies the [count] code units of [source] from unit [from] into [target]
/// from unit [at], which must have room for them, and answers true; answers
/// false, copying nothing, when [target] is one-byte and [source] two-byte.
/// Done inline by the native backend (`@df.string_write_units`).
@pragma("vm:external-name", "DartForge_string_copy_units")
external bool copyStringUnits(
    String target, int at, String source, int from, int count);

/// [buffer] — or the larger or wider string that replaces it, with its first
/// [units] code units — with [source] written after those units.
String textBufferAppend(String? buffer, int units, String source) {
  final int count = source.length;
  final int end = units + count;
  final bool wasTwo =
      buffer != null && ClassID.getID(buffer) == ClassID.cidTwoByteString;
  final bool two = wasTwo ||
      (ClassID.getID(source) == ClassID.cidTwoByteString &&
          !_fitsOneByte(source, count));
  String target;
  if (buffer == null || end > buffer.length || two != wasTwo) {
    final int old = buffer == null ? 0 : buffer.length;
    int capacity = old * 2;
    if (capacity < _textBufferFirstCapacity) {
      capacity = _textBufferFirstCapacity;
    }
    if (capacity < end) capacity = end;
    target =
        two ? allocateTwoByteString(capacity) : allocateOneByteString(capacity);
    if (buffer != null && units > 0) {
      copyStringUnits(target, 0, buffer, 0, units);
    }
  } else {
    target = buffer;
  }
  if (!copyStringUnits(target, units, source, 0, count)) {
    // A two-byte source whose units all fit in one byte.
    for (int i = 0; i < count; i++) {
      writeIntoOneByteString(target, units + i, source.codeUnitAt(i));
    }
  }
  return target;
}

bool _fitsOneByte(String source, int count) {
  for (int i = 0; i < count; i++) {
    if (source.codeUnitAt(i) > 0xFF) return false;
  }
  return true;
}

/// A new string with the first [units] code units of [buffer] (in its width).
String textBufferToString(String? buffer, int units) {
  if (buffer == null || units == 0) return "";
  final String result = ClassID.getID(buffer) == ClassID.cidTwoByteString
      ? allocateTwoByteString(units)
      : allocateOneByteString(units);
  copyStringUnits(result, 0, buffer, 0, units);
  return result;
}
