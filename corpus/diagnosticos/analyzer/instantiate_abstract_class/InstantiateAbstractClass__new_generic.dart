abstract class A<E> {}
void f() {
  new A<int>();
//    ^^^^^^
// [diag.instantiateAbstractClass] Abstract classes can't be instantiated.
}
