extension type A<T>(T it) {}

class B<U extends A<U>> {}
//      ^
// [diag.typeParameterSupertypeOfItsBound] 'U' can't be a supertype of its upper bound.
