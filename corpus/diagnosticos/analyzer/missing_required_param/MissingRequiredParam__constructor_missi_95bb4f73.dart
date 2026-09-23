class C {
  C({required int a}) {}
}
main() {
  new C();
//    ^
// [diag.missingRequiredArgument] The named parameter 'a' is required, but there's no corresponding argument.
}
