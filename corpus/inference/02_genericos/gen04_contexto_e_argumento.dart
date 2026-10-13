// R-GEN-04: com contexto e argumento, T <: contexto (para baixo) e
// T :> argumento (para cima): escolhe-se o limite inferior.
T id<T>(T x) => x;
void main() {
  num n = /*@*/id(1);
  Object o = /*@*/id('a');
  num? m = /*@*/id(null);
  print([n, o, m]);
}
