class A<T extends num> {
  T _f;
  A._(this._f);
}
class B extends A<int> {
  B._(int f) : super._(f);
}
void main() {
  B b = B._(7);
  print(b._f == 7);
}
