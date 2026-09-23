void g(Future<void> Function() fun) {}

void f() {
  g(() async => {1});
//              ^^^
// [diag.unnecessarySetLiteral] Braces unnecessarily wrap this expression in a set literal.
}
