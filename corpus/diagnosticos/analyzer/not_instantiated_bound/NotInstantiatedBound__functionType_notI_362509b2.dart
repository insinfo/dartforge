class A<T extends Function(T)> {}
class B<T extends T Function()> {}
class C<T extends A> {}
//                ^
// [diag.notInstantiatedBound] Type parameter bound types must be instantiated.
class D<T extends B> {}
//                ^
// [diag.notInstantiatedBound] Type parameter bound types must be instantiated.
