mixin class A {}
class B extends A with A {}
//                     ^
// [diag.mixinsSuperClass] 'mixin class A' can't be used in both the 'extends' and 'with' clauses.
