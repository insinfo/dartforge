import 'dart:ffi';
const c = {
  Abi.androidArm: Uint32(),
  Abi.androidArm64: IntPtr(),
  Abi.androidIA32: UintPtr(),
};
@AbiSpecificIntegerMapping(c)
//                         ^
// [diag.abiSpecificIntegerMappingUnsupported] Invalid mapping to 'IntPtr'; only mappings to 'Int8', 'Int16', 'Int32', 'Int64', 'Uint8', 'Uint16', 'UInt32', and 'Uint64' are supported.
// [diag.abiSpecificIntegerMappingUnsupported] Invalid mapping to 'UintPtr'; only mappings to 'Int8', 'Int16', 'Int32', 'Int64', 'Uint8', 'Uint16', 'UInt32', and 'Uint64' are supported.
final class UintPtr extends AbiSpecificInteger {
  const UintPtr();
}
