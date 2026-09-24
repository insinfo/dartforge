import 'dart:ffi';
@Native<Handle Function()>(isLeaf:true)
external Object doesntMatter();
//              ^^^^^^^^^^^^
// [diag.leafCallMustNotReturnHandle] FFI leaf call can't return a 'Handle'.
