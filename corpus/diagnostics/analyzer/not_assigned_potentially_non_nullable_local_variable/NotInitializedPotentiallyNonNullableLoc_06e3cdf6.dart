void f() {
  int v;
  try {
    v = 0;
  } catch (_) {
    rethrow;
  }
  v;
}
