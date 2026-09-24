class A<T extends B<A>> {}
//                  ^
// [diag.notInstantiatedBound] Type parameter bound types must be instantiated.
class B<T extends A<B>> {}
//                  ^
// [diag.notInstantiatedBound] Type parameter bound types must be instantiated.
