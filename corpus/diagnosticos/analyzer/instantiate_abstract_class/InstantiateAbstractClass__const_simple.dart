abstract class A {
  const A();
}
void f() {
  A a = const A();
//  ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
//            ^
// [diag.instantiateAbstractClass] Abstract classes can't be instantiated.
}