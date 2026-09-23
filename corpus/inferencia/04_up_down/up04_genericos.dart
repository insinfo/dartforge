// R-UP-04: UP de instâncias da mesma classe genérica: argumento a argumento
// (covariante: UP).
void main(List<String> args) {
  var b = args.isEmpty;
  var p = /*@*/b ? <int>[] : <double>[];
  var q = /*@*/b ? <int>[] : <String>[];
  var r = /*@*/b ? <String, int>{} : <String, double>{};
  var s = /*@*/b ? <int>[] : <int>{};
  var t = /*@*/b ? <int>[] : <num>[];
  print([p, q, r, s, t]);
}
