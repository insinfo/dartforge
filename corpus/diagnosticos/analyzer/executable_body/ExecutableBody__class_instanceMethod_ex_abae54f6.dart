// %before-language-feature: augmentations
class C {
  external void foo() => null;
//                    ^^
// [diag.externalMethodWithBody] An external or native method can't have a body.
}
