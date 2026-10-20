import 'dart:convert';
import 'dart:isolate';

const message = 'SDK literals: Ω 😀';

void worker(SendPort port) {
  final text = utf8.decode(utf8.encode(message));
  var rangeError = false;
  try {
    <int>[1][9];
  } on RangeError {
    rangeError = true;
  }
  port.send(jsonEncode(<String, Object>{'message': text, 'rangeError': rangeError}));
}

Future<void> main() async {
  print(jsonEncode(<String, Object>{'message': message, 'count': 3}));
  for (var index = 0; index < 3; index++) {
    final port = ReceivePort();
    await Isolate.spawn(worker, port.sendPort);
    print(await port.first);
    port.close();
  }
  print(base64.encode(utf8.encode('registered')));
}
