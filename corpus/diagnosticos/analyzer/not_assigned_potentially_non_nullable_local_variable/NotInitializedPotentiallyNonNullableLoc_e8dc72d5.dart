f() {
  int x;
  try {
    g(); // may throw an exception
    x = 1;
  } catch (_) {
    x = 1;
  } finally {
    x; // BAD
//  ^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'x' must be assigned before it can be used.
  }
}

void g() {}
