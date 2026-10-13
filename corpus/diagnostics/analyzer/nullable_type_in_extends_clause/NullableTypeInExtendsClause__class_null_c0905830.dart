class A {}
typedef B = A;
class C extends B? {}
//              ^^
// [diag.nullableTypeInExtendsClause] Nullable types can't be extended.
