import 'dart:collection';
import 'dart:typed_data';

String number(double value) {
  if (value.isNaN) return 'nan';
  if (value.isInfinite) return value.isNegative ? '-infinity' : 'infinity';
  if (value == 0.0) return value.isNegative ? '-zero' : 'zero';
  return value.toStringAsFixed(3);
}

String bits(double value) {
  final data = ByteData(8)..setFloat64(0, value);
  return data.getUint32(0).toRadixString(16).padLeft(8, '0') + data.getUint32(4).toRadixString(16).padLeft(8, '0');
}

String precision(Float64List a, Float64List b) {
  var sum = 0.0;
  for (var i = 0; i < a.length; i++) {
    sum += a[i] * b[i];
  }
  return bits(sum);
}

String product(Float64List a, Float64List b, double seed) {
  var sum = seed;
  try {
    for (var i = 0; i < a.length; i++) {
      sum += a[i] * b[i];
    }
    return number(sum);
  } on RangeError catch (error) {
    return '${number(sum)}:range=${error.invalidValue}';
  }
}

String offset(Float64List a) {
  var sum = 0.0;
  try {
    for (var i = 0; i < a.length; i++) {
      sum += a[i + 1];
    }
    return number(sum);
  } on RangeError catch (error) {
    return '${number(sum)}:range=${error.invalidValue}';
  }
}

String changeReceiver(Float64List a, Float64List b) {
  var sum = 0.0;
  for (var i = 0; i < a.length; i++) {
    sum += a[i];
    a = b;
  }
  return number(sum);
}

String changeIndex(Float64List a) {
  var sum = 0.0;
  for (var i = 0; i < a.length; i++) {
    sum += a[i];
    if (i == 0) i = 1;
  }
  return number(sum);
}

int callbackCount = 0;
String callback(Float64List a) {
  var sum = 0.0;
  void mutate() {
    callbackCount++;
    a[1] = 7.0;
  }
  for (var i = 0; i < a.length; i++) {
    sum += a[i];
    mutate();
  }
  return '${number(sum)}:callbacks=$callbackCount';
}

int lengthReads = 0;
class CustomList extends ListBase<double> {
  int get length {
    lengthReads++;
    return 2;
  }
  set length(int value) => throw UnsupportedError('fixed');
  double operator [](int index) => index == 0 ? 2.0 : 3.0;
  void operator []=(int index, double value) => throw UnsupportedError('fixed');
}

String generic(List<double> a) {
  var sum = 0.0;
  for (var i = 0; i < a.length; i++) {
    sum += a[i];
  }
  return '${number(sum)}:lengthReads=$lengthReads';
}

String nullableReceiver(Float64List? a) {
  var sum = 0.0;
  for (var i = 0; i < a!.length; i++) {
    sum += a[i];
  }
  return number(sum);
}

String lateSeed(Float64List a) {
  late double sum = (() { a[0] = 9.0; return 0.0; })();
  for (var i = 0; i < a.length; i++) {
    sum += a[i];
  }
  return number(sum);
}

String shadowBuiltins(Float64List a) {
  var ArrayBuffer = 1.0;
  var Float64Array = 2.0;
  var Object = 3.0;
  var sum = 0.0;
  for (var i = 0; i < a.length; i++) {
    sum += a[i] + ArrayBuffer + Float64Array + Object;
  }
  return number(sum);
}

int bytes(Uint8List a) {
  var sum = 0;
  for (var i = 0; i < a.length; i++) {
    sum += a[i];
  }
  return sum;
}

void main() {
  final a = Float64List.fromList([1.0, 2.0, 3.0]);
  final b = Float64List.fromList([4.0, 5.0, 6.0]);
  print('equal=${product(a, b, 0.0)}');
  print('alias=${product(a, a, 0.0)}');
  print('longer=${product(a, Float64List.fromList([4.0, 5.0, 6.0, 99.0]), 0.0)}');
  print('short=${product(a, Float64List.fromList([4.0, 5.0]), 0.0)}');
  print('empty=${product(Float64List(0), Float64List(0), -0.0)}');
  print('zero=${product(Float64List.fromList([-0.0]), Float64List.fromList([1.0]), -0.0)}');
  print('nan=${product(Float64List.fromList([double.nan]), Float64List.fromList([1.0]), 0.0)}');
  print('infinity=${product(Float64List.fromList([double.infinity]), Float64List.fromList([1.0]), 0.0)}');
  print('precision=${product(Float64List.fromList([0.1, 0.2, 0.3]), Float64List.fromList([0.7, 0.8, 0.9]), 0.0)}');
  print('precisionBits=${precision(Float64List.fromList([0.1, 0.2, 0.3]), Float64List.fromList([0.7, 0.8, 0.9]))}');
  print('readonly=${product(a.asUnmodifiableView(), b.asUnmodifiableView(), 0.0)}');
  print('view=${product(Float64List.view(a.buffer, 8, 2), Float64List.view(b.buffer, 8, 2), 0.0)}');
  print('offset=${offset(a)}');
  print('receiver=${changeReceiver(a, Float64List.fromList([8.0, 9.0]))}');
  print('index=${changeIndex(a)}');
  print('callback=${callback(Float64List.fromList([1.0, 2.0, 3.0]))}');
  print('generic=${generic(CustomList())}');
  print('nullable=${nullableReceiver(a)}');
  print('late=${lateSeed(Float64List.fromList([1.0, 2.0]))}');
  print('shadow=${shadowBuiltins(a)}');
  final u = Uint8List.fromList([0, 127, 128, 255]);
  print('bytes=${bytes(u)}:readonly=${bytes(u.asUnmodifiableView())}:empty=${bytes(Uint8List(0))}');
}
