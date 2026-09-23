class A {
  dynamic field = ({this.field}) {};
//                  ^^^^
// [diag.fieldInitializerOutsideConstructor] Field formal parameters can only be used in a constructor.
}
