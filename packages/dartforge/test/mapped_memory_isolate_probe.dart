import 'dart:io';
import 'dart:isolate';

import 'package:dartforge/memory.dart';

void worker(List<Object?> arguments) {
  final mapping = arguments[0] as MappedMemory;
  final reply = arguments[1] as SendPort;
  var rejected = false;
  try {
    mapping.bytes[0];
  } on StateError {
    rejected = true;
  }
  reply.send(rejected);
}

Future<void> main() async {
  final directory = Directory.systemTemp.createTempSync('mapping-owner-');
  final file = File('${directory.path}/byte')..writeAsBytesSync([37]);
  final mapping = MappedMemory.open(file.path);
  final reply = ReceivePort();
  try {
    var rejected = false;
    try {
      await Isolate.spawn(worker, <Object?>[mapping, reply.sendPort]);
      rejected = await reply.first == true;
    } on ArgumentError {
      // The official VM rejects the graph containing a Finalizable outright.
      // A runtime permitting copies must reject access through the owner guard.
      rejected = true;
    }
    if (!rejected) throw StateError('Foreign isolate accessed the mapping');
    if (mapping.bytes[0] != 37 || mapping.isClosed) {
      throw StateError('The owner mapping changed during transfer');
    }
    print('mapped memory foreign owner access rejected');
  } finally {
    reply.close();
    mapping.close();
    directory.deleteSync(recursive: true);
  }
}
