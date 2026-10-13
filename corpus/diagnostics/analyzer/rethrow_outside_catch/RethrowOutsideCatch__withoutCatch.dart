void f() {
  rethrow;
//^^^^^^^
// [diag.rethrowOutsideCatch] A rethrow must be inside of a catch clause.
}
