// R-EXT-03: extensão em tipo anulável, operador e `call` de extensão.
extension on int? {
  bool get vazio => this == null;
}

extension on String {
  String operator -(int n) => substring(n);
  int call() => length;
}

void f(int? x) {
  print([/*@*/x.vazio, /*@*/'abc' - 1, /*@*/'abc'()]);
}

void main() => f(1);
