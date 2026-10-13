class A {
  A() : this.noSuchConstructor();
//      ^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.redirectGenerativeToMissingConstructor] The constructor 'A.noSuchConstructor' couldn't be found in 'A'.
}
