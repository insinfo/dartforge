extension _A on bool {
  int operator [](int index) => 7;
//             ^^
// [diag.unusedElement] The declaration '[]' isn't referenced.
}
