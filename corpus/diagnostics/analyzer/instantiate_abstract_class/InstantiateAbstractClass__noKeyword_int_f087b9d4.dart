abstract class A {}
typedef B = A;
void f() {
  B();
//^
// [diag.instantiateAbstractClass] Abstract classes can't be instantiated.
}
