// R-UP-06: UP de registros: campo a campo; formas diferentes dão Record.
void main(List<String> args) {
  var b = args.isEmpty;
  var p = /*@*/b ? (1, 'a') : (2.5, 'b');
  var q = /*@*/b ? (1, 'a') : (1, 'a', 2);
  var r = /*@*/b ? (x: 1) : (x: 'a');
  var s = /*@*/b ? (x: 1) : (y: 1);
  var t = /*@*/b ? (1, 2) : 3;
  print([p, q, r, s, t]);
}
