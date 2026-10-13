mixin A {}
typedef B = A;
class C with B? {}
//           ^^
// [diag.nullableTypeInWithClause] Nullable types can't be mixed in.
