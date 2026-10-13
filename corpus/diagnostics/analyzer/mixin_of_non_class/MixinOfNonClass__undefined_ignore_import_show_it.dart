import 'a.dart' show M;
//     ^^^^^^^^
// [diag.uriDoesNotExist] Target of URI doesn't exist: 'a.dart'.

class C with M {}
