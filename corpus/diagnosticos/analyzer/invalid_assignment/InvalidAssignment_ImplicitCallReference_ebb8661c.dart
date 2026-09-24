class C {
  void call(int a) {}
}

void Function(String) f = C();
//                        ^^^
// [diag.invalidAssignment] A value of type 'void Function(int)' can't be assigned to a variable of type 'void Function(String)'.
