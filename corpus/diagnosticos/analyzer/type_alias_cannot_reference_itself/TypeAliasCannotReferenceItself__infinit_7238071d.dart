typedef F<X extends F<X>> = F Function();
//      ^
// [diag.typeAliasCannotReferenceItself] Typedefs can't reference themselves directly or recursively via another typedef.
