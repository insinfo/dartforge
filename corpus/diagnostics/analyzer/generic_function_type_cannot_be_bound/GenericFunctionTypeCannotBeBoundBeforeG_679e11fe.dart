// %before-language-feature: generic-metadata
typedef T foo<T extends S Function<S>(S)>(T t);
//                      ^^^^^^^^^^^^^^^^
// [diag.genericFunctionTypeCannotBeBound] Generic function types can't be used as type parameter bounds.
