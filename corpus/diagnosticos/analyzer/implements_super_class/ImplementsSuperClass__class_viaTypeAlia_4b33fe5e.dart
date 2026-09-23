// %before-language-feature: augmentations
class A {}
typedef B = A;
class C extends A implements B {}
//                           ^
// [diag.implementsSuperClass] 'class A' can't be used in both the 'extends' and 'implements' clauses.
