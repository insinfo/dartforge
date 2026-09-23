// R-FLU-04: `?:` com condição que promove; `as` e `!` promovem a variável
// daí em diante.
void f(Object o, int? x) {
  var a = o is int ? (/*@*/o) : (/*@*/o);
  o as String;
  print(/*@*/o);
  x!;
  print(/*@*/x);
  print(a);
}

void main() => f('a', 1);
