class A {}
mixin B on A? {}
//         ^^
// [diag.nullableTypeInOnClause] Nullable types can't be used as a superclass constraint.
