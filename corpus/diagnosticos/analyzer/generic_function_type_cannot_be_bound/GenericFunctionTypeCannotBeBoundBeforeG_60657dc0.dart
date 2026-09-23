// %before-language-feature: generic-metadata
late T Function<T extends S Function<S>(S)>(T) fun;
//                        ^^^^^^^^^^^^^^^^
// [diag.genericFunctionTypeCannotBeBound] Generic function types can't be used as type parameter bounds.
