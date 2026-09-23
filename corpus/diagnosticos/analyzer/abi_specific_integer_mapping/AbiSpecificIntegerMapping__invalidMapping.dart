import 'dart:ffi';
@AbiSpecificIntegerMapping({
  Abi.androidArm: Uint32(),
  Abi.androidArm64: IntPtr(),
//                  ^^^^^^^^
// [diag.abiSpecificIntegerMappingUnsupported] Invalid mapping to 'IntPtr'; only mappings to 'Int8', 'Int16', 'Int32', 'Int64', 'Uint8', 'Uint16', 'UInt32', and 'Uint64' are supported.
  Abi.androidIA32: UintPtr(),
//                 ^^^^^^^^^
// [diag.abiSpecificIntegerMappingUnsupported] Invalid mapping to 'UintPtr'; only mappings to 'Int8', 'Int16', 'Int32', 'Int64', 'Uint8', 'Uint16', 'UInt32', and 'Uint64' are supported.
})
final class UintPtr extends AbiSpecificInteger {
  const UintPtr();
}
