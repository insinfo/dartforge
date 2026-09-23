void f() {
  int v;
  try {
    v = 0;
  } finally {
    // not assigned
  }
  v;
}
