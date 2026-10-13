class A {}
mixin M {}
class main = A with M;
//    ^^^^
// [diag.mainIsNotFunction] The declaration named 'main' must be a function.
