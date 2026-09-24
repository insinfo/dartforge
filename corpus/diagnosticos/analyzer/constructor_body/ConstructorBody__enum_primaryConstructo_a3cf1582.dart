enum E() {
  v;
  this async {}
//     ^^^^^
// [diag.primaryConstructorBodyWithModifier] A primary constructor body can't have the modifier 'async'.
//           ^
// [diag.constPrimaryConstructorWithBlockBody] The body part of a constant primary constructor can't have a block body.
}
