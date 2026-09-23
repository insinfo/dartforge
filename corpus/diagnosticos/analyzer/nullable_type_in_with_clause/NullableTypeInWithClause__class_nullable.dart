mixin A {}
class B with A? {}
//           ^^
// [diag.nullableTypeInWithClause] Nullable types can't be mixed in.
