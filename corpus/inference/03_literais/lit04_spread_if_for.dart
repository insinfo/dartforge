// R-LIT-04: elementos de coleção: spread, spread nulo, if e for.
void main(List<String> args) {
  var l = [1];
  List<double>? n = args.isEmpty ? null : [];
  var a = /*@*/[...l, ...?n];
  var b = /*@*/[if (l.isEmpty) 1 else 2.5];
  var c = /*@*/[for (var i in l) i.toString()];
  var d = /*@*/{for (var i in l) i: i.isEven};
  var e = /*@*/[for (var i = 0; i < 3; i++) (/*@*/i)];
  var f = /*@*/[if (args.isEmpty) 'a'];
  var g = /*@*/{if (args.isEmpty) 1: 'a'};
  var h = /*@*/{...?null, 1};
  print([a, b, c, d, e, f, g, h]);
}
