import 'dart:ffi';

@Native()
@Array(12)
external Array<IntPtr> field0;

@Array(10, 20)
@Native()
external Array<Array<IntPtr>> field1;
