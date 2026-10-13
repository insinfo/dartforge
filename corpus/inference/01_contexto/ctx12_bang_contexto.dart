// R-CTX-12: o operando de `e!` recebe o contexto K? .
T? g<T>() => null;
void main() {
  int x = /*@*/g()!;
  List<num> y = /*@*/g()!;
  print([x, y]);
}
