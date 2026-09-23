class A<T extends B> {} // points to a
//                ^
// [diag.notInstantiatedBound] Type parameter bound types must be instantiated.
class B<T extends A> {} // points to b
//                ^
// [diag.notInstantiatedBound] Type parameter bound types must be instantiated.
class C<T extends A> {} // points to a cyclical type
//                ^
// [diag.notInstantiatedBound] Type parameter bound types must be instantiated.
