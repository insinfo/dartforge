//! Allocation-only contract for the two emitted stack-pointer register reads.
//! LLVM LangRef: read_register reads machine state; it grants no Ref provenance.
//! <https://llvm.org/docs/LangRef.html#llvm-read-register-llvm-read-volatile-register-llvm-write-register-and-llvm-write-volatile-register-intrinsics>

pub(super) const SYMBOL: &str = "llvm.read_register.i64";

pub(super) fn argument(value: &str) -> bool {
    matches!(value, "metadata !{!\"sp\"}" | "metadata !{!\"rsp\"}")
}

pub(super) fn call(
    symbol: &str,
    result: &str,
    parameters: &[String],
    arguments: &[String],
) -> bool {
    symbol == SYMBOL
        && result == "i64"
        && parameters == ["metadata"]
        && arguments.len() == 1
        && argument(&arguments[0])
}

pub(super) fn parse(operation: &str) -> Option<&str> {
    let argument = operation
        .strip_prefix("call i64 @llvm.read_register.i64(")?
        .strip_suffix(')')?;
    self::argument(argument).then_some(argument)
}
