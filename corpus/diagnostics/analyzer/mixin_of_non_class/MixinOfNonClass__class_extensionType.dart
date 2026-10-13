extension type A(int it) {}
class B with A {}
//           ^
// [diag.mixinOfNonClass] Classes can only mix in mixins and classes.
