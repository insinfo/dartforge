class A extends Object {}
augment class A with A {}
//                   ^
// [diag.recursiveInterfaceInheritanceWith] 'A' can't use itself as a mixin.
// [diag.classUsedAsMixin] The class 'A' can't be used as a mixin because it's neither a mixin class nor a mixin.
