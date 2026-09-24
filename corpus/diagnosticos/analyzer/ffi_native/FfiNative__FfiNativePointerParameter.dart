import 'dart:ffi';
@Native<Void Function(Pointer)>(symbol: 'free')
external void posixFree(Pointer pointer);
