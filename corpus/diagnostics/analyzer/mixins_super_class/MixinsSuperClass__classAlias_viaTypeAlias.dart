mixin class A {}
typedef B = A;
class C = A with B;
//               ^
// [diag.mixinsSuperClass] 'mixin class A' can't be used in both the 'extends' and 'with' clauses.
