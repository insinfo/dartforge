abstract class A {
  static late final int x;
}

void f() {
  A.x = 0;
  A.x += 0;
  ++A.x;
  A.x++;
}
