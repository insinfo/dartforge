mixin class A {}
class B extends A {}
augment class B with A {}
//                   ^
// [diag.mixinsSuperClass] 'mixin class A' can't be used in both the 'extends' and 'with' clauses.
