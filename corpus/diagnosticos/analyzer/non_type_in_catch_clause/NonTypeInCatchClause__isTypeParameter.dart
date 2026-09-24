class A<T extends Object> {
  f() {
    try {
    } on T catch (e) {
      e;
    }
  }
}
