class C<T> {
  f() {
    T = null;
//  ^
// [diag.assignmentToType] Types can't be assigned a value.
  }
}
