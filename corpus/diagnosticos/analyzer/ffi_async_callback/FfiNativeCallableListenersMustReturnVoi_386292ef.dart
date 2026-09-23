import 'dart:ffi';
int f(int i) => i * 2;
void g() {
  NativeCallable<Int32 Function(Double)>.isolateLocal(exceptionalReturn: 4, f);
//                                                                          ^
// [diag.mustBeASubtype] The type 'int Function(int)' must be a subtype of 'Int32 Function(Double)' for 'NativeCallable'.
}
