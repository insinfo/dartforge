// %before-language-feature: augmentations
mixin M {}
class A = Object with M implements Object;
//                                 ^^^^^^
// [diag.implementsSuperClass] 'class Object' can't be used in both the 'extends' and 'implements' clauses.
