enum E() {
  v;
  this {
//     ^
// [diag.constPrimaryConstructorWithBlockBody] The body part of a constant primary constructor can't have a block body.
    this;
  }
}
