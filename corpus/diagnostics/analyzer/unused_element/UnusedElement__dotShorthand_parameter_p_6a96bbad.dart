class A<T> {
  A(T a);
}
void main() {
  A<int> a;
  a = .new(0);
  print(a);
}
