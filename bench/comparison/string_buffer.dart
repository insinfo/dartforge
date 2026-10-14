/// Functional StringBuffer/JSON/isolate oracle for accumulator representation.
/// UTF-16 code units are inspected directly, including invalid surrogate text.
import 'dart:convert';
import 'dart:isolate';

void require(bool condition, String message) {
  if (!condition) throw StateError(message);
}

void units(String label, String actual, List<int> expected) {
  final codes = actual.codeUnits;
  require(codes.length == expected.length, '$label length');
  for (var i = 0; i < codes.length; i++) {
    require(codes[i] == expected[i], '$label unit $i');
  }
  print('$label ${codes.join(',')}');
}

final class Rendered {
  Rendered(this.name, this.log, {this.fail = false});
  final String name;
  final List<String> log;
  final bool fail;
  @override
  String toString() {
    log.add(name);
    if (fail) throw StateError('render failure');
    return '<$name>';
  }
}

void bufferWorker(List<Object> message) {
  final reply = message[0] as SendPort;
  final buffer = message[1] as StringBuffer;
  final before = buffer.toString();
  buffer.write('|worker');
  reply.send([before.codeUnits, buffer.toString().codeUnits, buffer]);
  if (message.length > 2) {
    buffer.write('|after-send');
    (message[2] as SendPort).send(buffer.toString().codeUnits);
  }
}

Future<void> narrowBufferRoundTrip(String label, List<int> initialUnits) async {
  final outgoing = StringBuffer()..write(String.fromCharCodes(initialUnits));
  final before = outgoing.toString();
  final replies = ReceivePort();
  final mutations = ReceivePort();
  try {
    await Isolate.spawn(bufferWorker,
        [replies.sendPort, outgoing, mutations.sendPort]);
    outgoing.write('|parent');
    final reply = await replies.first.timeout(const Duration(seconds: 5))
        as List<dynamic>;
    final childMutation = await mutations.first.timeout(const Duration(seconds: 5))
        as List<dynamic>;
    final childBefore = String.fromCharCodes((reply[0] as List<dynamic>).cast<int>());
    final childAfter = String.fromCharCodes((reply[1] as List<dynamic>).cast<int>());
    final returned = reply[2] as StringBuffer;
    require(childBefore == before && childAfter == before + '|worker',
        '$label outgoing snapshot copied');
    require(returned.toString() == childAfter,
        '$label returned snapshot unaffected by child mutation');
    returned.write('|return');
    require(outgoing.toString() == before + '|parent' &&
        returned.toString() == before + '|worker|return' &&
        String.fromCharCodes(childMutation.cast<int>()) == before + '|worker|after-send',
        '$label mutations are independent in both directions');
    units(label, childBefore, initialUnits);
    print('$label buffer copies verified');
  } finally {
    replies.close();
    mutations.close();
  }
}

Future<void> main() async {
  final buffer = StringBuffer();
  require(buffer.isEmpty && buffer.length == 0, 'empty buffer');
  buffer.write('ASCII');
  buffer.writeCharCode(0);
  buffer.write(String.fromCharCodes([0x7f, 0x80, 0xff]));
  final oneByte = buffer.toString();
  units('one-byte', oneByte, [65, 83, 67, 73, 73, 0, 127, 128, 255]);
  buffer.writeCharCode(0x100);
  buffer.write('中');
  buffer.writeCharCode(0x1f600);
  buffer.write(String.fromCharCodes([0xd800, 120, 0xdc00]));
  final mixed = buffer.toString();
  units('mixed', mixed, [65, 83, 67, 73, 73, 0, 127, 128, 255,
    256, 0x4e2d, 0xd83d, 0xde00, 0xd800, 120, 0xdc00]);
  require(buffer.length == 16 && buffer.isNotEmpty, 'UTF-16 length');
  final repeated = buffer.toString();
  require(repeated == mixed, 'repeated snapshot');
  buffer.write('tail');
  require(buffer.toString() == mixed + 'tail', 'mutation after snapshot');
  require(oneByte.codeUnitAt(8) == 255 && oneByte.length == 9 && mixed.length == 16,
      'prior snapshots remain immutable');
  buffer.clear();
  require(buffer.isEmpty && buffer.toString().isEmpty, 'clear');
  buffer.write('reset');
  units('clear-reset', buffer.toString(), [114, 101, 115, 101, 116]);

  final log = <String>[];
  buffer.clear();
  buffer.writeAll([Rendered('a', log), null, Rendered('b', log)], '|');
  buffer.writeln('!');
  require(buffer.toString() == '<a>|null|<b>!\n' && log.join(',') == 'a,b',
      'writeAll conversion order');
  print('write-all order=${log.join(",")} units=${buffer.toString().codeUnits.join(",")}');
  var failed = false;
  try {
    buffer.writeAll([Rendered('c', log), Rendered('fail', log, fail: true),
        Rendered('never', log)], ':');
  } on StateError catch (error) {
    failed = error.message == 'render failure';
  }
  require(failed && log.join(',') == 'a,b,c,fail' &&
      buffer.toString() == '<a>|null|<b>!\n<c>:', 'partial state after conversion error');
  for (final code in [-1, 0x110000]) {
    final before = buffer.toString();
    var rangeError = false;
    try { buffer.writeCharCode(code); } on RangeError { rangeError = true; }
    require(rangeError && buffer.toString() == before, 'invalid code point unchanged');
  }
  print('conversion-error order=${log.join(",")} partial-state preserved; range-errors=2');

  final odd = String.fromCharCodes([0xd800, 65, 0xdc00]);
  final object = <String, Object?>{
    'ascii': 'quotes " and slash \\ and\nline',
    'latin': String.fromCharCodes([0x80, 0xff]),
    'nested': [null, true, false, 17, -0.0, {'wide': '中😀', 'odd': odd}],
  };
  final encoded = jsonEncode(object);
  final decoded = jsonDecode(encoded) as Map<String, dynamic>;
  require(decoded['ascii'] == object['ascii'] && decoded['latin'] == object['latin'],
      'JSON scalar strings');
  final nested = decoded['nested'] as List<dynamic>;
  require(nested[0] == null && nested[1] == true && nested[2] == false &&
      nested[3] == 17 && (nested[4] as double).isNegative, 'JSON scalar values');
  final inner = nested[5] as Map<String, dynamic>;
  require(inner['wide'] == '中😀' && inner['odd'] == odd, 'JSON UTF-16 round trip');
  require(jsonEncode(decoded) == encoded, 'JSON deterministic round trip');
  print('json units=${encoded.codeUnits.join(",")}');
  final pretty = const JsonEncoder.withIndent(' ').convert(object);
  require(jsonEncode(jsonDecode(pretty)) == encoded, 'pretty JSON round trip');
  print('json pretty round trip verified');

  await narrowBufferRoundTrip('isolate-ascii', [65, 0, 127]);
  await narrowBufferRoundTrip('isolate-latin1', [65, 128, 255]);

  final outgoing = StringBuffer()..write('copy:')..write(odd)..write('中😀');
  final beforeSend = outgoing.toString();
  final port = ReceivePort();
  try {
    await Isolate.spawn(bufferWorker, [port.sendPort, outgoing]);
    outgoing.write('|parent');
    final reply = await port.first.timeout(const Duration(seconds: 5)) as List<dynamic>;
    final childBefore = String.fromCharCodes((reply[0] as List<dynamic>).cast<int>());
    final childAfter = String.fromCharCodes((reply[1] as List<dynamic>).cast<int>());
    final returned = reply[2] as StringBuffer;
    require(childBefore == beforeSend && childAfter == beforeSend + '|worker',
        'worker receives copied snapshot');
    require(outgoing.toString() == beforeSend + '|parent', 'parent buffer independent');
    require(returned.toString() == childAfter, 'returned accumulator copied');
    returned.write('|return');
    require(outgoing.toString() == beforeSend + '|parent', 'return mutation independent');
    units('isolate-before', childBefore, beforeSend.codeUnits);
    print('isolate buffer copies verified');
  } finally {
    port.close();
  }
}
