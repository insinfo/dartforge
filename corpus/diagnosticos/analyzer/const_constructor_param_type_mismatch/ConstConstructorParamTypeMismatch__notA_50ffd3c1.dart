class A {
  final Unresolved x;
//      ^^^^^^^^^^
// [diag.undefinedClass] Undefined class 'Unresolved'.
  const A(String this.x);
}
var v = const A('foo');
