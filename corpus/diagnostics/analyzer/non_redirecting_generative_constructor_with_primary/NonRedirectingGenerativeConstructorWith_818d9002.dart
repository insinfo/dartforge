enum E(int x) {
  v(0);
  const E.named();
//      ^^^^^^^
// [diag.nonRedirectingGenerativeConstructorWithPrimary] Classes with primary constructors can't have non-redirecting generative constructors.
//        ^^^^^
// [diag.unusedElement] The declaration 'E.named' isn't referenced.
}
