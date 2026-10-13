extension _A on bool {
  operator []=(int index, int value) {}
//         ^^^
// [diag.unusedElement] The declaration '[]=' isn't referenced.
}
