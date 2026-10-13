const elems = const [
//    ^^^^^
// [diag.recursiveCompileTimeConstant] The compile-time constant expression depends on itself.
  const [
    1, elems, 3,
  ],
];
