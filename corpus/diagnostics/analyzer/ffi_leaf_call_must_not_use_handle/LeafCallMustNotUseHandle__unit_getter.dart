import 'dart:ffi';

@Native<Handle Function()>(symbol: 'foo', isLeaf:true)
external Object get foo;
//                  ^^^
// [diag.leafCallMustNotReturnHandle] FFI leaf call can't return a 'Handle'.
