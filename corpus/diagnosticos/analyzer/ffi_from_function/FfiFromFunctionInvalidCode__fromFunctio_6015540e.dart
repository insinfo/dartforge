import 'dart:ffi';
int f(int x) => x;
void main() {
  Pointer.fromFunction<Int32 Function(Int32)>(f, undefinedConst);
//                                               ^^^^^^^^^^^^^^
// [diag.undefinedIdentifier] Undefined name 'undefinedConst'.
// [diag.mustBeASubtype] The type 'InvalidType' must be a subtype of 'Int32' for 'fromFunction'.
// [diag.argumentMustBeAConstant] Argument 'exceptionalReturn' must be a constant.
}
