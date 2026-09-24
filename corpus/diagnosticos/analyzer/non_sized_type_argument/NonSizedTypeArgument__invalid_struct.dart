import 'dart:ffi';

final class C extends Struct {
  @Array(8)
  external Array<Void> a0;
//               ^^^^
// [diag.nonSizedTypeArgument] The type 'Void' isn't a valid type argument for 'Array'. The type argument must be a native integer, 'Float', 'Double', 'Pointer', or subtype of 'Struct', 'Union', or 'AbiSpecificInteger'.
}
