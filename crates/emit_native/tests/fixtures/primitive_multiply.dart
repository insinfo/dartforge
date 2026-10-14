import 'dart:typed_data';

@pragma('vm:never-inline')
dynamic multiply(dynamic a, dynamic b) => a * b;

class Other {
  int calls = 0;
  String operator *(dynamic other) {
    calls++;
    return 'custom:$other:$calls';
  }
}

String describe(dynamic value) {
  if (value is double) {
    final bytes = ByteData(8)..setFloat64(0, value, Endian.big);
    return 'double:${bytes.getUint64(0, Endian.big).toRadixString(16)}';
  }
  return '${value.runtimeType}:$value';
}

void main() {
  final values = <num>[
    0, 1, -1, 0x3fffffffffffffff, 0x4000000000000000,
    0x7fffffffffffffff, -0x8000000000000000,
    9007199254740993, 0.0, -0.0, 0.5, -0.5,
    double.infinity, double.negativeInfinity, double.nan,
  ];
  for (final a in values) {
    for (final b in values) {
      print(describe(multiply(a, b)));
    }
  }
  final other = Other();
  print(multiply(other, 3));
  print(multiply(other, 4));
  print(multiply('x', 3));
  for (final bad in [null, Object()]) {
    try {
      multiply(bad, 2);
    } catch (error) {
      print(error.runtimeType);
    }
  }
}
