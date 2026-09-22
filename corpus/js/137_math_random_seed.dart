// Random(seed): nextInt, nextDouble e nextBool com semente fixa (verificação experimental de igualdade VM×web).
import 'dart:math';

void main() {
  var r = Random(42);
  for (var i = 0; i < 5; i++) {
    print(r.nextInt(100));
  }
  for (var i = 0; i < 3; i++) {
    print(r.nextDouble().toStringAsFixed(6));
  }
  for (var i = 0; i < 5; i++) {
    print(r.nextBool());
  }
  var r2 = Random(42);
  print(r2.nextInt(100));
  print(r2.nextInt(1));
  print(r2.nextInt(1 << 30));
  print(r2.nextInt(4294967296));
}
