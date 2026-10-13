class C<T> {
  C(T a);
  void call<U>(T t, U u) {}
}

void Function(bool, String) f = C(7);
//                              ^^^^
// [diag.invalidAssignment] A value of type 'void Function(int, dynamic)' can't be assigned to a variable of type 'void Function(bool, String)'.
