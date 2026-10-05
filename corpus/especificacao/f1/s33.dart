void f() {
  break L;
  L: while (true) {
    () { break L; };
    continue M;
  }
  x: 0;
}
