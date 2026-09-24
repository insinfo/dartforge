class A {
  noSuchMethod(x) => super.noSuchMethod(x);
}
class B extends A {
  mmm();
  noSuchMethod(y) {
//^^^^^^^^^^^^
// [diag.unnecessaryNoSuchMethod] Unnecessary 'noSuchMethod' declaration.
    return super.noSuchMethod(y);
  }
}
