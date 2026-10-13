abstract class A<E> {}
void f() {
  A<int>();
//^^^^^^
// [diag.instantiateAbstractClass] Abstract classes can't be instantiated.
}
