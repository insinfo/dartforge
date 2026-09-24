// %before-language-feature: augmentations
class A {}
class B extends A implements A {}
//                           ^
// [diag.implementsSuperClass] 'class A' can't be used in both the 'extends' and 'implements' clauses.
