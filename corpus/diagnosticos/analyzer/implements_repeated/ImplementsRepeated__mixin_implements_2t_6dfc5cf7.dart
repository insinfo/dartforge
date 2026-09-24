// %before-language-feature: augmentations
class A {}
mixin M implements A, A {}
//                    ^
// [diag.implementsRepeated] 'A' can only be implemented once.
