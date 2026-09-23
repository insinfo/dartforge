// R-GEN-09: argumentos de tipo explícitos; parâmetro `T?`.
T id<T>(T x) => x;
T n<T>(T? x) => throw 0;
void main() {
  var a = /*@*/id<num>(/*@*/1);
  var b = /*@*/n(1);
  var c = /*@*/n(null);
  int? i;
  var d = /*@*/n(i);
  print([a, b, c, d]);
}
