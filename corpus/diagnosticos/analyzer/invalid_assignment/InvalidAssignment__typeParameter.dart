class B<T> {
  T? value;
  void test(num n) {
    value = n;
//          ^
// [diag.invalidAssignment] A value of type 'num' can't be assigned to a variable of type 'T?'.
  }
}
