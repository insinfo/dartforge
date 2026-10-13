typedef T<X extends T<Never>> = List<X>;
//      ^
// [diag.typeAliasCannotReferenceItself] Typedefs can't reference themselves directly or recursively via another typedef.
