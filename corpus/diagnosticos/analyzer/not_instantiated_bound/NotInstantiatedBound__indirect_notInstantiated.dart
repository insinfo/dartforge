class A<K, V extends K> {}
class C<T extends List<A>> {}
//                     ^
// [diag.notInstantiatedBound] Type parameter bound types must be instantiated.
