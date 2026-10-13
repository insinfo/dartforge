class A {
  A();
  const A.c(Object o);
}
class G<T> {
  const G(Object o);
}
class C<@A() T, @A.c(T) U, @G<T>(1) V> {
  void m<@A.c(X) X>() {}
  @A.c(T)
  int f = 0;
}
void f(@A() int p, [@A.c(q) int q = 0]) {
  @A() var loc = 0;
}
