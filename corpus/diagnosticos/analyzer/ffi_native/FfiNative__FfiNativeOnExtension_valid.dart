import 'dart:ffi';

extension on int {
  @Native<Bool Function(Int64, Int64)>(symbol: 'x')
  external bool f(int m);
}

void g() {
  0.f(0);
}
