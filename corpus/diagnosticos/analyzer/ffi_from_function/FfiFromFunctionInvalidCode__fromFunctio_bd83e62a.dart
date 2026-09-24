import 'dart:ffi';
import 'dart:math' as prefix;
int f(int x) => x;
void main() {
  Pointer.fromFunction<Int32 Function(Int32)>(f, prefix);
//                                               ^^^^^^
// [diag.prefixIdentifierNotFollowedByDot] The name 'prefix' refers to an import prefix, so it must be followed by '.'.
// [diag.mustBeASubtype] The type 'InvalidType' must be a subtype of 'Int32' for 'fromFunction'.
// [diag.argumentMustBeAConstant] Argument 'exceptionalReturn' must be a constant.
}
