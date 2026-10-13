f(void Function<T>() foo, void Function<T, U>() bar) {
  (1 == 2 ? foo : bar)<int, String>;
//^^^^^^^^^^^^^^^^^^^^
// [diag.disallowedTypeInstantiationExpression] Only a generic type, generic function, generic instance method, or generic constructor can have type arguments.
}
