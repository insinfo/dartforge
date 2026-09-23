import 'dart:ffi';
int f(int i) => i * 2;
void g() {
  NativeCallable<Int32 Function(Int32)>.isolateLocal(f, exceptionalReturn: '?');
//                                                                         ^^^
// [diag.mustBeASubtype] The type 'String' must be a subtype of 'Int32' for 'isolateLocal'.
}
