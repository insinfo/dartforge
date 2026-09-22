// Convertido de tests/conformance/modules/colecoesops (módulo antigo do corpus de conformidade).
// Fixture de conformidade: coleções e operadores inteiros.
//
// Cobre literais Set, spreads `...`/`...?`, elementos `if`/`for`, acesso
// null-aware `?.`, divisão inteira `~/`, módulo `%`, bitwise, `Enum.values`
// e Map const. A saída esperada (EXPECTED em colecoes_ops.rs) foi copiada de
// `dart run` sem edição, idêntica nos SDKs Dart 3.6.2 e 3.13.4.
enum Cor { vermelho, verde, azul }

List<int> origem() {
  print('origem');
  return [7];
}

Iterable<int> gerador() sync* {
  print('gerador');
  yield 1;
  yield 2;
}

int efeito() {
  print('efeito');
  return 1;
}

void nulos(int? z, String? q, int? u, List<int>? n, List<int>? l) {
  print(z?.isEven);
  print(z);
  print(q?.length);
  var passo = u?.abs();
  print(passo?.toString());
  print(l?.length);
  print([0, ...?n]);
}

void condicao(bool t) {
  print([if (t) 1 else 2, if (!t) 3]);
}

void main() {
  var dois = 2;
  var conjunto = {1, 2, dois};
  print(conjunto);
  print(conjunto.length);
  var a = [1, 2];
  print([0, ...a, 3]);
  print({...{'y': 2}, 'x': 1});
  print({0, ...{1, 2}});
  print([...origem(), ...origem()]);
  condicao(true);
  print([for (var x in gerador()) x * 10]);
  nulos(null, 'oi', null, null, [1, 2, 3]);
  nulos(3, null, -4, [9], null);
  print(efeito().abs().toString());
  print(7 ~/ 2);
  print(-7 ~/ 2);
  print(7 ~/ -2);
  print(-7 % 3);
  print(7 % -3);
  print(5 | 3);
  print(5 & 3);
  print(5 ^ 3);
  print(~5 & 0xff); // ~5 sem máscara diverge: 32 bits sem sinal na web
  print(1 << 3);
  print(8 >> 2);
  print((-8 >> 2) & 0xf); // -8 >> 2 sem máscara diverge na web
  print(Cor.values);
  print(Cor.values.length);
  print(Cor.values[0]);
  print(identical(Cor.values, Cor.values));
  const mapa = {'a': 1, 'b': 2};
  print(mapa);
  print(mapa.length);
  print(mapa['a']);
}
