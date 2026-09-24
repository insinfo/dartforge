int y = 0;
class A([int x = y]);
//               ^
// [diag.nonConstantDefaultValue] The default value of an optional parameter must be constant.
