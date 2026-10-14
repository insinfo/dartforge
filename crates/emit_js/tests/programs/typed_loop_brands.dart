import 'dart:typed_data';

String bits(double value) {
  final bytes = ByteData(8)..setFloat64(0, value);
  return bytes.getUint32(0).toRadixString(16).padLeft(8, '0') + bytes.getUint32(4).toRadixString(16).padLeft(8, '0');
}

String mixed(Float32List a, Int16List b) {
  var sum = -0.0;
  try {
    for (var i = 0; i < a.length; i++) {
      sum += a[i] * b[i];
    }
    return bits(sum);
  } on RangeError catch (error) {
    return '${bits(sum)}:range=${error.invalidValue}';
  }
}

int sum0(Int8List a) {
  var sum = 0;
  for (var i = 0; i < a.length; i++) {
    sum += a[i];
  }
  return sum;
}

int sum1(Uint8ClampedList a) {
  var sum = 0;
  for (var i = 0; i < a.length; i++) {
    sum += a[i];
  }
  return sum;
}

int sum2(Int16List a) {
  var sum = 0;
  for (var i = 0; i < a.length; i++) {
    sum += a[i];
  }
  return sum;
}

int sum3(Uint16List a) {
  var sum = 0;
  for (var i = 0; i < a.length; i++) {
    sum += a[i];
  }
  return sum;
}

int sum4(Int32List a) {
  var sum = 0;
  for (var i = 0; i < a.length; i++) {
    sum += a[i];
  }
  return sum;
}

int sum5(Uint32List a) {
  var sum = 0;
  for (var i = 0; i < a.length; i++) {
    sum += a[i];
  }
  return sum;
}

String mutate(Float32List a, Float32List b) {
  var sum = 0.0;
  for (var i = 0; i < a.length; i++) {
    sum += a[i];
    a = b;
  }
  return bits(sum);
}

String offset(Int32List a) {
  var sum = 0;
  try {
    for (var i = 0; i < a.length; i++) {
      sum += a[i + 1];
    }
    return '$sum';
  } on RangeError catch (error) {
    return '$sum:range=${error.invalidValue}';
  }
}

void main() {
  final a = Float32List.fromList([0.1, 0.2, 0.3]);
  final b = Int16List.fromList([-7, 8, 9]);
  print('mixed=${mixed(a, b)}');
  print('readonly=${mixed(a.asUnmodifiableView(), b.asUnmodifiableView())}');
  print('view=${mixed(Float32List.view(a.buffer, 4, 2), Int16List.view(b.buffer, 2, 2))}');
  print('short=${mixed(a, Int16List.fromList([7]))}');
  print('empty=${mixed(Float32List(0), Int16List(0))}');
  print('negativeZero=${mixed(Float32List.fromList([-0.0]), Int16List.fromList([1]))}');
  print('infinity=${mixed(Float32List.fromList([double.infinity]), Int16List.fromList([1]))}');
  print('mutate=${mutate(a, Float32List.fromList([9.0, 8.0]))}');
  print('offset=${offset(Int32List.fromList([1, -2, 3]))}');
  final t0 = Int8List.fromList([-129, -128, -1, 0, 127, 128, 255]);
  print('Int8List=${sum0(t0)}:readonly=${sum0(t0.asUnmodifiableView())}:view=${sum0(Int8List.view(t0.buffer, t0.elementSizeInBytes, t0.length - 1))}:empty=${sum0(Int8List(0))}');
  final t1 = Uint8ClampedList.fromList([-1, 0, 127, 255, 256]);
  print('Uint8ClampedList=${sum1(t1)}:readonly=${sum1(t1.asUnmodifiableView())}:view=${sum1(Uint8ClampedList.view(t1.buffer, t1.elementSizeInBytes, t1.length - 1))}:empty=${sum1(Uint8ClampedList(0))}');
  final t2 = Int16List.fromList([-32769, -32768, -1, 0, 32767, 32768]);
  print('Int16List=${sum2(t2)}:readonly=${sum2(t2.asUnmodifiableView())}:view=${sum2(Int16List.view(t2.buffer, t2.elementSizeInBytes, t2.length - 1))}:empty=${sum2(Int16List(0))}');
  final t3 = Uint16List.fromList([-1, 0, 32768, 65535, 65536]);
  print('Uint16List=${sum3(t3)}:readonly=${sum3(t3.asUnmodifiableView())}:view=${sum3(Uint16List.view(t3.buffer, t3.elementSizeInBytes, t3.length - 1))}:empty=${sum3(Uint16List(0))}');
  final t4 = Int32List.fromList([-2147483648, -1, 0, 2147483647]);
  print('Int32List=${sum4(t4)}:readonly=${sum4(t4.asUnmodifiableView())}:view=${sum4(Int32List.view(t4.buffer, t4.elementSizeInBytes, t4.length - 1))}:empty=${sum4(Int32List(0))}');
  final t5 = Uint32List.fromList([0, 2147483648, 4294967295]);
  print('Uint32List=${sum5(t5)}:readonly=${sum5(t5.asUnmodifiableView())}:view=${sum5(Uint32List.view(t5.buffer, t5.elementSizeInBytes, t5.length - 1))}:empty=${sum5(Uint32List(0))}');
}
