import 'dart:ffi';
@Native<Handle Function(Uint32, Uint32, Handle)>()
external void voidReturn(int width, int height, Object outImage);
//            ^^^^^^^^^^
// [diag.mustBeASubtype] The type 'Handle Function(Uint32, Uint32, Handle)' must be a subtype of 'void Function(int, int, Object)' for 'Native'.
