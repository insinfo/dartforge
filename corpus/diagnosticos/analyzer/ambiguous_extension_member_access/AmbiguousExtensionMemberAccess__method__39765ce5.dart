// ignore_for_file: unused_element
void f(Iterable<void> p) {
  p.foo();
//  ^^^
// [diag.ambiguousExtensionMemberAccessTwo][context 1][context 2] A member named 'foo' is defined in 'extension on Iterable<InvalidType> (where <unnamed extension> is defined in /home/test/lib/test.dart)' and 'extension on Iterable<InvalidType> (where <unnamed extension> is defined in /home/test/lib/test.dart)', and neither is more specific.
}
extension on Iterable<Undef1> { void foo() {} }
// [context 1][column 1][length 0] <unnamed extension> is defined in /home/test/lib/test.dart
//                    ^^^^^^
// [diag.nonTypeAsTypeArgument] The name 'Undef1' isn't a type, so it can't be used as a type argument.
extension on Iterable<Undef2> { void foo() {} }
// [context 2][column 1][length 0] <unnamed extension> is defined in /home/test/lib/test.dart
//                    ^^^^^^
// [diag.nonTypeAsTypeArgument] The name 'Undef2' isn't a type, so it can't be used as a type argument.
