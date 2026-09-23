typedef Fn<T, U> = void Function(T, U);
var t = Fn<int>;
//        ^^^^^
// [diag.wrongNumberOfTypeArguments] The type 'Fn' is declared with 2 type parameters, but 1 type arguments were given.
