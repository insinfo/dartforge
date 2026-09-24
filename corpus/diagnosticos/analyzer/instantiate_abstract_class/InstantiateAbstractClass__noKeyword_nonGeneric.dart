abstract class A {}
void f() {
  A();
//^
// [diag.instantiateAbstractClass] Abstract classes can't be instantiated.
}
