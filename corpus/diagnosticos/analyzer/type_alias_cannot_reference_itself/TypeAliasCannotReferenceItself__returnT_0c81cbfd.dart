typedef B A();
//        ^
// [diag.typeAliasCannotReferenceItself] Typedefs can't reference themselves directly or recursively via another typedef.
typedef A B();
//        ^
// [diag.typeAliasCannotReferenceItself] Typedefs can't reference themselves directly or recursively via another typedef.
