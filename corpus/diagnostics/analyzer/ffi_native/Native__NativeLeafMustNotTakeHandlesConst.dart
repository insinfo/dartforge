import 'dart:ffi';
const annotation = Native<Void Function(Handle)>(symbol: 'DoesntMatter', isLeaf:true);

@annotation
external void doesntMatter(Object o);
//            ^^^^^^^^^^^^
// [diag.leafCallMustNotTakeHandle] FFI leaf call can't take arguments of type 'Handle'.
