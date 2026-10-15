import 'dart:typed_data';

@pragma('vm:never-inline')
dynamic unitClamp(dynamic value) => value.clamp(0, 1);

@pragma('vm:never-inline')
num typedUnitClamp(num value) => value.clamp(0, 1);

@pragma('vm:never-inline')
dynamic genericClamp(dynamic value, dynamic lower, dynamic upper) =>
    value.clamp(lower, upper);

class Other {
  int calls = 0;
  String clamp(dynamic lower, dynamic upper) {
    calls++;
    return 'custom:$lower:$upper:$calls';
  }
}

double fromBits(int bits) {
  final data = ByteData(8)..setInt64(0, bits, Endian.big);
  return data.getFloat64(0, Endian.big);
}

String describe(dynamic value) {
  if (value is double) {
    final data = ByteData(8)..setFloat64(0, value, Endian.big);
    return 'double:${data.getInt64(0, Endian.big).toRadixString(16)}';
  }
  if (value is int) return 'int:$value';
  return '$value';
}

String failure(Object error) {
  if (error is ArgumentError) return 'ArgumentError';
  if (error is TypeError) return 'TypeError';
  if (error is NoSuchMethodError) return 'NoSuchMethodError';
  return '${error.runtimeType}';
}

int evaluations = 0;
dynamic evaluated(dynamic value) {
  evaluations++;
  return value;
}

void main() {
  final values = <num>[
    -0x8000000000000000, -9007199254740993, -1, 0, 1, 2,
    9007199254740993, 0x7fffffffffffffff,
    double.negativeInfinity, -1.0, -0.5, -0.0, 0.0, 0.5, 1.0,
    double.infinity, double.nan,
    fromBits(1), fromBits(-0x7fffffffffffffff),
    fromBits(0x3fefffffffffffff), fromBits(0x3ff0000000000001),
    fromBits(0x7ff8000000000042), fromBits(-0x7ffffffffffbe),
  ];
  for (final value in values) {
    final result = unitClamp(evaluated(value));
    print('${describe(result)}:${identical(result, value)}:'
        '${identical(result, 0)}:${identical(result, 1)}');
    print(describe(typedUnitClamp(value)));
  }
  print('evaluations:$evaluations');
  final ranges = <List<dynamic>>[
    [0, 1], [0.0, 1.0], [-0.0, 0.0], [0, -0.0],
    [double.nan, double.nan], [double.nan, 1], [0, double.nan],
    [1, 0], [null, 1], [0, null], ['x', 1], [0, 'x'],
    [9007199254740992, 9007199254740993],
    [-0x8000000000000000, 0x7fffffffffffffff],
  ];
  for (final range in ranges) {
    for (final value in values) {
      try {
        final result = genericClamp(value, range[0], range[1]);
        print('${describe(result)}:${identical(result, value)}:'
            '${identical(result, range[0])}:${identical(result, range[1])}');
      } catch (error) {
        print(failure(error));
      }
    }
  }
  final other = Other();
  print(unitClamp(other));
  print(unitClamp(other));
  for (final value in [null, Object()]) {
    try {
      unitClamp(value);
    } catch (error) {
      print(failure(error));
    }
  }
}
