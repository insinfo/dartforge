// R-CTX-07: `await e` com contexto K dá a `e` o contexto FutureOr<K>.
Future<void> main() async {
  List<num> a = await /*@*/Future.value([1]);
  var b = /*@*/await Future.value(1);
  double c = await /*@*/Future.value(1);
  print([a, b, c]);
}
