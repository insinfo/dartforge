import 'dart:ffi';
import 'dart:math' as prefix;
void main() {
  Pointer.fromFunction<Void Function()>(prefix);
//                                      ^^^^^^
// [diag.prefixIdentifierNotFollowedByDot] The name 'prefix' refers to an import prefix, so it must be followed by '.'.
// [diag.mustBeASubtype] The type 'InvalidType' must be a subtype of 'Void Function()' for 'fromFunction'.
}
