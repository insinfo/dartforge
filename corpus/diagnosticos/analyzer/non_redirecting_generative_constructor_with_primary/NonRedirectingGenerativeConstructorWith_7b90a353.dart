class C(int x) {
  C.named();
//^^^^^^^
// [diag.nonRedirectingGenerativeConstructorWithPrimary] Classes with primary constructors can't have non-redirecting generative constructors.
}
