class A {}
int B = 7;
class C = A with B;
//               ^
// [diag.mixinOfNonClass] Classes can only mix in mixins and classes.
