f() async* {
  var await = 1;
//    ^^^^^
// [diag.asyncKeywordUsedAsIdentifier] The keywords 'await' and 'yield' can't be used as identifiers in an asynchronous or generator function.
// [diag.unusedLocalVariable] The value of the local variable 'await' isn't used.
}
