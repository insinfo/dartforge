// %before-language-feature: augmentations
class A {}
mixin M {}
class B = A with M implements A;
//                            ^
// [diag.implementsSuperClass] 'class A' can't be used in both the 'extends' and 'implements' clauses.
