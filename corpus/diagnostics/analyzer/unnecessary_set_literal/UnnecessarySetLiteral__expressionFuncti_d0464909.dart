void g(void Function() fun) {}

void f(bool b) {
  g(() => {1, if (b) 2 else 3, 4, for (;;) 5},);
//        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.unnecessarySetLiteral] Braces unnecessarily wrap this expression in a set literal.
}
