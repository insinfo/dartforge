class C(int x) {
  C.named1() : this.named2();
  C.named2();
//^^^^^^^^
// [diag.nonRedirectingGenerativeConstructorWithPrimary] Classes with primary constructors can't have non-redirecting generative constructors.
}
