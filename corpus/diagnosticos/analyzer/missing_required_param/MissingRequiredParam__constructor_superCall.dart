class C {
  C({required int a}) {}
}

class D extends C {
  D() : super();
//      ^^^^^^^
// [diag.missingRequiredArgument] The named parameter 'a' is required, but there's no corresponding argument.
}
