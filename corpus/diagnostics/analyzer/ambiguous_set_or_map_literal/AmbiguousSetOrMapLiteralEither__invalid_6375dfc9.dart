union(a, b) => !{...a, ...b};
//              ^^^^^^^^^^^^
// [diag.ambiguousSetOrMapLiteralEither] This literal must be either a map or a set, but the elements don't have enough information for type inference to work.
