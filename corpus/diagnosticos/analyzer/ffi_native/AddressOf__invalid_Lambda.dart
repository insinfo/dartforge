import 'dart:ffi';

void main() => print(Native.addressOf(() => 3));
//                                    ^^^^^^^
// [diag.argumentMustBeNative] Argument to 'Native.addressOf' must be annotated with @Native
