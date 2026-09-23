enum E {
  v;
//^
// [diag.constConstructorFieldTypeMismatch] In a const constructor, a value of type 'String' can't be assigned to the field 'x', which has type 'int'.
  final int x;
  const E() : x = '';
//                ^^
// [diag.constFieldInitializerNotAssignable] The initializer type 'String' can't be assigned to the field type 'int' in a const constructor.
}
