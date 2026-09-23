void f<T>() {
  T x;
  x; // 0
//^
// [diag.notAssignedPotentiallyNonNullableLocalVariable] The non-nullable local variable 'x' must be assigned before it can be used.
}
