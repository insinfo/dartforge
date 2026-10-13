void f() {
  int v;
  try {
    // not assigned
  } finally {
    v = 0;
  }
  v;
}
