class A {
  const A.b() : this.a();
//              ^^^^^^^^
// [diag.redirectGenerativeToMissingConstructor] The constructor 'A.a' couldn't be found in 'A'.
}
