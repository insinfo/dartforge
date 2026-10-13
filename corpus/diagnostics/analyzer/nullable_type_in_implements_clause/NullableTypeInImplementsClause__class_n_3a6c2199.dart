class A {}
typedef B = A?;
class C implements B {}
//                 ^
// [diag.nullableTypeInImplementsClause] Nullable types can't be implemented.
