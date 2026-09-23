// R-LIT-05: literais de registro: tipo campo a campo, com o contexto de
// cada campo.
void main() {
  var a = /*@*/(1, 'a');
  var b = /*@*/(x: 1, y: 2.5);
  (double, num) c = /*@*/(/*@*/1, /*@*/2);
  (int, {String s}) d = /*@*/(1, s: 'x');
  var e = /*@*/(1, [2], n: null);
  print([/*@*/a.$1, /*@*/b.y, c, /*@*/d.s, e]);
}
