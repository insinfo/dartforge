enum E {
  v._foo();
  const E._foo();
  const E._bar();
//        ^^^^
// [diag.unusedElement] The declaration 'E._bar' isn't referenced.
}
