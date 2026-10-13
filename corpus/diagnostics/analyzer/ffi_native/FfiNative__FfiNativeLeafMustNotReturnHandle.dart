import 'dart:ffi';
@Native<Handle Function()>(symbol: 'DoesntMatter', isLeaf:true)
external Object doesntMatter();
//              ^^^^^^^^^^^^
// [diag.leafCallMustNotReturnHandle] FFI leaf call can't return a 'Handle'.
