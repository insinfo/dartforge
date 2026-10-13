abstract class A implements A {}
//                          ^
// [diag.recursiveInterfaceInheritanceImplements] 'A' can't implement itself.
class B implements A {}
