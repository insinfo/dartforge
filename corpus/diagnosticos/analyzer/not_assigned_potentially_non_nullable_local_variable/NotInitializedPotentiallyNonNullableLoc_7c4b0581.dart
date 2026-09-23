void f() {
  int v;
  try {
    // not assigned
  } catch (_) {
    // not assigned
  } finally {
    v = 0;
  }
  v;
}
