f({a, b}) {}
main() {
  f(a: 1, a: 2);
//        ^
// [diag.duplicateNamedArgument] The argument for the named parameter 'a' was already specified.
}
