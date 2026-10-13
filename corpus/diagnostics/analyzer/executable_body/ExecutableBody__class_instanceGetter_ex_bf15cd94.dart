// %before-language-feature: augmentations
class C {
  external int get foo => 0;
//                     ^^
// [diag.externalMethodWithBody] An external or native method can't have a body.
}
