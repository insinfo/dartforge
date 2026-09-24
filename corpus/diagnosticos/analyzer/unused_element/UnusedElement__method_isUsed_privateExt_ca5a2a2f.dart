class A<T> {}
extension _A<T> on A<T> {
  A<T> foo() => throw 0;
}
void f(A<int> a) {
  a.foo();
}
