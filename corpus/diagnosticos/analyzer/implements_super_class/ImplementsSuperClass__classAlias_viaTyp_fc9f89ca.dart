// %before-language-feature: augmentations
class A {}
mixin M {}
typedef B = A;
class C = A with M implements B;
//                            ^
// [diag.implementsSuperClass] 'class A' can't be used in both the 'extends' and 'implements' clauses.
