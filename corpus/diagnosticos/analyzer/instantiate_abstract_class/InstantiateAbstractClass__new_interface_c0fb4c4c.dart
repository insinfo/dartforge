abstract class A {}
typedef B = A;
void f() {
  new B();
//    ^
// [diag.instantiateAbstractClass] Abstract classes can't be instantiated.
}
