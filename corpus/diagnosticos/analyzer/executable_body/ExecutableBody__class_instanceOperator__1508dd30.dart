// %before-language-feature: augmentations
class C {
  external int operator +(int other) {
//                                   ^
// [diag.externalMethodWithBody] An external or native method can't have a body.
    return 0;
  }
}
