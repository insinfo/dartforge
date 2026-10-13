// %before-language-feature: generic-metadata
class C<T extends S Function<S>(S)> {
//                ^^^^^^^^^^^^^^^^
// [diag.genericFunctionTypeCannotBeBound] Generic function types can't be used as type parameter bounds.
}
