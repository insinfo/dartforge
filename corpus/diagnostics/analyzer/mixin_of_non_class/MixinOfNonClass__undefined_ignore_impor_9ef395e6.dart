import 'a.dart' show N;
//     ^^^^^^^^
// [diag.uriDoesNotExist] Target of URI doesn't exist: 'a.dart'.

class C with M {}
//           ^
// [diag.mixinOfNonClass] Classes can only mix in mixins and classes.
