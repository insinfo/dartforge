// R-FLU-06: declaração: `var x;` sem inicializador, `var` com tipo variável,
// e o tipo declarado não é promovido pelo inicializador.
void f<T>(T t) {
  var a;
  a = 1;
  print(/*@*/a);
  num b = 1;
  print(/*@*/b);
  int? c = 1;
  print(/*@*/c);
  if (t is int) {
    var d = t;
    print(/*@*/d);
  }
  int e;
  e = 2;
  print(/*@*/e);
}

void main() => f(1);
