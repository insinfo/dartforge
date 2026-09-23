abstract class I {
  String toString([int? value]);
}

enum E1 implements I {
//   ^^
// [diag.invalidImplementationOverride] 'Object.toString' ('String Function()') isn't a valid concrete implementation of 'I.toString' ('String Function([int?])').
    v
}

enum E2 implements I {
  v;
  String toString([int? value]) => '';
}
