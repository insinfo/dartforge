// Strings: construção, interpolação, divisão e hash.
import 'comum.dart';

int construir(int n) {
  final sb = StringBuffer();
  for (var i = 0; i < n; i++) {
    sb.write('item $i;');
  }
  final s = sb.toString();
  return s.split(';').where((p) => p.endsWith('7')).length + s.length;
}

int hashes(int n) {
  var h = 0;
  for (var i = 0; i < n; i++) {
    h = (h * 31 + 'abc$i'.hashCode) & 0x3FFFFFFF;
  }
  return h;
}

void main() {
  medir('construir', () => construir(300000));
  medir('hashes', () => hashes(500000));
}
