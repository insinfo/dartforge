// %before-language-feature: augmentations
class C {
  external void set foo(int v) {}
//                             ^
// [diag.externalMethodWithBody] An external or native method can't have a body.
}
