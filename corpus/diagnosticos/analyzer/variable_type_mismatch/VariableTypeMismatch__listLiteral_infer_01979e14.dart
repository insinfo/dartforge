const dynamic x = [1];
const List<String> y = x;
//                     ^
// [diag.variableTypeMismatch] A value of type 'List<int>' can't be assigned to a const variable of type 'List<String>'.
