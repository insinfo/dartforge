void f<T>() {}
const dynamic x = f;
const y = (f)<int, String>;
//           ^^^^^^^^^^^^^
// [diag.wrongNumberOfTypeArgumentsFunction] The type of this function is 'void Function<T>()', which has 1 type parameters, but 2 type arguments were given.
