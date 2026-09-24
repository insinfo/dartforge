import 'dart:ffi';
typedef NativeReturnsHandle = Handle Function();
typedef ReturnsHandle = Object Function();
doThings() {
  DynamicLibrary l = DynamicLibrary.open("my_lib");
  l.lookupFunction<NativeReturnsHandle, ReturnsHandle>("timesFour", isLeaf:true);
//                 ^^^^^^^^^^^^^^^^^^^
// [diag.leafCallMustNotReturnHandle] FFI leaf call can't return a 'Handle'.
}
