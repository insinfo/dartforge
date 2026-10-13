// %before-language-feature: augmentations
class C {
  const factory C();
//^^^^^
// [diag.constFactory] Only redirecting factory constructors can be declared to be 'const'.
//                 ^
// [diag.missingFunctionBody] A function body must be provided.
}
