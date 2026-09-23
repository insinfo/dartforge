var elems = [
//  ^^^^^
// [diag.topLevelCycle] The type of 'elems' can't be inferred because it depends on itself through the cycle: elems.
  [
    1, elems, 3,
  ],
];
