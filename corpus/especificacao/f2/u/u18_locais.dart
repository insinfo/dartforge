void f(Object o, List<int> l) {
  var nunca = 0;
  var soEscrita = 0;
  var lida = 0;
  var inc = 0;
  var neg = 0;
  var naoNula = 0 as int?;
  var composta = 0;
  int? seNula;
  var emClosure = 0;
  var _ = 0;
  var __ = 0;
  late int tardia;
  soEscrita = 1;
  print(lida);
  inc++;
  -neg;
  naoNula!;
  composta += 1;
  seNula ??= 1;
  () { emClosure = 2; };
  tardia = 3;
  for (var i = 0; ;) {}
  for (var e in l) {}
  for (var j = 0, k = 0; j < 1; j++) {}
}
