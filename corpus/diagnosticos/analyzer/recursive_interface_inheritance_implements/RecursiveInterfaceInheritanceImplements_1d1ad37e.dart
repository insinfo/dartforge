class A {}
mixin M {}
class B = A with M implements B;
//                            ^
// [diag.recursiveInterfaceInheritanceImplements] 'B' can't implement itself.
