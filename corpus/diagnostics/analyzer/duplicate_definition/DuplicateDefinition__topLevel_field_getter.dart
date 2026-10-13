int f = 1;
//  ^
// [context 1] The first definition of this name.
int get f => 7;
//      ^
// [diag.duplicateDefinition][context 1] The name 'f' is already defined.
