// %before-language-feature: augmentations
class A {}
enum E implements A, A, A, A {
//                   ^
// [diag.implementsRepeated] 'A' can only be implemented once.
//                      ^
// [diag.implementsRepeated] 'A' can only be implemented once.
//                         ^
// [diag.implementsRepeated] 'A' can only be implemented once.
  v
}
