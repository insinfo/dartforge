void f() {
  try {} catch (e) {
    () {
      rethrow;
//    ^^^^^^^
// [diag.rethrowOutsideCatch] A rethrow must be inside of a catch clause.
    };
  }
}
