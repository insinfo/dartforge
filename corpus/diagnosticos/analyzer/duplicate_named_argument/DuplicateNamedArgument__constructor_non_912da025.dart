class C {
  C({int? a, int? b});
}
typedef D = C;
main() {
  D(a: 1, a: 2);
//        ^
// [diag.duplicateNamedArgument] The argument for the named parameter 'a' was already specified.
}
