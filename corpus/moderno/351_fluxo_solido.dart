// requer-dart: 3.13
// Fluxo sólido (3.9): `x != null` com `x` não anulável é sempre verdadeiro
// para alcançabilidade e atribuição definitiva. Getter e setter de tipos
// diferentes (3.9). Tipo de retorno de gerador (3.10).
class G {
  String get foo => 'getter';
  set foo(int v) => print('set $v');
}

void main() {
  var x = 0;
  String y;
  if (x != null) y = 'definido';
  print(y);

  int z = 5;
  String w;
  if (z == null) {
    print('inalcançável');
  } else {
    w = 'else';
  }
  print(w);

  var g = G();
  g.foo = 3;
  print(g.foo);

  f() sync* {
    yield 1;
    return;
  }
  print(f().toList());
  print(f is Iterable<int> Function());
}
