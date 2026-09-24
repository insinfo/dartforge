f() {
    [
      for (var __ in [1, 2, 3]) 1
//             ^^
// [diag.unusedLocalVariable] The value of the local variable '__' isn't used.
    ];
}
