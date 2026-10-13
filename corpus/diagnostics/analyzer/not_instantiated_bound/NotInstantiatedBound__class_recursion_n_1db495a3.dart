class A<T extends void Function<U extends A>()> {}
//                                        ^
// [diag.notInstantiatedBound] Type parameter bound types must be instantiated.
