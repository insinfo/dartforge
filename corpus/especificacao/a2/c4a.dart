class C<T> { C(); C.n(); static void s() {} void i() {} static int get g => 0; }
typedef F<T> = void Function();
void f() {
  C<int>.s;
  C<int>.i;
  C<int>.g;
  C<int>.nada;
  C<int>.n;
  C<int>.s();
  var l = <int, int>[];
  var m = <int>{1: 2};
  var m3 = <int, int, int>{};
  var s = <int, int>{1};
  var e = <int, int>{};
  const cl = <int, int>[];
  int x = 0;
  x<int>;
  f<int>;
  print([l, m, m3, s, e, cl]);
}
extension E on int {}
var a = E;
var b = [E];
void g() { E; E.toString(); print(E); }
