// R-UP-02: UP com Null, tipos anuláveis, topo e fundo.
void main(List<String> args) {
  var b = args.isEmpty;
  int? n = b ? null : 1;
  dynamic d = 1;
  Object? o = 1;
  var p = /*@*/b ? 1 : null;
  var q = /*@*/b ? null : null;
  var r = /*@*/b ? 1 : n;
  var s = /*@*/b ? 1 : d;
  var t = /*@*/b ? o : d;
  var u = /*@*/b ? 1 : o;
  var v = /*@*/b ? 1 : throw 0;
  var w = /*@*/b ? 'a' : n;
  print([p, q, r, s, t, u, v, w]);
}
