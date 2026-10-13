class A<T1 extends T3, T2, T3 extends T1> {
//      ^^
// [diag.typeParameterSupertypeOfItsBound] 'T1' can't be a supertype of its upper bound.
//                         ^^
// [diag.typeParameterSupertypeOfItsBound] 'T3' can't be a supertype of its upper bound.
}
