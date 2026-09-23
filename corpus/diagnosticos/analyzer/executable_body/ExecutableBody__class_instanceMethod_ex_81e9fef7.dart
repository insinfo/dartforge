// %before-language-feature: augmentations
class C {
  external void foo() {}
//                    ^
// [diag.externalMethodWithBody] An external or native method can't have a body.
}
