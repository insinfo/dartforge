class A<T> {}
extension _A<T> on A<T> {
  void operator []=(int index, T value) {
}}
void f(A<int> a) {
  a[0] = 1;
}
