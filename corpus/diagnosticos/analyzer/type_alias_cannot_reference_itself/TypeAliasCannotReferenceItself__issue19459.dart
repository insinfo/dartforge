class A<B, C> {}
abstract class D {
  f(E e);
}
abstract class E extends A<dynamic, F> {}
typedef D F();
