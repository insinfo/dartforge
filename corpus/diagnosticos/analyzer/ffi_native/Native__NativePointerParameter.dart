import 'dart:ffi';
@Native<Void Function(Pointer)>()
external void free(Pointer pointer);
