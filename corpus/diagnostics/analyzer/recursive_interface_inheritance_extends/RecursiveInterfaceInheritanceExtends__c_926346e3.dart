class A {}
augment class A extends A {}
//                      ^
// [diag.recursiveInterfaceInheritanceExtends] 'A' can't extend itself.
