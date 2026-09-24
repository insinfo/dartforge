// %before-language-feature: augmentations
class A {}
class B implements A, A {}
//                    ^
// [diag.implementsRepeated] 'A' can only be implemented once.
