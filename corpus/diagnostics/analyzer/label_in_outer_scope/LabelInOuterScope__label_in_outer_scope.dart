class A {
  void m(int i) {
    l: while (i > 0) {
      void f() {
//         ^
// [diag.unusedElement] The declaration 'f' isn't referenced.
        break l;
//            ^
// [diag.labelInOuterScope] Can't reference label 'l' declared in an outer method.
      };
    }
  }
}
