mixin class A {}
class B with A {}
//           ^
// [diag.mixinsSuperClass] 'mixin class A' can't be used in both the 'extends' and 'with' clauses.
augment class B extends A {}
