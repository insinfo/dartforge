// A patch of `dart:_http` (overlay `native_sdk/`, the `dartforge_native` section of
// `libraries.json`): the VM has none. `_CopyingBytesBuilder.add` copies a
// `String.codeUnits` (the header names and values of `_HttpHeaders._build`, the
// status line) with the string's own code units instead of indexing the
// `CodeUnits` list, whose class the loop retested at every byte. The result is the
// same: the list's units are the string's, and the `Uint8List` keeps the low
// eight bits of each.

// Copyright (c) 2013, the Dart project authors.  Please see the AUTHORS file
// for details. All rights reserved. Use of this source code is governed by a
// BSD-style license that can be found in the LICENSE file.

import "dart:_internal" show CodeUnits, patch;
import "dart:typed_data" show Uint8List;

@patch
class _CopyingBytesBuilder {
  @patch
  void add(List<int> bytes) {
    int bytesLength = bytes.length;
    if (bytesLength == 0) return;
    int required = _length + bytesLength;
    if (_buffer.length < required) {
      _grow(required);
    }
    assert(_buffer.length >= required);
    if (bytes is Uint8List) {
      _buffer.setRange(_length, required, bytes);
    } else if (bytes is CodeUnits) {
      _addCodeUnits(CodeUnits.stringOf(bytes), bytesLength);
    } else {
      for (int i = 0; i < bytesLength; i++) {
        _buffer[_length + i] = bytes[i];
      }
    }
    _length = required;
  }

  /// The first [count] code units of [string] at the end of the buffer, which
  /// already has room for them.
  void _addCodeUnits(String string, int count) {
    final Uint8List buffer = _buffer;
    final int start = _length;
    for (int i = 0; i < count; i++) {
      buffer[start + i] = string.codeUnitAt(i);
    }
  }
}
