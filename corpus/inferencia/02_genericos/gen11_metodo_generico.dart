// R-GEN-11: métodos genéricos de instância e do SDK.
void main() {
  var l = [1, 2];
  var a = /*@*/l.map((x) => x * 2.5);
  var b = /*@*/l.expand((x) => [x, x.toString()]);
  var c = /*@*/l.whereType<num>();
  var d = /*@*/l.cast<Object>();
  var e = /*@*/Future.wait([Future.value(1), Future.value(2)]);
  var f = /*@*/l.reduce((a, b) => a + b);
  print([a, b, c, d, e, f]);
}
