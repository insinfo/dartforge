class S {
  S();
}
class A extends S {
  int x = 0;
  const A();
}
mixin M {
  int m = 0;
}
class B extends S with M {
  int x = 0;
  const B();
}
class C extends S {
  const C() : this.r();
  const C.r();
  C.nc();
  const C.q() : this.nc();
}
