void g(void Function() fun) {}

void f() {
  g(() => {1});
//        ^^^
// [diag.unnecessarySetLiteral] Braces unnecessarily wrap this expression in a set literal.
}
