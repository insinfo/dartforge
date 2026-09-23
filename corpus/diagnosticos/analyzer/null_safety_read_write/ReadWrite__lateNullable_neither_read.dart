void f(bool b) {
  late int? x;
  if (b) x = 0;
  x; // 0
}
