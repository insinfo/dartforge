final class C {}
@Deprecated.subclass()
// [diag.invalidDeprecatedSubclassAnnotation][column 2][length 19] The annotation '@Deprecated.subclass' can only be applied to subclassable classes and mixins.
typedef D = C;
