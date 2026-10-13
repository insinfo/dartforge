import 'dart:ffi';

@Native<IntPtr>()
external int get foo;

@Native<IntPtr>()
external set foo(int value);
