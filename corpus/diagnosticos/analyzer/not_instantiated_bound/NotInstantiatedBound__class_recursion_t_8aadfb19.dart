typedef F(C value);
//      ^
// [diag.typeAliasCannotReferenceItself] Typedefs can't reference themselves directly or recursively via another typedef.
class C<T extends F> {}
//                ^
// [diag.notInstantiatedBound] Type parameter bound types must be instantiated.
class D<T extends C> {}
//                ^
// [diag.notInstantiatedBound] Type parameter bound types must be instantiated.
