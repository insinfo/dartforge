// R-CLO-03: async: Future<flatten(UP dos retornos)>.
void main() {
  var a = /*@*/() async => 1;
  var b = /*@*/() async {};
  var c = /*@*/() async => Future.value(1);
  Future<num> Function() d = /*@*/() async => 1;
  var e = /*@*/() async {
    return;
  };
  var f = /*@*/() async => null;
  print([a, b, c, d, e, f]);
}
