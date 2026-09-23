// R-CLO-02: tipo de retorno inferido: UP das expressões retornadas; fim
// alcançável acrescenta Null; corpo que não retorna dá Never.
void main(List<String> args) {
  var b = args.isEmpty;
  var a = /*@*/() => 1;
  var c = /*@*/() {
    if (b) return 1;
    return 2.5;
  };
  var d = /*@*/() {};
  var e = /*@*/() {
    if (b) return 1;
  };
  var f = /*@*/() => throw 0;
  var g = /*@*/() {
    throw 0;
  };
  var h = /*@*/() {
    return;
  };
  print([a, c, d, e, f, g, h]);
}
