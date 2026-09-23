import 'dart:ffi';
void g() {
  NativeCallable<Int32 Function(Int32)>.isolateLocal(exceptionalReturn: 0);
//                                                   ^^^^^^^^^^^^^^^^^
// [diag.notEnoughPositionalArgumentsNameSingular] 1 positional argument expected by 'isolateLocal', but 0 found.
}
