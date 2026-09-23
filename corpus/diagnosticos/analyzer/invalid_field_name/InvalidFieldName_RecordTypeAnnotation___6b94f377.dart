// %before-language-feature: wildcard-variables

void f(({int _, int b}) r) {}
//           ^
// [diag.invalidFieldNamePrivate] Record field names can't be private.
