class A<K, V extends K> {}
class C<T extends A> {}
//                ^
// [diag.notInstantiatedBound] Type parameter bound types must be instantiated.
