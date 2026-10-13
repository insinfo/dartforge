void f(({int _, int _}) r) {}
//           ^
// [context 1] The first
// [diag.invalidFieldNamePrivate] Record field names can't be private.
//                  ^
// [diag.duplicateFieldName][context 1] The field name '_' is already used in this record.
// [diag.invalidFieldNamePrivate] Record field names can't be private.
