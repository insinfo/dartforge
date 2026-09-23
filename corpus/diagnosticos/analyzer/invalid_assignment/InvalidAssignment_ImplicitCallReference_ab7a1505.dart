class C {
  T call<T>(T t) => t;
}

void Function() f = C();
//                  ^^^
// [diag.invalidAssignment] A value of type 'dynamic Function(dynamic)' can't be assigned to a variable of type 'void Function()'.
