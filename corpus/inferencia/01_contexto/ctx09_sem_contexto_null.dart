// R-CTX-09: `var x = null` e `var x;` — o tipo declarado inferido.
var t = null;
void main() {
  var a = null;
  var b;
  print([/*@*/t, /*@*/a, /*@*/b]);
}
