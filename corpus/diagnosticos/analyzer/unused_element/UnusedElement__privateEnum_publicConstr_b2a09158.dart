enum _E {
  v.foo();
  const _E.foo();
  const _E.bar();
//         ^^^
// [diag.unusedElement] The declaration '_E.bar' isn't referenced.
}

void f() {
  _E.v;
}
