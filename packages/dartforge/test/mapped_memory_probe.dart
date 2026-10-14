import 'dart:io';
import 'package:dartforge/memory.dart';
import 'package:dartforge/resources.dart';

void check(bool condition, String message) {
  if (!condition) throw StateError(message);
}

void rejects<T>(void Function() body, String message) {
  try {
    body();
  } catch (error) {
    if (error is T) return;
    rethrow;
  }
  throw StateError(message);
}

void main() {
  check(MappedMemory.isSupported, 'File mapping unsupported');
  final directory = Directory.systemTemp.createTempSync('mapped-memory-');
  final file = File('${directory.path}/bytes-ü.bin');
  try {
    file.writeAsBytesSync(List.generate(8197, (i) => i & 255));
    rejects<ArgumentError>(() => MappedMemory.open(''), 'Empty path accepted');
    rejects<ArgumentError>(
        () => MappedMemory.open('bad\u0000path'), 'NUL accepted');
    try {
      MappedMemory.open('${directory.path}/missing');
      throw StateError('Missing file opened');
    } on FileSystemException catch (error) {
      // The parent directory exists: ENOENT / ERROR_FILE_NOT_FOUND is 2.
      check(error.osError?.errorCode == 2, 'Original OS error code was lost');
    }
    rejects<RangeError>(() => MappedMemory.open(file.path, offset: 8198),
        'Bad offset accepted');
    rejects<RangeError>(
        () => MappedMemory.open(file.path, offset: 1, length: 8197),
        'Bad length accepted');
    final read = MappedMemory.open(file.path, offset: 3, length: 4100);
    final view = read.bytes;
    check(view.length == 4100 && view[0] == 3 && view[4099] == 6,
        'Unaligned view incorrect');
    check(read.view(offset: 7, length: 3).join(',') == '10,11,12',
        'Subview incorrect');
    rejects<RangeError>(() => view[-1], 'Negative index accepted');
    rejects<RangeError>(() => view[4100], 'Past-end index accepted');
    rejects<UnsupportedError>(() => view[0] = 9, 'Read-only write accepted');
    rejects<UnsupportedError>(read.flush, 'Read-only flush accepted');
    read.withBytes((raw) {
      check(raw.length == 4100 && raw[0] == 3, 'Raw read incorrect');
      rejects<UnsupportedError>(
          () => raw[0] = 2, 'Raw read-only write accepted');
      rejects<UnsupportedError>(
          () => raw.buffer.asUint8List()[0] = 2, 'Readonly buffer writable');
      rejects<StateError>(read.close, 'Close during borrow accepted');
    });
    final copy = view.sublist(0, 3);
    final iterator = view.iterator;
    check(iterator.moveNext(), 'Iterator failed');
    read.close();
    read.close();
    check(copy.join(',') == '3,4,5', 'Copy tied to mapping');
    rejects<StateError>(() => view[0], 'Closed read accepted');
    rejects<StateError>(() => view.length, 'Closed length accepted');
    rejects<StateError>(() => view.toList(), 'Closed copy accepted');
    rejects<StateError>(() => iterator.moveNext(), 'Closed iterator accepted');
    rejects<StateError>(() => read.withBytes((_) {}), 'Closed borrow accepted');
    final write =
        MappedMemory.open(file.path, offset: 4095, length: 4, writable: true);
    write.bytes[0] = 511;
    write.withBytes((raw) {
      raw[1] = 82;
    });
    rejects<StateError>(() => write.withBytes((_) => throw StateError('body')),
        'Body error lost');
    write.flush();
    write.dispose();
    final actual = file.readAsBytesSync();
    check(actual[4095] == 255 && actual[4096] == 82 && actual[4097] == 1,
        'Writeback incorrect');
    final empty = MappedMemory.open(file.path, offset: 8197, writable: true);
    check(empty.bytes.isEmpty && empty.withBytes((raw) => raw.isEmpty),
        'Empty view incorrect');
    empty.flush();
    empty.close();
    final zeroFile = File('${directory.path}/empty')..writeAsBytesSync([]);
    final zero = MappedMemory.open(zeroFile.path);
    check(zero.bytes.isEmpty, 'Empty file mapping incorrect');
    zero.close();
    final scoped = MappedMemory.open(file.path);
    check(using(scoped, (m) => m.bytes[3]) == 3 && scoped.isClosed,
        'Scoped close incorrect');
    print('mapped memory ranges, borrowing, writeback and lifecycle verified');
  } finally {
    directory.deleteSync(recursive: true);
  }
}
