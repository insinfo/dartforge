void f(bool b) {
  late final int? x;
  if (b) x = 0;
  x; // 0
}
