// %before-language-feature: augmentations
class A {}
typedef B = A;
enum E implements A, B {
//                   ^
// [diag.implementsRepeated] 'A' can only be implemented once.
  v
}
