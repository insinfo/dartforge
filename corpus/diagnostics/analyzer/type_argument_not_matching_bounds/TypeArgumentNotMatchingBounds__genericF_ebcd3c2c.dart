typedef F = T Function<T>(T);
typedef FB<T extends F> = S Function<S extends T>(S);
class CB<T extends F> {}
void f(CB<FB<F>> a) {}
//     ^^^^^^^^^
// [context 1] The inverted type 'CB<S Function<S extends T Function<T>(T)>(S)>' is also not regular-bounded, so the type is not well-bounded.
//        ^^^^^
// [diag.typeArgumentNotMatchingBounds][context 1] 'FB<F>' doesn't conform to the bound 'F' of the type parameter 'T'.
