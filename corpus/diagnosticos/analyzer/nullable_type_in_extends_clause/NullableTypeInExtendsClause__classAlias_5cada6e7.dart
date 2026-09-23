class A {}
mixin B {}
class C = A? with B;
//        ^^
// [diag.nullableTypeInExtendsClause] Nullable types can't be extended.
