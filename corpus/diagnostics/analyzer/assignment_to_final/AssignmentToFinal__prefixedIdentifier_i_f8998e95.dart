abstract class A {
  external int x;
}

void f(A a) {
  a.x = 0;
  a.x += 0;
  ++a.x;
  a.x++;
}
