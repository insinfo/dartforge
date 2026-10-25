mixin M {
  int _p() => -1;
}

class A {
  int _q() => -1;
  int callQ() => _q();
}

int callP(M m) => m._p();
int callQ(A a) => a._q();
int dynamicP(dynamic m) => m._p();
int tearP(M m) => (m._p)();
