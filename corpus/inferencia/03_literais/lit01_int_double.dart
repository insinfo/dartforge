// R-LIT-01: literal inteiro recebe o tipo double quando o contexto aceita
// double e não aceita int.
import 'dart:async';

void main() {
  double a = /*@*/1;
  double? b = /*@*/2;
  num c = /*@*/3;
  Object d = /*@*/4;
  FutureOr<double> e = /*@*/5;
  var f = /*@*/0x10;
  double g = /*@*/0x10;
  dynamic h = /*@*/6;
  double i = /*@*/-7;
  print([a, b, c, d, e, f, g, h, i]);
}
