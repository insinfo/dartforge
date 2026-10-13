// R-UP-07: UP com FutureOr.
import 'dart:async';

void main(List<String> args) {
  var b = args.isEmpty;
  FutureOr<int> fo = 1;
  var p = /*@*/b ? fo : 1;
  var q = /*@*/b ? fo : Future.value(1);
  var r = /*@*/b ? Future.value(1) : 1;
  var s = /*@*/b ? fo : 2.5;
  print([p, q, r, s]);
}
