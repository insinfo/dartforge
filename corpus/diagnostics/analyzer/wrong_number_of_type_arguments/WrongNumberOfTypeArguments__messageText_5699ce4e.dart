class A<T> {
  A.foo() {}
}

var x = A.foo<int>;
//           ^^^^^
// [diag.wrongNumberOfTypeArgumentsConstructor] The constructor 'A.foo' doesn't have type parameters.
