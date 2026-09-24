import 'dart:ffi';
void main() {
  Pointer.fromFunction<Void Function()>(undefinedFn);
//                                      ^^^^^^^^^^^
// [diag.undefinedIdentifier] Undefined name 'undefinedFn'.
// [diag.mustBeASubtype] The type 'InvalidType' must be a subtype of 'Void Function()' for 'fromFunction'.
}
