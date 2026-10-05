void f(int x, Object o) {
  if (x case int() || 0) {
    print(1);
  }
  if (o case int() || String() || int()) {
    print(2);
  }
  if (x case _ || 1 || 2) {
    print(3);
  } else {
    print(4);
  }
}
