void acceptFunOptBool(void funOptBool([bool b])) {}
class C {
  static void funBool(bool b) {}
}
main() {
  acceptFunOptBool(C.funBool);
//                 ^^^^^^^^^
// [diag.argumentTypeNotAssignable] The argument type 'void Function(bool)' can't be assigned to the parameter type 'void Function([bool])'.
}
