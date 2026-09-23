const int? s = null;
const bool? c = s?.isEven;
//              ^^^^^^^^^
// [diag.constEvalPropertyAccess] The property 'isEven' can't be accessed on the type 'Null' in a constant expression.
