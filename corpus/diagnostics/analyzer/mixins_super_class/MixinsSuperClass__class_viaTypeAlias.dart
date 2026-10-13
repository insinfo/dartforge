mixin class A {}
typedef B = A;
class C extends A with B {}
//                     ^
// [diag.mixinsSuperClass] 'mixin class A' can't be used in both the 'extends' and 'with' clauses.
