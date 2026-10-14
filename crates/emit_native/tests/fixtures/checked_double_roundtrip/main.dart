import 'dart:typed_data';
@pragma('vm:never-inline')
Object roundTrip(dynamic value) {
  final double scalar = value;
  return scalar;
}
String bits(double value) {
  final data = ByteData(8)..setFloat64(0, value, Endian.little);
  return data.buffer.asUint8List().join(',');
}
void main() {
  final data = ByteData(8)..setUint64(0, 0x7ff8000000000042, Endian.little);
  for (final value in <double>[0.0, -0.0, 1.25, double.infinity,
      double.negativeInfinity, double.nan, data.getFloat64(0, Endian.little)]) {
    final result = roundTrip(value) as double;
    print('${bits(result)}:${identical(value, result)}:${identityHashCode(value) == identityHashCode(result)}');
  }
  for (final value in <dynamic>[null, '1', true, Object()]) {
    try { roundTrip(value); print('unexpected'); }
    on TypeError { print('TypeError'); }
  }
}
