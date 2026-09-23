// %before-language-feature: class-modifiers
class Function {}
//    ^^^^^^^^
// [diag.builtInIdentifierAsTypeName] The built-in identifier 'Function' can't be used as a type name.
class A extends Function {}
