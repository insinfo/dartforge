typedef T1 = T2;
//      ^^
// [diag.typeAliasCannotReferenceItself] Typedefs can't reference themselves directly or recursively via another typedef.
typedef T2 = T1;
//      ^^
// [diag.typeAliasCannotReferenceItself] Typedefs can't reference themselves directly or recursively via another typedef.
