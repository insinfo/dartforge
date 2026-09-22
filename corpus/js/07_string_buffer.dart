// StringBuffer: write/writeln/writeAll/writeCharCode, length, isEmpty, clear, toString e uso em laço.
void main() {
  var sb = StringBuffer();
  print(sb.isEmpty);
  print(sb.length);
  sb.write('olá');
  print(sb.length);
  print(sb.isNotEmpty);
  sb.write(' ');
  sb.write(42);
  sb.write(' ');
  sb.write(true);
  sb.write(' ');
  sb.write(null);
  print(sb.toString());
  sb.clear();
  print(sb.isEmpty);
  print(sb.toString().isEmpty);

  sb.writeln('primeira');
  sb.writeln();
  sb.writeln('terceira');
  print(sb.toString());
  print(sb.length);
  sb.clear();

  sb.writeAll([1, 2, 3]);
  print(sb);
  sb.clear();
  sb.writeAll([1, 2, 3], ', ');
  print(sb);
  sb.clear();
  sb.writeAll(['a', null, 3.5], '|');
  print(sb);
  sb.clear();
  sb.writeAll([], ',');
  print(sb.isEmpty);

  sb.writeCharCode(72);
  sb.writeCharCode(105);
  sb.writeCharCode(0x1F600);
  sb.writeCharCode(0xE9);
  print(sb);
  print(sb.length);
  sb.clear();

  for (var i = 0; i < 5; i++) {
    if (i > 0) sb.write('-');
    sb.write(i * i);
  }
  print(sb);
  sb.clear();

  var sb2 = StringBuffer('inicial');
  print(sb2);
  sb2.write(' mais');
  print(sb2.toString());
  print(sb2.length);
  var sb3 = StringBuffer(123);
  print(sb3);
  var sb4 = StringBuffer(sb2);
  sb4.write('!');
  print(sb4);
  print(sb2);
  sb.write(sb4);
  print(sb);
  print('${StringBuffer('x')..write('y')}');
  var s1 = sb.toString();
  sb.write('z');
  print(s1);
  print(sb.toString() == s1 + 'z');
  var lines = StringBuffer();
  for (var w in ['um', 'dois', 'tres']) {
    lines.writeln('${w.length}:$w');
  }
  print(lines.toString().trimRight());
  var sb5 = StringBuffer()
    ..write('a')
    ..write(1)
    ..writeln('!')
    ..writeAll(['x', 'y'], '+');
  print(sb5);
  print(sb5.toString().split('\n'));
  sb5.write(sb5);
  print(sb5.length);
  sb5.write('');
  print(sb5.length);
  print(StringBuffer('').isEmpty);
  print(StringBuffer('').toString() == '');
}
