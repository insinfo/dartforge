import 'dart:typed_data';

double chooseDouble(double value, int mode) {
  if (mode == 0) return value;
  if (mode == 1) return -value;
  return value * 2.0;
}

String bits(double value) {
  final bytes = ByteData(8)..setFloat64(0, value, Endian.big);
  return bytes.getUint64(0, Endian.big).toRadixString(16);
}

void main() {
  print(bits(chooseDouble(0.0, 0)));
  print(bits(chooseDouble(-0.0, 0)));
  print(bits(chooseDouble(0.0, 1)));
  print(bits(chooseDouble(-0.0, 1)));
  print(bits(chooseDouble(double.nan, 0)));
  print(bits(chooseDouble(double.infinity, 0)));
  print(bits(chooseDouble(double.negativeInfinity, 0)));
  print(bits(chooseDouble(0.5, 2)));
  final generic = chooseDouble;
  print(bits(generic(-0.0, 0)));
}
