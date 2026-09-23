void f(bool b) {
  // ignore:unused_local_variable
  late final int? x;
  if (b) x = 0;
  x = 1;
}
