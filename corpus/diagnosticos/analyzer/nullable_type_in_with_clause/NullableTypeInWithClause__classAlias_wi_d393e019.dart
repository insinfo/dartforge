class A {}
mixin B {}
typedef C = B?;
class D = A with C;
//               ^
// [diag.nullableTypeInWithClause] Nullable types can't be mixed in.
