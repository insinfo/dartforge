abstract class A<E> {
  const A();
}
void f() {
  var a = const A<int>();
//    ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
//              ^^^^^^
// [diag.instantiateAbstractClass] Abstract classes can't be instantiated.
}