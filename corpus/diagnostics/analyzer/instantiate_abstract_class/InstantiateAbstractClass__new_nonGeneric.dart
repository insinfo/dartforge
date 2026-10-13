abstract class A {}
void f() {
  new A();
//    ^
// [diag.instantiateAbstractClass] Abstract classes can't be instantiated.
}
