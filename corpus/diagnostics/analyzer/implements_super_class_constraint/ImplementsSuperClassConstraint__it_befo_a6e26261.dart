// %before-language-feature: augmentations
class A {}
mixin M on A implements A {}
//                      ^
// [diag.implementsSuperClassConstraint] 'class A' can't be used in both the 'on' and 'implements' clauses.
