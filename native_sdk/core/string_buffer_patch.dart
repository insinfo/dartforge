// Replaces `_internal/vm_shared/lib/string_buffer_patch.dart` (overlay
// `native_sdk/`, P5c). The VM's joins the parts in a list and the loose units
// in a `Uint16List`, compacts the list from time to time and creates the
// string through a native. Here the written units go straight to a runtime
// accumulator (`_AcumuladorDeTexto`, an object with the units vector
// attached, released by the collector with it; `natives_strings.rs`): one
// call per `write`, without an object per part and without the covariance
// check of the list's `add`; `toString` copies what was accumulated into a new
// string (the `StringBuffer` can keep growing). What the program observes —
// the text, `length`, `isEmpty` — is the same.

import "dart:_internal" show patch;

@patch
class StringBuffer {
  /// The runtime accumulator; `null` while empty.
  Object? _accumulator;

  /// The code units written.
  int _units = 0;

  @patch
  StringBuffer([Object content = ""]) {
    write(content);
  }

  @patch
  int get length => _units;

  @patch
  void write(Object? obj) {
    // The string goes to the accumulator in a single call (the `is String` test,
    // `isEmpty` and `length` stay in the runtime); the rest goes through
    // `toString`, as on the VM.
    final int n = _writeIfString(_accumulator ??= _create(), obj);
    if (n >= 0) {
      _units += n;
      return;
    }
    String str = "$obj";
    if (str.isEmpty) return;
    _write(_accumulator!, str);
    _units += str.length;
  }

  // The units go straight to the accumulator, without the one-character string
  // that `String.fromCharCode` allocated (the `writeCharCode` of
  // `_JsonStringifier` and of `_JsonStringParser` writes through here); the
  // checks and the `RangeError` are the VM's.
  @patch
  void writeCharCode(int charCode) {
    if (charCode <= 0xFFFF) {
      if (charCode < 0) {
        throw new RangeError.range(charCode, 0, 0x10FFFF);
      }
      _writeCharCodeNative(_accumulator ??= _create(), charCode);
      _units += 1;
    } else {
      if (charCode > 0x10FFFF) {
        throw new RangeError.range(charCode, 0, 0x10FFFF);
      }
      _writeCharCodeNative(_accumulator ??= _create(), charCode);
      _units += 2;
    }
  }

  @patch
  void writeAll(Iterable objects, [String separator = ""]) {
    Iterator iterator = objects.iterator;
    if (!iterator.moveNext()) return;
    if (separator.isEmpty) {
      do {
        write(iterator.current);
      } while (iterator.moveNext());
    } else {
      write(iterator.current);
      while (iterator.moveNext()) {
        write(separator);
        write(iterator.current);
      }
    }
  }

  @patch
  void writeln([Object? obj = ""]) {
    write(obj);
    write("\n");
  }

  @patch
  void clear() {
    _accumulator = null;
    _units = 0;
  }

  @patch
  String toString() {
    final accumulator = _accumulator;
    if (accumulator == null) return "";
    return _toStringNative(accumulator);
  }

  @pragma("vm:external-name", "DartForge_sb_new")
  external static Object _create();

  @pragma("vm:external-name", "DartForge_sb_write")
  external static void _write(Object accumulator, String str);

  /// If `obj` is a `String`, appends its units and returns how many; otherwise
  /// -1 (changing nothing).
  @pragma("vm:external-name", "DartForge_sb_write_if_string")
  external static int _writeIfString(Object accumulator, Object? obj);

  /// Appends the code point `code` (0..0x10FFFF, checked by Dart): one unit, or
  /// the surrogate pair above 0xFFFF.
  @pragma("vm:external-name", "DartForge_sb_write_char_code")
  external static void _writeCharCodeNative(Object accumulator, int code);

  @pragma("vm:external-name", "DartForge_sb_to_string")
  external static String _toStringNative(Object accumulator);
}
