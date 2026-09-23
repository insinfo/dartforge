import 'dart:ffi';

void main() => print(Native.addressOf('malloc'));
//                                    ^^^^^^^^
// [diag.argumentMustBeNative] Argument to 'Native.addressOf' must be annotated with @Native
