enum E<T extends int> {
  v<String>()
//  ^^^^^^
// [diag.typeArgumentNotMatchingBounds] 'String' doesn't conform to the bound 'int' of the type parameter 'T'.
}
