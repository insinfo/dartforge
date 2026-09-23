const dynamic d = null;
const int? c = d.length;
//             ^^^^^^^^
// [diag.constEvalPropertyAccess] The property 'length' can't be accessed on the type 'Null' in a constant expression.
