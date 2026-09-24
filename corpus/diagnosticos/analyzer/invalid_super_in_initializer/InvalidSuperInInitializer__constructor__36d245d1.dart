class C {
  C() : super.const();
//      ^^^^^
// [diag.invalidSuperInInitializer] Can only use 'super' in an initializer for calling the superclass constructor (e.g. 'super()' or 'super.namedConstructor()')
//            ^^^^^
// [diag.expectedIdentifierButGotKeyword] 'const' can't be used as an identifier because it's a keyword.
// [diag.missingIdentifier] Expected an identifier.
}
