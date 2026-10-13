import 'dart:ffi';
@Native<Void Function(Handle)>(symbol: 'DoesntMatter', isLeaf:true)
external void doesntMatter(Object o);
//            ^^^^^^^^^^^^
// [diag.leafCallMustNotTakeHandle] FFI leaf call can't take arguments of type 'Handle'.
