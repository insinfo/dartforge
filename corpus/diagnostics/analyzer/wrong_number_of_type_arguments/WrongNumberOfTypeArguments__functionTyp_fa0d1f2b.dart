f(void Function<T, U>() foo, void Function<T, U>() bar) {
  (1 == 2 ? foo : bar)<int>;
//                    ^^^^^
// [diag.wrongNumberOfTypeArgumentsFunction] The type of this function is 'void Function<T, U>()', which has 2 type parameters, but 1 type arguments were given.
}
