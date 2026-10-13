// R-LIT-06: outros literais: string (com interpolação), símbolo, null, bool.
void main() {
  var a = /*@*/'a${1}b';
  var b = /*@*/'a' 'b';
  var c = /*@*/#simbolo;
  var d = /*@*/true;
  var e = /*@*/1.5;
  print([a, b, c, d, e, /*@*/null]);
}
