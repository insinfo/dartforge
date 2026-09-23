// R-DOWN-01: DOWN (greatest lower bound) aparece nos parâmetros de UP de
// funções e nos limites superiores da inferência.
void a(int x) {}
void b(num x) {}
void c(String x) {}
void d({int? x}) {}
void e({String? y}) {}
void f((int, num) r) {}
void g((num, int) r) {}
void main(List<String> args) {
  var k = args.isEmpty;
  var p = /*@*/k ? a : b;
  var q = /*@*/k ? a : c;
  var r = /*@*/k ? d : e;
  var s = /*@*/k ? f : g;
  print([p, q, r, s]);
}
