class A {
  const A() : x = 'foo';
//            ^^^^^^^^^
// [diag.initializerForNonExistentField] 'x' isn't a field in the enclosing class.
}
A a = const A();
