// R-CTX-08: parênteses e o alvo de uma cascata repassam o contexto.
void main() {
  List<num> a = (/*@*/[1]);
  List<num> b = /*@*/[1]..add(2.5);
  print([a, b]);
}
