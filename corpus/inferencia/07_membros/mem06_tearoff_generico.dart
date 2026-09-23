// R-MEM-06: tear-off de função genérica: instanciação implícita pelo
// contexto; instanciação explícita; tear-off de construtor.
T id<T>(T x) => x;

class C<T> {
  C();
  C.nomeado();
}

void main() {
  int Function(int) f = /*@*/id;
  var g = /*@*/id<String>;
  var h = /*@*/id;
  var k = /*@*/C.new;
  var l = /*@*/C<int>.nomeado;
  C<num> Function() m = /*@*/C.new;
  var n = /*@*/[1].map<String>;
  print([f, g, h, k, l, m, n]);
}
