class A {}
typedef B = A?;
extension type E(A _) implements B {}
//                               ^
// [diag.nullableTypeInImplementsClause] Nullable types can't be implemented.
