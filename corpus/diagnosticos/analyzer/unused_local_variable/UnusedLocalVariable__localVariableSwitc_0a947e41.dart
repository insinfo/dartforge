void f(Object o) {
  switch(o) {
    case [var __] : {}
//            ^^
// [diag.unusedLocalVariable] The value of the local variable '__' isn't used.
  }
}
