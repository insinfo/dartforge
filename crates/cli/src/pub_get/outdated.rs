//! `dart pub outdated`.
use super::Failure;

pub(super) fn run(_args: &[String]) -> Result<u8, Failure> {
    Err("pub outdated is not implemented yet".into())
}
