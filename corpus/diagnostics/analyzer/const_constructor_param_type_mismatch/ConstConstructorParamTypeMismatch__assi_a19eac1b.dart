class A {
  const A(Unresolved x);
//        ^^^^^^^^^^
// [diag.undefinedClass] Undefined class 'Unresolved'.
}
var v = const A(null);
