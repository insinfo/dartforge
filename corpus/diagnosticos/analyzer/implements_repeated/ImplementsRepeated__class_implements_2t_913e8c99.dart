// %before-language-feature: augmentations
class A {}
typedef B = A;
class C implements A, B {}
//                    ^
// [diag.implementsRepeated] 'A' can only be implemented once.
