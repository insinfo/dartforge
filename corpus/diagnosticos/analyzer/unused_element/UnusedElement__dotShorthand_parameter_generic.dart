class _A<T> {
  _A(T a);
}
void main() {
  _A<int> a;
  a = .new(0);
  print(a);
}
