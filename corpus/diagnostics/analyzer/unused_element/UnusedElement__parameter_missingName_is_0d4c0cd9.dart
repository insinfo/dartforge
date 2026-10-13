class C {
  C.impl({int? x});
  factory C({}) = C.impl;
//           ^
// [diag.missingIdentifier] Expected an identifier.
//                ^^^^^^
// [diag.redirectToInvalidFunctionType] The redirected constructor 'C Function({int? x})' has incompatible parameters with 'C Function({dynamic})'.
}
