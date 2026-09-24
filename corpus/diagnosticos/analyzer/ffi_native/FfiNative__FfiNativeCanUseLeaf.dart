import 'dart:ffi';
@Native<Int8 Function(Int64)>(symbol: 'DoesntMatter', isLeaf:true)
external int doesntMatter(int x);
