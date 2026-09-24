class A {}
mixin M on A, A {}
//            ^
// [diag.onRepeated] The type 'A' can be included in the superclass constraints only once.
