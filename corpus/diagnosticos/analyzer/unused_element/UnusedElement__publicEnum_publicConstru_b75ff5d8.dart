enum E {
  v.foo();
  const E.foo();
  const E.bar();
//        ^^^
// [diag.unusedElement] The declaration 'E.bar' isn't referenced.
}
