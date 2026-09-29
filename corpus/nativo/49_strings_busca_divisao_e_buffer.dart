// As buscas e a divisão de `String` feitas no runtime (`nativos_strings.rs`:
// `startsWith`, `endsWith`, `indexOf`, `lastIndexOf`, `contains`, `split`
// por um caractere) e o `StringBuffer` com acumulador do runtime
// (`string_buffer_patch.dart`): strings de um e de dois bytes, padrões
// vazios, maiores que o texto, índices nas bordas e os erros da VM.
class Ponto {
  final int x, y;
  Ponto(this.x, this.y);
  String toString() => 'Ponto($x, $y)';
}

void tentar(String nome, Object? Function() f) {
  try {
    print('$nome: ${f()}');
  } catch (e) {
    print('$nome: ${e.runtimeType} $e');
  }
}

void main(List<String> args) {
  final textos = ['', 'a', 'abc', 'abcabc', 'ção ção', 'Ā€𝄞xĀ', 'xxxxxxxxxx', 'a;b;;c;', ';', 'item 17;item 27;'];
  final padroes = ['', 'a', 'c', 'abc', 'bca', 'ção', 'Ā', '€𝄞', '𝄞', 'x', 'xx', ';', 'item', '7;', 'z'];
  for (final t in textos) {
    for (final p in padroes) {
      final r = [
        t.startsWith(p),
        t.endsWith(p),
        t.indexOf(p),
        t.lastIndexOf(p),
        t.contains(p),
        if (t.isNotEmpty) t.indexOf(p, t.length ~/ 2),
        if (t.isNotEmpty) t.lastIndexOf(p, t.length ~/ 2),
        if (t.isNotEmpty) t.startsWith(p, 1),
        t.indexOf(p, t.length),
        t.lastIndexOf(p, 0),
      ];
      print('"$t" "$p" $r');
    }
    print('"$t" split: ${t.split(';')} ${t.split('a')} ${t.split('Ā')} ${t.split('')}');
  }
  tentar('indexOf fora', () => 'abc'.indexOf('a', 4));
  tentar('indexOf negativo', () => 'abc'.indexOf('a', -1));
  tentar('lastIndexOf fora', () => 'abc'.lastIndexOf('a', 4));
  tentar('startsWith fora', () => 'abc'.startsWith('a', 4));
  tentar('contains fora', () => 'abc'.contains('a', 4));
  tentar('contains regexp', () => 'abc'.contains(RegExp('b.')));
  tentar('indexOf regexp', () => 'abcabc'.indexOf(RegExp('c'), 3));
  tentar('split regexp', () => 'a1b22c'.split(RegExp(r'\d+')));

  final sb = StringBuffer();
  print('vazio: "${sb.toString()}" ${sb.length} ${sb.isEmpty}');
  sb.write('abc');
  sb.write(12);
  sb.write(null);
  sb.write(Ponto(1, 2));
  sb.write('');
  sb.writeCharCode(0x41);
  sb.writeCharCode(0x100);
  sb.writeCharCode(0x1D11E);
  sb.writeln('fim');
  sb.writeln();
  sb.writeAll([1, 'dois', 3.5, null], ', ');
  sb.writeAll(<Object>[]);
  sb.writeAll(['x', 'y']);
  print(sb.toString());
  print('${sb.length} ${sb.isEmpty} ${sb.isNotEmpty}');
  final antes = sb.toString();
  sb.write('mais');
  print('${antes.length} ${sb.toString().length} ${identical(antes, sb.toString())}');
  sb.clear();
  print('limpo: "$sb" ${sb.length}');
  sb.write('ção');
  print('$sb ${sb.length} ${sb.toString().codeUnits}');
  final grande = StringBuffer('início:');
  for (var i = 0; i < 2000; i++) {
    grande.write('item $i;');
  }
  final g = grande.toString();
  print('${g.length} ${g.hashCode} ${g.split(';').length} ${g.split(';').where((p) => p.endsWith('7')).length}');
  print(g.substring(0, 40));
  print(g.lastIndexOf('item 1'));
  print(g.indexOf('item 1999'));
  try {
    StringBuffer().writeCharCode(-1);
  } catch (e) {
    print('writeCharCode: ${e.runtimeType}');
  }
}
