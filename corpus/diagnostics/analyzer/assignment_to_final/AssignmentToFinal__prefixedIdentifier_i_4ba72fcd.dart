class A {
  var x = 0;
}

void f(A a) {
  a.x = 0;
  a.x += 0;
  ++a.x;
  a.x++;
}
