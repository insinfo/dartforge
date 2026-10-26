// Um `typedef` usado como valor é o `Type` do tipo apelidado (com e sem
// argumentos de tipo); uma classe genérica sem argumentos é instanciada aos
// limites. `continue rótulo` para outro `case` funciona também num `switch`
// sobre `dynamic` (a forma com testes de padrão) e para um `default:`.
typedef Fun = int Function(int);
typedef G<T> = List<T>;
typedef CA = C;

class C {}

class B<T extends num> {}

String test(int i) {
  switch (i) {
    a:
    case 5:
      if (i == 0) return 'a';
      i -= 1;
      continue a;
    b:
    default:
      if (i == 10) continue a;
      if (i == 7) return 'b';
      i = 7;
      continue b;
  }
}

void main() {
  print([Fun, G<int>, CA, B, identical(CA, C), Fun == Fun]);
  print([test(5), test(10), test(7), test(3)]);

  final r = [];
  for (dynamic s in ['moo', 'woof', 'x']) {
    switch (s) {
      MOO:
      case 'moo':
        r.add('moo');
        break;
      case 'woof':
        continue MOO;
      default:
        r.add('default');
    }
    r.add('after $s');
  }
  print(r);
}
