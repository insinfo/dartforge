// %before-language-feature: augmentations
extension type const E(int it) {
  const factory E.named();
//^^^^^
// [diag.constFactory] Only redirecting factory constructors can be declared to be 'const'.
//                       ^
// [diag.missingFunctionBody] A function body must be provided.
}
