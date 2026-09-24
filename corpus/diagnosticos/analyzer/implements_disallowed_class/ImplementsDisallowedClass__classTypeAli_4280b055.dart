// %before-language-feature: enhanced-enums
mixin M {}
class A = Object with M implements Enum;
//                                 ^^^^
// [diag.implementsDisallowedClass] Classes and mixins can't implement 'Enum'.
