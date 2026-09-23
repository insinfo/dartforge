import 'dart:ffi';
@Native<Int8 Function(Int64, VarArgs<(Int32, Double)>)>()
external int doesntMatter(int x, int y, double z);
