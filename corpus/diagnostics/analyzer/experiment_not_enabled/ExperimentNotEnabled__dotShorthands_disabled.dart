// %before-language-feature: dot-shorthands
void main() {
  Object c = .hash(1, 2);
//           ^
// [diag.experimentNotEnabled] This requires the 'dot-shorthands' language feature to be enabled.
  print(c);
}
