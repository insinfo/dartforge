void acceptFunOptBool(void funNumOptBool([bool b])) {}
void funBool(bool b) {}
main() {
  acceptFunOptBool(funBool);
//                 ^^^^^^^
// [diag.argumentTypeNotAssignable] The argument type 'void Function(bool)' can't be assigned to the parameter type 'void Function([bool])'.
}
