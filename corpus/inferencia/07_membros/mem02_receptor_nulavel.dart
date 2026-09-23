// R-MEM-02: receptor anulável: membros de Object são acessíveis; `?.`
// dá o tipo anulável.
void f(int? x) {
  print([/*@*/x.toString(), /*@*/x.hashCode, /*@*/x?.isEven, /*@*/x.runtimeType, /*@*/x == 1]);
}

void main() => f(1);
