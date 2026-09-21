// Coleções e operadores: conjuntos, espalhamentos, elementos `if`/`for` e
// acesso null-aware. A saída é idêntica no Dart VM 3.6.2/3.13.4 e em dart2js.
class Caixa {
  final int v;
  Caixa? proxima;
  Caixa(this.v);
  int get valor {
    print('valor');
    return v;
  }

  int dobro() {
    print('dobro');
    return v * 2;
  }
}

Caixa? fonte(Caixa? c) {
  print('fonte');
  return c;
}

List<int>? talvez(bool b) {
  print('talvez');
  return b ? <int>[1, 2] : null;
}

Map<String, int>? mapa(bool b) => b ? <String, int>{'a': 1} : null;

void main() {
  // Conjuntos preservam a ordem de inserção e ignoram repetições.
  final repetido = 3;
  final s = <int>{3, 1, repetido, 2};
  print(s);
  print(s.length);
  print(s.contains(1));
  print(s.contains(8));
  print(s.add(1));
  print(s.add(9));
  print(s);
  for (final x in s) {
    print(x);
  }
  print(<int>{}.isEmpty);
  print(<int>{}.isNotEmpty);
  print(<int>{});
  print({});
  print(<int>{5, 6}.first);
  print(<int>{5, 6}.toList());
  print(<String>{'a'}.length);

  // Espalhamentos em lista, conjunto e mapa; `...?` omite o operando null.
  final base = <int>[1, 2];
  print([...base, 3]);
  print(<int>{...base, 1, 4});
  print([...?talvez(false), 9]);
  print([...?talvez(true), 9]);
  print(<String, int>{...mapa(true)!, 'b': 2});
  print(<String, int>{...?mapa(false), 'b': 2});
  print(<String, int>{...?mapa(true), 'a': 7});
  print([...<int>[], ...base]);

  // `if` e `for`, inclusive aninhados e combinados com espalhamentos.
  final n = s.length;
  print([if (n > 2) 'sim' else 'nao']);
  print([if (n > 90) 'sim']);
  print([if (n > 2) if (n > 90) 1 else 2]);
  print([for (var i = 0; i < 3; i++) i * i]);
  print([for (final x in base) for (final y in <int>[10, 20]) x * y]);
  print([for (final x in base) if (x > 1) x]);
  print([0, ...base, if (n > 2) 3, for (final x in base) x + 10]);
  print(<int>{for (final x in <int>[1, 2, 1]) x});
  print(<String, int>{if (n > 2) 'x': 1 else 'y': 2});
  print(<String, int>{for (final n in <String>['p', 'q']) n: 7});
  print(<String, int>{for (var i = 0; i < 2; i++) 'k': i});

  // Curto-circuito do `?.`: o receptor é avaliado uma única vez.
  final c = Caixa(1);
  c.proxima = Caixa(2);
  print(fonte(null)?.valor);
  print(fonte(c)?.valor);
  print(fonte(c)?.proxima?.valor);
  print(fonte(null)?.proxima?.valor);
  print(fonte(c)?.dobro());
  print(talvez(false)?[0]);
  print(talvez(true)?[0]);
  print(base[1]);
  print(mapa(false)?['x']);
  print(mapa(true)?['a']);

  // Bits e deslocamentos onde a VM e dart2js concordam.
  print(-1 & 3);
  print(6 & 3);
  print(6 | 3);
  print(6 ^ 3);
  print(-5 & 255);
  print(3 << 4);
  print(1 << 30);
  print(255 >> 4);
  print(255 >>> 4);
  print(0 >>> 0);
  print(1 << 0);
  print((1 << 4) | (1 << 2));
  print(7 & 3 | 8 ^ 1);
  print(1 + 2 << 3);
}
