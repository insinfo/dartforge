const List? l = [];
const int? c = l?.length;
//             ^^^^^^^^^
// [diag.constEvalPropertyAccess] The property 'length' can't be accessed on the type 'List<dynamic>' in a constant expression.
