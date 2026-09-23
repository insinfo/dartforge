void f(({int a, int a}) r) {}
//           ^
// [context 1] The first
//                  ^
// [diag.duplicateFieldName][context 1] The field name 'a' is already used in this record.
