Iterable<int> f() sync* => 3;
//                      ^^
// [diag.returnInGenerator] Can't return a value from a generator function that uses the 'async*' or 'sync*' modifier.
