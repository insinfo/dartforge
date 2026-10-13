const dynamic x = {1: 1};
const Map<dynamic, String> y = x;
//                             ^
// [diag.variableTypeMismatch] A value of type 'Map<int, int>' can't be assigned to a const variable of type 'Map<dynamic, String>'.
