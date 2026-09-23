typedef A<X> = X Function(X);
typedef G<X extends A<X>> = void Function<Y extends X>();
foo(G g) {}
//  ^
// [context 1] The raw type was instantiated as 'G<dynamic Function(dynamic)>', and is not regular-bounded.
// [context 2] The inverted type 'G<Never Function(Never)>' is also not regular-bounded, so the type is not well-bounded.
// [diag.typeArgumentNotMatchingBounds][context 1][context 2] 'A<Never>' doesn't conform to the bound 'A<A<Never>>' of the type parameter 'X'.
