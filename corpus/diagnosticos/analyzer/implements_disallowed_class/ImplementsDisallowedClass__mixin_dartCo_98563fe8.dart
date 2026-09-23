// %before-language-feature: enhanced-enums
mixin M implements Enum {}
//                 ^^^^
// [diag.implementsDisallowedClass] Classes and mixins can't implement 'Enum'.
