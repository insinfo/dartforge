class A<K, V extends K> {}
typedef void F<T extends A>();
//                       ^
// [diag.notInstantiatedBound] Type parameter bound types must be instantiated.
