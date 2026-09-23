class A {
  final int x;
  const A() : x = '';
//                ^^
// [diag.constFieldInitializerNotAssignable] The initializer type 'String' can't be assigned to the field type 'int' in a const constructor.
}
