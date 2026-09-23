f(list) {
  await for (var e in list) {
//^^^^^
// [diag.asyncForInWrongContext] The async for-in loop can only be used in an async function.
//               ^
// [diag.unusedLocalVariable] The value of the local variable 'e' isn't used.
  }
}
