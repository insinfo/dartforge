// R-CTX-04: o argumento recebe o tipo do parâmetro (com a substituição
// parcial dos argumentos de tipo já inferidos) como contexto.
void f(List<num> x, {Set<Object>? y}) {}
T id<T>(T x) => x;
void main() {
  f(/*@*/[1], y: /*@*/{1});
  double d = /*@*/id(/*@*/1);
  List<num> l = /*@*/id(/*@*/[1]);
  print([d, l]);
}
