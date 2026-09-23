import 'dart:ffi';
final class UintPtr extends AbiSpecificInteger {
//          ^^^^^^^
// [diag.abiSpecificIntegerMappingMissing] Classes extending 'AbiSpecificInteger' must have exactly one 'AbiSpecificIntegerMapping' annotation specifying the mapping from ABI to a 'NativeType' integer with a fixed size.
  const UintPtr();
}
