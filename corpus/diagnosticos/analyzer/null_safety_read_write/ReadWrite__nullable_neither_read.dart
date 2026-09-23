void f(bool b) {
  int? x;
  if (b) x = 0;
  x; // 0
}
