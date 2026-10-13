// %before-language-feature: primary-constructors
class C {
  C([final this.value = 0]);
//   ^^^^^
// [diag.unnecessaryFinal] The keyword 'final' isn't necessary because the parameter is implicitly 'final'.
  int value;
}
