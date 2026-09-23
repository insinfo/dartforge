extension type A(int it) implements X {}
//                                  ^
// [diag.nullableTypeInImplementsClause] Nullable types can't be implemented.
typedef X = num?;
