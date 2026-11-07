//! Native artifacts bind both compiler declarations and SDK execution flags.
use crate::{CompilationEnvironment, DartFlags};

const SCHEMA: &str = "dartforge-native-context-v1";

pub(crate) struct IrContext {
    pub environment: CompilationEnvironment,
    pub flags: Option<DartFlags>,
}

pub(crate) fn identity(environment: &CompilationEnvironment, flags: DartFlags) -> String {
    serde_json::to_string(&(
        SCHEMA,
        environment.identity(),
        flags.enable_asserts,
        flags.product,
    ))
    .expect("native context strings and flags serialize")
}

pub(crate) fn from_ir(ir: &str) -> Result<Option<IrContext>, String> {
    dartforge_abi::compilation_environment::ir_identity(ir)?
        .map(parse)
        .transpose()
}

fn parse(identity: &str) -> Result<IrContext, String> {
    let value: serde_json::Value = serde_json::from_str(identity)
        .map_err(|error| format!("invalid native compilation context: {error}"))?;
    if value.get(0).and_then(serde_json::Value::as_str) == Some(SCHEMA) {
        let (_, environment, enable_asserts, product): (String, String, bool, bool) =
            serde_json::from_value(value)
                .map_err(|error| format!("invalid native compilation context: {error}"))?;
        Ok(IrContext {
            environment: CompilationEnvironment::from_identity(&environment)?,
            flags: Some(DartFlags {
                enable_asserts,
                product,
            }),
        })
    } else {
        // Older IR records declarations alone. Its execution flags are unknown;
        // callers must retain that distinction when selecting an SDK image.
        Ok(IrContext {
            environment: CompilationEnvironment::from_identity(identity)?,
            flags: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EnvironmentTarget;

    #[test]
    fn sdk_and_linker_reject_changed_execution_flags_before_loading_or_linking() {
        let directory = tempfile::tempdir().unwrap();
        let environment = CompilationEnvironment::new(
            EnvironmentTarget::NativeAot,
            &dartforge_elements::SdkLayout::default(),
            true,
            &[],
        )
        .unwrap();
        let flags = DartFlags::PRODUCTION;
        let changed = DartFlags {
            enable_asserts: true,
            ..flags
        };
        let dll = directory.path().join("absent-sdk.dll");
        std::fs::write(
            directory
                .path()
                .join(dartforge_abi::compilation_environment::SDK_IDENTITY_FILE),
            identity(&environment, flags),
        )
        .unwrap();
        assert!(
            crate::sdk_module::validate_sdk_environment(&dll, Some(&environment), flags).is_ok()
        );
        assert!(
            crate::sdk_module::validate_sdk_environment(&dll, Some(&environment), changed).is_err()
        );
        let ir = format!(
            "{}{}\n",
            dartforge_abi::compilation_environment::IR_PREFIX,
            identity(&environment, flags)
        );
        let output = directory.path().join("must-not-be-created.exe");
        let options = crate::driver::NativeDriverOptions {
            flags: changed,
            ..Default::default()
        };
        let error = crate::driver::compile_and_link(&ir, &output, &options).unwrap_err();
        assert!(error.contains("execution flags"), "{error}");
        assert!(!output.exists());
    }

    #[test]
    fn assertions_and_product_are_bound_independently_of_declaration_overrides() {
        let env = CompilationEnvironment::new(
            EnvironmentTarget::NativeAot,
            &dartforge_elements::SdkLayout::default(),
            true,
            &["dart.vm.product=false".into()],
        )
        .unwrap();
        let flags = DartFlags::PRODUCTION;
        let context = parse(&identity(&env, flags)).unwrap();
        assert_eq!(context.environment, env);
        assert_eq!(context.flags, Some(flags));
        assert_eq!(context.environment.get("dart.vm.product"), Some("false"));
        assert_ne!(
            identity(&env, flags),
            identity(
                &env,
                DartFlags {
                    enable_asserts: true,
                    ..flags
                }
            )
        );
        assert_ne!(
            identity(&env, flags),
            identity(
                &env,
                DartFlags {
                    product: false,
                    ..flags
                }
            )
        );
        assert_eq!(parse(&env.identity()).unwrap().flags, None);
        assert!(parse("[\"dartforge-native-context-v1\",\"bad\",false,true]").is_err());
        assert!(parse("[\"dartforge-native-context-v1\",null,false,true]").is_err());
    }
}
