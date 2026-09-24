class A {}
typedef B = A?;
mixin C on B {}
//         ^
// [diag.nullableTypeInOnClause] Nullable types can't be used as a superclass constraint.
