void f() {
  try {} catch (e1) {
    () {
      try {} catch (e2) {
        rethrow;
      }
    };
  }
}
