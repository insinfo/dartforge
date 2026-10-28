// Searches of a `String` by a `String` call the JavaScript methods in
// production; every result, the evaluation order and the paths left as
// calls must match the VM.

final log = <String>[];

String traced(String s) {
  log.add(s);
  return s;
}

extension on String {
  bool get shouted => startsWith('!') && endsWith('!');
  int firstDot() => indexOf('.');
}

void main() {
  final samples = ['', 'a', 'dart', 'dartdart', 'ação 😀 ação', '!hey!', 'a.b.c'];
  final patterns = ['', 'a', 'da', 'rt', 'dartdart', '😀', 'ção', '.', 'zz'];
  for (final s in samples) {
    for (final p in patterns) {
      print('${s.startsWith(p)} ${s.endsWith(p)} ${s.contains(p)} '
          '${s.indexOf(p)} ${s.lastIndexOf(p)}');
    }
  }
  print(traced('receiver').startsWith(traced('argument')));
  print(log);
  final a = 'da', b = 'rt';
  print((a + b).contains(b));
  String? maybe = samples.length > 3 ? 'value' : null;
  if (maybe != null) print(maybe.indexOf('l'));
  late final String late_ = 'late value';
  print(late_.lastIndexOf('a'));
  print(['!hey!'.shouted, 'hey'.shouted, 'a.b'.firstDot()]);
  // A `Pattern` argument and a start index keep the Dart methods.
  final Pattern regex = RegExp(r'[0-9]+');
  print(['ab12'.contains(regex), 'ab12'.indexOf(regex), 'ab12'.startsWith(regex, 2)]);
  print(['dart'.indexOf('a', 2), 'dart'.lastIndexOf('a', 0), 'dart'.contains('d', 1)]);
  try {
    'dart'.startsWith('d', 9);
  } on RangeError catch (e) {
    print('RangeError ${e.start} ${e.end}');
  }
  dynamic d = 'dynamic';
  print(d.startsWith('dyn'));
}
