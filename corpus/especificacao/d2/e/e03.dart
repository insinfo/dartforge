import 'dart:async';
enum E { a, b }
enum F { c }
extension type Id(int i) {}
extension type const Nm(String s) {}
class A { const A(); }
class B extends A { const B(); }
class Eq { const Eq(); bool operator ==(Object o) => true; int get hashCode => 0; }
void f(E e, F g, Id id, FutureOr<int> fo, FutureOr<int>? fon, A a, B b, Object o, dynamic dy, int Function() fn, void Function()? fq, (int, String) r, Null nu, Never nv) {
  if (e case F.c) {}
  if (e case E.a) {}
  if (g case null) {}
  if (id case 1) {}
  if (id case 'a') {}
  if (id case const Nm('a')) {}
  if (fo case 1) {}
  if (fo case 'a') {}
  if (fo case null) {}
  if (fon case null) {}
  if (a case const B()) {}
  if (b case const A()) {}
  if (b case const Eq()) {}
  if (o case 1) {}
  if (dy case 1) {}
  if (fn case 1) {}
  if (fn case null) {}
  if (fq case null) {}
  if (fn case f) {}
  if (r case 1) {}
  if (r case (1, 'a')) {}
  if (r case const (1, 2)) {}
  if (nu case 1) {}
  if (nu case null) {}
  if (nv case 1) {}
  if (o case int) {}
  if (e case #a) {}
  if (e case const [1]) {}
  if (e case const {1: 2}) {}
  if (e case 1.5) {}
}
