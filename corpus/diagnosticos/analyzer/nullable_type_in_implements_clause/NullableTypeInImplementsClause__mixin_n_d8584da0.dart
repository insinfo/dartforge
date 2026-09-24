class A {}
typedef B = A?;
mixin C implements B {}
//                 ^
// [diag.nullableTypeInImplementsClause] Nullable types can't be implemented.
