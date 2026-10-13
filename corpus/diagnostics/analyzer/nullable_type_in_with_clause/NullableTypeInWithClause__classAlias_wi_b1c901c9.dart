class A {}
mixin B {}
class C = A with B?;
//               ^^
// [diag.nullableTypeInWithClause] Nullable types can't be mixed in.
