class C {
  T call<T extends num>(T t) => t;
}

String Function(String) f = C();
//                          ^^^
// [diag.invalidAssignment] A value of type 'num Function(num)' can't be assigned to a variable of type 'String Function(String)'.
