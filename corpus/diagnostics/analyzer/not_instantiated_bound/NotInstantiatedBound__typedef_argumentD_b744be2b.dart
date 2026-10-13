class A<K, V extends List<List<K>>> {}
typedef void F<T extends A>();
//                       ^
// [diag.notInstantiatedBound] Type parameter bound types must be instantiated.
