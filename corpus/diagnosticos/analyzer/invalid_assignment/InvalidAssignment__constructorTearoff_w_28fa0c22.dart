class C<T> {
  C(T a);
}

C Function(String) g = C<int>.new;
//                     ^^^^^^^^^^
// [diag.invalidAssignment] A value of type 'C<int> Function(int)' can't be assigned to a variable of type 'C<dynamic> Function(String)'.
