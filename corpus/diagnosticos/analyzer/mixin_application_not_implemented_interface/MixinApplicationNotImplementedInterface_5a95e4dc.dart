class A {}
class B {}
class C {}

mixin M on A, B {}

class X = C with M;
//               ^
// [diag.mixinApplicationNotImplementedInterface] 'M' can't be mixed onto 'C' because 'C' doesn't implement 'A'.
