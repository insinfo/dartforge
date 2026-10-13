// %before-language-feature: generic-metadata
typedef foo = T Function<T extends S Function<S>(S)>(T t);
//                                 ^^^^^^^^^^^^^^^^
// [diag.genericFunctionTypeCannotBeBound] Generic function types can't be used as type parameter bounds.
