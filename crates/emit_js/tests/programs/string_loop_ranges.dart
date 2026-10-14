import 'dart:convert';

int direct(String text) {
  var sum = 0;
  for (var i = 0; i < text.length; i++) {
    sum += text.codeUnitAt(i);
  }
  return sum;
}

int cached(String text) {
  final length = text.length;
  var sum = 0;
  for (var i = 0; i < length; i++) {
    sum += text.codeUnitAt(i);
  }
  return sum;
}

String offsets(String text) {
  var sum = 0;
  try {
    for (var i = 0; i < text.length; i++) {
      sum += text.codeUnitAt(i + 1);
    }
  } on RangeError catch (error) {
    return '$sum:range=${error.invalidValue}';
  }
  return '$sum';
}

int mutableSource(String text) {
  var sum = 0;
  for (var i = 0; i < text.length; i++) {
    sum += text.codeUnitAt(i);
    text = 'xyz';
  }
  return sum;
}

int capturedSource(String text) {
  void mutate() { text = 'xyz'; }
  var sum = 0;
  for (var i = 0; i < text.length; i++) {
    sum += text.codeUnitAt(i);
    mutate();
  }
  return sum;
}

int mutableIndex(String text) {
  var sum = 0;
  for (var i = 0; i < text.length; i++) {
    sum += text.codeUnitAt(i);
    if (i == 0) i = 1;
  }
  return sum;
}

int mutableBound(String text) {
  var length = text.length;
  var sum = 0;
  for (var i = 0; i < length; i++) {
    sum += text.codeUnitAt(i);
    length--;
  }
  return sum;
}

String writeWithCalls(String text, StringBuffer output) {
  final length = text.length;
  for (var i = 0; i < length; i++) {
    final unit = text.codeUnitAt(i);
    if (unit < 32) {
      output.write('control:');
    }
    output.write('$unit;');
  }
  return output.toString();
}

class CustomJson {
  final List<String> events;
  CustomJson(this.events);
  Object toJson() { events.add('toJson'); return '\uD800\uDC00\uDC00'; }
}

void main() {
  final samples = ['', 'ASCII', '\u0000\u007f\u0080\u00ff', '😀\uD800x\uDC00', 'é中文'];
  for (final text in samples) {
    print('units=${direct(text)}:${cached(text)}:${offsets(text)}');
  }
  print('mutation=${mutableSource('abc')}:${capturedSource('abc')}:${mutableIndex('abc')}:${mutableBound('abc')}');
  print(writeWithCalls('A\u0001😀', StringBuffer()));
  final events = <String>[];
  final object = {'a': '\uD800\uDC00\uDC00', 'b': [CustomJson(events), '\u0000\n"\\é😀']};
  final encoded = jsonEncode(object);
  print(encoded);
  print(JsonEncoder.withIndent('  ').convert(object));
  print(utf8.decode(json.fuse(utf8).encode(object)) == encoded);
  print('events=$events');
}
