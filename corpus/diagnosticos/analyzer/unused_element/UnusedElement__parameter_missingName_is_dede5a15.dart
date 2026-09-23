class C {
  C.impl({});
//        ^
// [diag.missingIdentifier] Expected an identifier.
  factory C({int? x}) = C.impl;
//                      ^^^^^^
// [diag.redirectToInvalidFunctionType] The redirected constructor 'C Function({dynamic})' has incompatible parameters with 'C Function({int? x})'.
}
