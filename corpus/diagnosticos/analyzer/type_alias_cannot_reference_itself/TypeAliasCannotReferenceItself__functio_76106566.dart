typedef A(A b());
//      ^
// [diag.typeAliasCannotReferenceItself] Typedefs can't reference themselves directly or recursively via another typedef.
