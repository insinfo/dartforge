import 'a.dart' show B;
//     ^^^^^^^^
// [diag.uriDoesNotExist] Target of URI doesn't exist: 'a.dart'.

class C extends A {}
//              ^
// [diag.extendsNonClass] Classes can only extend other classes.
