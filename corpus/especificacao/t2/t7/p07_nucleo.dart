import 'dart:async';

void f<T>(FutureOr<int>? a, Never b, Null c, dynamic Function(void) d, T? e,
    List<T?> g, Object? o) {
  String s1 = a;
  String s3 = c;
  String s4 = d;
  String s5 = e;
  String s6 = g;
  String s7 = o;
  print([s1, s3, s4, s5, s6, s7, b]);
}
