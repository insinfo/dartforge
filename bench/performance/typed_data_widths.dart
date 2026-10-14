import 'dart:typed_data';

double mixed(Float32List a, Int16List b, int reps) {
  var sum = 0.0;
  for (var r = 0; r < reps; r++) {
    for (var i = 0; i < a.length; i++) {
      sum += a[i] * b[i];
    }
  }
  return sum;
}

int signed(Int32List a, int reps) {
  var sum = 0;
  for (var r = 0; r < reps; r++) {
    for (var i = 0; i < a.length; i++) {
      sum += a[i];
    }
  }
  return sum;
}

void measure(String name, Object Function() body) {
  final samples = <int>[];
  Object result = 0;
  for (var i = 0; i < 6; i++) {
    final watch = Stopwatch()..start();
    result = body();
    samples.add(watch.elapsedMicroseconds);
  }
  print('$name: ${samples.join(' ')} us | $result');
}

void main() {
  final a = Float32List(100000), b = Int16List(100000), c = Int32List(100000);
  for (var i = 0; i < a.length; i++) {
    a[i] = i * 0.001;
    b[i] = (i % 13) - 6;
    c[i] = (i % 200001) - 100000;
  }
  measure('mixed_f32_i16', () => mixed(a, b, 100));
  measure('sum_i32', () => signed(c, 100));
}
