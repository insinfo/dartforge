// R-PAD-05: padrões relacionais, lógicos, null-check, null-assert e cast.
void f(int? n, Object o, num k) {
  switch (n) {
    case var v? when v > 0:
      print(/*@*/v);
    default:
  }
  switch (k) {
    case > 0 && < 10:
      print(/*@*/k);
    default:
  }
  if (o case int() || double()) print(/*@*/o);
  var (a!, b) = (n, 1);
  var [c as int, d as String] = <Object>[1, 'x'];
  if (k case int j && > 3) print(/*@*/j);
  print([/*@*/a, /*@*/b, /*@*/c, /*@*/d]);
}

void main() => f(1, 2, 3);
