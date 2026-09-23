class C {
  external int get foo {
//                     ^
// [diag.externalMethodWithBody] An external or native method can't have a body.
    return 0;
  }
}
