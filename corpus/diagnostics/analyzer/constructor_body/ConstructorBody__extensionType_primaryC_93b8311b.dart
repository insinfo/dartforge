extension type A(int x) {
  this sync* {}
//     ^^^^
// [diag.primaryConstructorBodyWithModifier] A primary constructor body can't have the modifier 'sync*'.
}
