class A {}
typedef B = A;
mixin M on A, B {}
//            ^
// [diag.onRepeated] The type 'A' can be included in the superclass constraints only once.
