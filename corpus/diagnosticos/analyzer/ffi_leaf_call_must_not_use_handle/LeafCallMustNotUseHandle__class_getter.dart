import 'dart:ffi';

base class NativeFieldWrapperClass1 {}

base class A extends NativeFieldWrapperClass1 {
  @Native<Handle Function(Pointer<Void>)>(symbol: 'foo', isLeaf:true)
  external Object get foo;
//                    ^^^
// [diag.leafCallMustNotReturnHandle] FFI leaf call can't return a 'Handle'.
}
