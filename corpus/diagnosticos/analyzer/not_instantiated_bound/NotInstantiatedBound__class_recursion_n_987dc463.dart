class A<T extends void Function(A)> {}
//                              ^
// [diag.notInstantiatedBound] Type parameter bound types must be instantiated.
