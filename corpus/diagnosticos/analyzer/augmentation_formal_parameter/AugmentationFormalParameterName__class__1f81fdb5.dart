class A {
  final int p1;
  A(this.p1);
//       ^^
// [context 1] The preceding declaration is here.
  augment A(int p2);
//              ^^
// [diag.augmentationPositionalFormalParameterName][context 1] The parameter name 'p2' must either match the name 'p1' from a preceding declaration or be '_'.
}
