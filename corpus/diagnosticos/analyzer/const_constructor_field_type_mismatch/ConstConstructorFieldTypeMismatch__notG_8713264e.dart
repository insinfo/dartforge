class A {
  const A(x) : y = x;
  final Unresolved y;
//      ^^^^^^^^^^
// [diag.undefinedClass] Undefined class 'Unresolved'.
}
var v = const A(null);
