typedef T = void Function(T);
//      ^
// [diag.typeAliasCannotReferenceItself] Typedefs can't reference themselves directly or recursively via another typedef.
