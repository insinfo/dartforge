extension type A<T>(T it) {}

class B<T1 extends A<T2>, T2 extends T1> {}
//      ^^
// [diag.typeParameterSupertypeOfItsBound] 'T1' can't be a supertype of its upper bound.
//                        ^^
// [diag.typeParameterSupertypeOfItsBound] 'T2' can't be a supertype of its upper bound.
