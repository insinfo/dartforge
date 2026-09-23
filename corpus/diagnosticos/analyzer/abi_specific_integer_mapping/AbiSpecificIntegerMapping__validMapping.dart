import 'dart:ffi';
@AbiSpecificIntegerMapping({
  Abi.androidArm: Uint32(),
  Abi.androidArm64: Uint64(),
  Abi.androidIA32: Uint32(),
})
final class UintPtr extends AbiSpecificInteger {
  const UintPtr();
}
