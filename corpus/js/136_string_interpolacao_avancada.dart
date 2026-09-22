// Interpolação avançada: records, enums, coleções aninhadas, closures chamadas em ${}, literais adjacentes e expressões dentro.
enum Cor { vermelho, verde, azul }

enum Planeta {
  terra(1),
  marte(2);

  final int ordem;
  const Planeta(this.ordem);
  @override
  String toString() => 'Planeta#$ordem';
}

class Caixa<T> {
  final T v;
  Caixa(this.v);
  @override
  String toString() => 'Caixa<$v>';
}

String saudacao(String nome) => 'Olá, $nome!';
int dobro(int x) => x * 2;

void main() {
  var r = (1, 'a', true);
  print('$r');
  print('${r.$1} ${r.$2} ${r.$3}');
  var nomeado = (x: 1, y: 2.5);
  print('$nomeado');
  print('${nomeado.x + 1} e ${nomeado.y}');
  var misto = (0, nome: 'n', 1.5);
  print('$misto');
  print('${(1, 2)}');
  print('${(a: 1)}');
  print('${(1, (2, 3))}');
  print('${[(1, 2), (3, 4)]}');
  print('${(1, 2) == (1, 2)}');

  print('$Cor');
  print('${Cor.verde}');
  print('${Cor.verde.name}');
  print('${Cor.verde.index}');
  print('${Cor.values}');
  print('${Cor.values.map((c) => c.name).join('|')}');
  print('${Planeta.marte}');
  print('${Planeta.marte.name} ${Planeta.marte.ordem}');
  print('${Planeta.values}');
  print('${Cor.azul == Cor.azul} ${Cor.azul == Cor.verde}');

  print('${[1, [2, [3, [4]]]]}');
  print('${{'a': [1, {'b': 2}]}}');
  print('${{1, 2, {3}}}');
  print('${[]}');
  print('${{}}');
  print('${<int>{}}');
  print('${[null, [null]]}');
  print('${['a', 'b'].map((e) => '${e.toUpperCase()}${e}')}');
  print('${['a', 'b'].map((e) => e * 2).toList()}');
  print('${{'k': 'v'}.entries.first}');
  print('${{'k': 'v'}.entries.first.key}=${{'k': 'v'}.entries.first.value}');
  print('${[1, 2, 3].where((e) => e > 1)}');
  print('${[1, 2, 3].reversed}');
  print('${[1, 2, 3].map((e) => e * 2)}');
  print('${[3, 1, 2]..sort()}');
  print('${'abc'.split('')}');
  print('${'abc'.codeUnits}');
  print('${'abc'.runes}');
  print('${[Caixa(1), Caixa('x')]}');
  print('${Caixa([1, 2])}');
  print('${Caixa(Caixa(Cor.azul))}');
  print('${Caixa((1, 'r'))}');

  print('${saudacao('Ana')}');
  print('${dobro(21)}');
  print('${dobro(dobro(1))}');
  print('${(() => 'iife')()}');
  print('${((int x) => x + 1)(41)}');
  print('${[1, 2, 3].fold(0, (a, b) => a + b)}');
  var f = (String s) => s.length;
  print('${f('quatro')}');
  var lista = [saudacao, (String s) => s.toUpperCase()];
  print('${lista[0]('x')} ${lista[1]('y')}');
  print('${(() { var t = 0; for (var i = 1; i <= 4; i++) { t += i; } return t; })()}');
  print('${identical(1, 1) ? 'sim' : 'não'}');

  print('${'a' 'b'}');
  print('${'a' 'b' 'c'.length}');
  print('${('a' 'b').length}');
  print('${"x".length}');
  print("${'x'.length}");
  print('${'${'${'x'}'}'}');
  print('${'$r'.length}');
  print('${'abc'[1]}');
  print('${'abc'.substring(1)}');
  print('${"a" + "b"}');
  print('${'a' + '$r'}');
  print('${'"'}');
  print("${"'"}");
  print('${"\$"}');
  print('${'\n'.length}');
  print('${'\\'}');
  print('${r'\n'}');
  print('${r'$r'}');
  print('${'''
multi'''}');
  print('${"""a
b""".split('\n')}');

  print('${1 + 2}${3 + 4}');
  print('${1 + 2 * 3}');
  print('${(1 + 2) * 3}');
  print('${1 < 2}${2 < 1}');
  print('${true && false}');
  print('${null ?? 'padrão'}');
  print('${1 == 1 ? 'a' : 'b'}${2 == 1 ? 'a' : 'b'}');
  print('${[1, 2, 3][1]}');
  print('${{'k': 'v'}['k']}');
  print('${{'k': 'v'}['z']}');
  print('${[1, 2, 3].length > 2 ? [1, 2, 3].last : -1}');
  print('${7 ~/ 2}${7 % 2}${7 / 2}');
  print('${-1}${-r.$1}${-(1)}');
  print('${1 is int}${'x' is int}');
  print('${[1] is List<int>}');
  print('${(1 as num) is int}');
  int? talvez;
  print('${talvez}${talvez?.abs()}${talvez ?? 0}');
  talvez = -5;
  print('${talvez}${talvez.abs()}${talvez ?? 0}');
  print('a${''}b${''}c');
  print('${''}');
  print('${' '}|');
  print('${'a'}${'b'}${'c'}');
  print('${1}${'2'}${3.5}${true}${null}${[4]}${{5: 6}}${(7,)}${Cor.azul}');
  var partes = <String>[];
  for (var i = 0; i < 3; i++) {
    partes.add('${i}:${i * i}');
  }
  print('${partes.join(',')}');
  print('${partes}');
  var s = StringBuffer();
  s.write('${s.length}');
  s.write('${s.length}');
  s.write('${s}');
  print('$s');
  print('${'$s$s'}');
  var obj = Object();
  print('${obj.toString() == "Instance of 'Object'"}');
  print('${'$obj'.startsWith('Instance of')}');
  print('${DateTime.utc(2000)}');
  print('${Duration(seconds: 1)}');
  print('${Uri.parse('http://a/b')}');
  print('${#simbolo}');
  print('${#simbolo == #simbolo}');
  print('${1.5}${1 / 4}${-0.5}');
  print('${(1.5).toStringAsFixed(3)}');
  var pequeno = 1e-7;
  var grande = 1.5e300;
  print('$pequeno$grande');
  print('${9007199254740991}');
  print('${0x7FFFFFFF}');
  print('${'x'.padLeft(3, '.')}${'x'.padRight(3, '.')}');
}
