class A {}
mixin B {}
typedef C = A?;
class D = C with B;
//        ^
// [diag.nullableTypeInExtendsClause] Nullable types can't be extended.
