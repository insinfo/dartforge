class A {
  final x;
  A([this.x = 1, this.x = 2]) {}
//        ^
// [context 1] The first definition of this name.
//                    ^
// [diag.duplicateFieldFormalParameter][context 1] The field 'x' can't be initialized by multiple parameters in the same constructor.
}
