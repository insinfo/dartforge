f({a, b}) {}
main() {
  f(c: 1);
//  ^
// [diag.undefinedNamedParameter] The named parameter 'c' isn't defined.
}
