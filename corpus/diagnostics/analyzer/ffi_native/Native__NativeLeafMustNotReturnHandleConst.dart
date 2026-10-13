import 'dart:ffi';
const annotation = Native<Handle Function()>(isLeaf:true);

@annotation
external Object doesntMatter();
//              ^^^^^^^^^^^^
// [diag.leafCallMustNotReturnHandle] FFI leaf call can't return a 'Handle'.
