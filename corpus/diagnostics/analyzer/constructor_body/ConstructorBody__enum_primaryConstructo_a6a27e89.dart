enum E() {
  v;
  this sync* {}
//     ^^^^
// [diag.primaryConstructorBodyWithModifier] A primary constructor body can't have the modifier 'sync*'.
//           ^
// [diag.constPrimaryConstructorWithBlockBody] The body part of a constant primary constructor can't have a block body.
}
