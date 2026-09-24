class A {}
mixin B implements A? {}
//                 ^^
// [diag.nullableTypeInImplementsClause] Nullable types can't be implemented.
