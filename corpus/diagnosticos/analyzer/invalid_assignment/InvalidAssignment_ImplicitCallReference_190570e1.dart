// %before-language-feature: constructor-tearoffs
class C {
  T call<T>(T t) => t;
}

int Function(int) f = C();
//                    ^^^
// [diag.invalidAssignment] A value of type 'T Function<T>(T)' can't be assigned to a variable of type 'int Function(int)'.
