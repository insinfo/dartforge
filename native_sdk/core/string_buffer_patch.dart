// Replaces `_internal/vm_shared/lib/string_buffer_patch.dart` (overlay
// `native_sdk/`, P5c). The VM's joins the parts in a list and the loose units
// in a `Uint16List`, compacts the list from time to time and creates the
// string through a native. Here the written units go straight into a text
// buffer (`dart:_internal`, `textBufferAppend`): a string that is never
// published, whose first `_units` code units are the text, grown by doubling.
// A `write` of a string that fits is one inline copy (`copyStringUnits`)
// without a runtime call, an object per part or the covariance check of the
// list's `add`; `toString` copies the text into a new string (the
// `StringBuffer` can keep growing). What the program observes — the text,
// `length`, `isEmpty` — is the same.

import "dart:_internal"
    show
        copyStringUnits,
        patch,
        textBufferAppend,
        textBufferToString,
        writeIntoOneByteString,
        writeIntoTwoByteString;

@patch
class StringBuffer {
  /// The text buffer; null while nothing was written.
  String? _buffer;

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
    final String str = obj is String ? obj : "$obj";
    final int count = str.length;
    final int at = _units;
    final String? buffer = _buffer;
    if (buffer != null &&
        at + count <= buffer.length &&
        copyStringUnits(buffer, at, str, count)) {
      _units = at + count;
      return;
    }
    if (count == 0) return;
    _buffer = textBufferAppend(buffer, at, str);
    _units = at + count;
  }

  // The unit goes straight into the buffer, without the one-character string
  // that `String.fromCharCode` allocates (the `writeCharCode` of
  // `_JsonStringParser` writes through here); the checks and the `RangeError`
  // are the VM's.
  @patch
  void writeCharCode(int charCode) {
    if (charCode <= 0xFF) {
      if (charCode < 0) {
        throw new RangeError.range(charCode, 0, 0x10FFFF);
      }
      final int at = _units;
      final String? buffer = _buffer;
      if (buffer != null && at < buffer.length) {
        if (buffer is _OneByteString) {
          writeIntoOneByteString(buffer, at, charCode);
        } else {
          writeIntoTwoByteString(buffer, at, charCode);
        }
        _units = at + 1;
        return;
      }
    } else if (charCode > 0x10FFFF) {
      throw new RangeError.range(charCode, 0, 0x10FFFF);
    }
    // No room, or a unit above 0xFF: the buffer grows or widens (not through
    // `write`, which a subclass may override).
    final String str = new String.fromCharCode(charCode);
    final int at = _units;
    _buffer = textBufferAppend(_buffer, at, str);
    _units = at + str.length;
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

  // The line terminator is not written through `write` (which a subclass may
  // override), as on the VM.
  @patch
  void writeln([Object? obj = ""]) {
    write(obj);
    final int at = _units;
    _buffer = textBufferAppend(_buffer, at, "\n");
    _units = at + 1;
  }

  @patch
  void clear() {
    _buffer = null;
    _units = 0;
  }

  @patch
  String toString() => textBufferToString(_buffer, _units);
}
