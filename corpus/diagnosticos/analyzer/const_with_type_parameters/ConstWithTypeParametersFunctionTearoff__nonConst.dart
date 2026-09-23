void f<T>(T a) {}
class A<U> {
  void m() {
    f<U>;
  }
}
