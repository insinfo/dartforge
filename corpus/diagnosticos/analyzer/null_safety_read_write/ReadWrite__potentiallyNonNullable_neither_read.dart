void f<T>(bool b, T t) {
  T x;
  if (b) x = t;
  x; // 0
//^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'x' must be assigned before it can be used.
}
