abstract class A {
  static late final int x;

  void f() {
    x = 0;
    x += 0;
    ++x;
    x++;
  }
}
