void f(bool b) {
  late final x;
  if (b) x = 0;
  x; // 0
}
