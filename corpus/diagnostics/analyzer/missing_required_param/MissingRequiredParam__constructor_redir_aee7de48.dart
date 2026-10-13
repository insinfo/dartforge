class C {
  C({required int x});
  C.named() : this();
//            ^^^^^^
// [diag.missingRequiredArgument] The named parameter 'x' is required, but there's no corresponding argument.
}
