class A {}
extension type E(A _) implements A? {}
//                               ^^
// [diag.nullableTypeInImplementsClause] Nullable types can't be implemented.
