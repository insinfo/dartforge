class A {}
int A = 0;
mixin B {}
class B {}
class C {}
enum C { v }
void D() {}
class D {}
typedef E = int;
class E {}
void f() {
  A.isEven;
  B? b;
  C.v;
  D();
  E e = 0;
  print([b, e]);
}
class X extends B {}
class Y with B {}
