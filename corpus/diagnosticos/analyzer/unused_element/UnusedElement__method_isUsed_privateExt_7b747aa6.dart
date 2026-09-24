class A<T> {}
extension _A<T> on A<T> {
  T operator ~() => throw 0;
}
void f(A<int> a) {
  ~a;
}
