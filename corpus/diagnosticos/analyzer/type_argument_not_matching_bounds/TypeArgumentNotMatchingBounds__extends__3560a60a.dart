class X<T extends Type> {}
class Y<U> extends X<U> {}
//                   ^
// [diag.typeArgumentNotMatchingBounds] 'U' doesn't conform to the bound 'Type' of the type parameter 'T'.
