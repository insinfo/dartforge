import 'dart:ffi';
typedef NativeTakesHandle = Void Function(Handle);
typedef TakesHandle = void Function(Object);
class MyClass {}
doThings() {
  DynamicLibrary l = DynamicLibrary.open("my_lib");
  l.lookupFunction<NativeTakesHandle, TakesHandle>("timesFour", isLeaf:true);
//                 ^^^^^^^^^^^^^^^^^
// [diag.leafCallMustNotTakeHandle] FFI leaf call can't take arguments of type 'Handle'.
}
