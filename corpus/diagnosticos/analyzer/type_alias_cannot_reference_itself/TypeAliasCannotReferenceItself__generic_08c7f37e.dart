typedef A<T extends A<int>> = void Function();
//      ^
// [diag.typeAliasCannotReferenceItself] Typedefs can't reference themselves directly or recursively via another typedef.
