class C([int x]) {
//           ^
// [diag.missingDefaultValueForParameterPositional] The parameter 'x' can't have a value of 'null' because of its type, but the implicit default value is 'null'.
  augment C([int x]);
}
