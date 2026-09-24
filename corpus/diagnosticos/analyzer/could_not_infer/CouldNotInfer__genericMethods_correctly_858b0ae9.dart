class Foo<T extends Pattern> {
  U method<U extends T>(U u) => u;
}
main() {
  new Foo<String>().method(42);
//                         ^^
// [diag.argumentTypeNotAssignable] The argument type 'int' can't be assigned to the parameter type 'String'.
}
