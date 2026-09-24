enum E {
  v;
  const E() : this.noSuchConstructor();
//            ^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.redirectGenerativeToMissingConstructor] The constructor 'E.noSuchConstructor' couldn't be found in 'E'.
}
