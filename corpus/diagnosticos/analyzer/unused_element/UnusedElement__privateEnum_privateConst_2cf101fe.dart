enum _E {
  v._foo();
  const _E._foo();
  const _E._bar();
//         ^^^^
// [diag.unusedElement] The declaration '_E._bar' isn't referenced.
}

void f() {
  _E.v;
}
