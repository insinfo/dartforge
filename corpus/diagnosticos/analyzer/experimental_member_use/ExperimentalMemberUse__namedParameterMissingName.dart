class C {
  const C({this.});
//         ^^^^^
// [diag.initializingFormalForNonExistentField] '' isn't a field in the enclosing class.
//              ^
// [diag.missingIdentifier] Expected an identifier.
}
var z = C(x: '');
//        ^
// [diag.undefinedNamedParameter] The named parameter 'x' isn't defined.
