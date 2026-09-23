import 'dart:ffi';
typedef NativeReturnsHandle = Handle Function();
typedef ReturnsHandle = Object Function();
doThings() {
  Pointer<NativeFunction<NativeReturnsHandle>> p = Pointer.fromAddress(1337);
  ReturnsHandle f = p.asFunction(isLeaf:true);
//                    ^^^^^^^^^^
// [diag.leafCallMustNotReturnHandle] FFI leaf call can't return a 'Handle'.
  f();
}
