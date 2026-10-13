void f<T>(T t, T t2) {
  // ignore:unused_local_variable
  late final T x;
  x = t;
  x = t2;
//^
// [diag.lateFinalLocalAlreadyAssigned] The late final local variable is already assigned.
}
