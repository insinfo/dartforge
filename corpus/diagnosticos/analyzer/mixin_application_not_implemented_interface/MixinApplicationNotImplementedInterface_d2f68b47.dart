class A {}

mixin M on A {}

class X = Object with M;
//                    ^
// [diag.mixinApplicationNotImplementedInterface] 'M' can't be mixed onto 'Object' because 'Object' doesn't implement 'A'.
