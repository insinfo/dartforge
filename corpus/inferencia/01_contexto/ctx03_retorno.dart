// R-CTX-03: o `return`/`=>` recebe o tipo de retorno como contexto; em
// `async` o contexto é FutureOr<flatten(R)> (futureValueTypeSchema); em
// geradores, o `yield` recebe o tipo do elemento.
List<num> f() => /*@*/[1];
Future<List<num>> g() async => /*@*/[1];
Future<List<num>> h() async {
  return /*@*/[1];
}

Iterable<List<num>> s() sync* {
  yield /*@*/[1];
}

Stream<List<num>> t() async* {
  yield /*@*/[1];
}

Future<double> u() async => /*@*/1;
void main() {
  print([f(), g(), h(), s(), t(), u()]);
}
