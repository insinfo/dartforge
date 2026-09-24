int f() sync* {}
// [diag.illegalSyncGeneratorReturnType][column 1][length 3] Functions marked 'sync*' must have a return type that is a supertype of 'Iterable<T>' for some type 'T'.
