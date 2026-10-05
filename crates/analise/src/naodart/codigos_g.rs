// GERADO por `scripts/gerar-tabelas-analise.py` (frente INFRA-FASES). NÃO EDITE.
// Fonte: analyzer do SDK 3.6.2 — `analysis_options/error/option_codes.g.dart`,
// `pubspec/pubspec_warning_code.g.dart`, `manifest/manifest_warning_code.g.dart`.
// TODO(catálogo): estes 53 códigos não estão em `crates/diagnostics/src/codigos_g.rs`;
// quando entrarem, `Relato::para_diagnostic` passa a usar `Diagnostic::com_codigo`.
#![allow(missing_docs)]

use super::CodigoNaoDart;
use dartforge_diagnostics::{Severidade, TipoErro};

pub mod opcoes {
    use super::*;
    pub static INCLUDED_FILE_PARSE_ERROR: CodigoNaoDart = CodigoNaoDart { nome: "included_file_parse_error", unico: "AnalysisOptionsErrorCode.INCLUDED_FILE_PARSE_ERROR", mensagem: "{3} in {0}({1}..{2})", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false };
    pub static PARSE_ERROR: CodigoNaoDart = CodigoNaoDart { nome: "parse_error", unico: "AnalysisOptionsErrorCode.PARSE_ERROR", mensagem: "{0}", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false };
    pub static DEPRECATED_LINT: CodigoNaoDart = CodigoNaoDart { nome: "deprecated_lint", unico: "AnalysisOptionsHintCode.DEPRECATED_LINT", mensagem: "'{0}' is a deprecated lint rule and should not be used.", correcao: Some("Try removing '{0}'."), tipo: TipoErro::Hint, severidade: Severidade::Info, documentado: false };
    pub static DEPRECATED_LINT_WITH_REPLACEMENT: CodigoNaoDart = CodigoNaoDart { nome: "deprecated_lint_with_replacement", unico: "AnalysisOptionsHintCode.DEPRECATED_LINT_WITH_REPLACEMENT", mensagem: "'{0}' is deprecated and should be replaced by '{1}'.", correcao: Some("Try replacing '{0}' with '{1}'."), tipo: TipoErro::Hint, severidade: Severidade::Info, documentado: false };
    pub static DUPLICATE_RULE: CodigoNaoDart = CodigoNaoDart { nome: "duplicate_rule", unico: "AnalysisOptionsHintCode.DUPLICATE_RULE", mensagem: "The rule {0} is already specified and doesn't need to be specified again.", correcao: Some("Try removing all but one specification of the rule."), tipo: TipoErro::Hint, severidade: Severidade::Info, documentado: false };
    pub static ANALYSIS_OPTION_DEPRECATED: CodigoNaoDart = CodigoNaoDart { nome: "analysis_option_deprecated", unico: "AnalysisOptionsWarningCode.ANALYSIS_OPTION_DEPRECATED", mensagem: "The option '{0}' is no longer supported.", correcao: None, tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false };
    pub static ANALYSIS_OPTION_DEPRECATED_WITH_REPLACEMENT: CodigoNaoDart = CodigoNaoDart { nome: "analysis_option_deprecated", unico: "AnalysisOptionsWarningCode.ANALYSIS_OPTION_DEPRECATED_WITH_REPLACEMENT", mensagem: "The option '{0}' is no longer supported.", correcao: Some("Try using the new '{1}' option."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false };
    pub static INCLUDED_FILE_WARNING: CodigoNaoDart = CodigoNaoDart { nome: "included_file_warning", unico: "AnalysisOptionsWarningCode.INCLUDED_FILE_WARNING", mensagem: "Warning in the included options file {0}({1}..{2}): {3}", correcao: None, tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false };
    pub static INCLUDE_FILE_NOT_FOUND: CodigoNaoDart = CodigoNaoDart { nome: "include_file_not_found", unico: "AnalysisOptionsWarningCode.INCLUDE_FILE_NOT_FOUND", mensagem: "The include file '{0}' in '{1}' can't be found when analyzing '{2}'.", correcao: None, tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false };
    pub static INCOMPATIBLE_LINT: CodigoNaoDart = CodigoNaoDart { nome: "incompatible_lint", unico: "AnalysisOptionsWarningCode.INCOMPATIBLE_LINT", mensagem: "The rule '{0}' is incompatible with the rule '{1}'.", correcao: Some("Try removing one of the incompatible rules."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false };
    pub static INVALID_OPTION: CodigoNaoDart = CodigoNaoDart { nome: "invalid_option", unico: "AnalysisOptionsWarningCode.INVALID_OPTION", mensagem: "Invalid option specified for '{0}': {1}", correcao: None, tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false };
    pub static INVALID_SECTION_FORMAT: CodigoNaoDart = CodigoNaoDart { nome: "invalid_section_format", unico: "AnalysisOptionsWarningCode.INVALID_SECTION_FORMAT", mensagem: "Invalid format for the '{0}' section.", correcao: None, tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false };
    pub static MULTIPLE_PLUGINS: CodigoNaoDart = CodigoNaoDart { nome: "multiple_plugins", unico: "AnalysisOptionsWarningCode.MULTIPLE_PLUGINS", mensagem: "Multiple plugins can't be enabled.", correcao: Some("Remove all plugins following the first, '{0}'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false };
    pub static RECURSIVE_INCLUDE_FILE: CodigoNaoDart = CodigoNaoDart { nome: "recursive_include_file", unico: "AnalysisOptionsWarningCode.RECURSIVE_INCLUDE_FILE", mensagem: "The include file '{0}' in '{1}' includes itself recursively.", correcao: Some("Try changing the chain of 'include's to not re-include this file."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false };
    pub static REMOVED_LINT: CodigoNaoDart = CodigoNaoDart { nome: "removed_lint", unico: "AnalysisOptionsWarningCode.REMOVED_LINT", mensagem: "'{0}' was removed in Dart '{1}'", correcao: Some("Remove the reference to '{0}'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false };
    pub static REPLACED_LINT: CodigoNaoDart = CodigoNaoDart { nome: "replaced_lint", unico: "AnalysisOptionsWarningCode.REPLACED_LINT", mensagem: "'{0}' was replaced by '{2}' in Dart '{1}'.", correcao: Some("Replace '{0}' with '{1}'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false };
    pub static UNDEFINED_LINT: CodigoNaoDart = CodigoNaoDart { nome: "undefined_lint", unico: "AnalysisOptionsWarningCode.UNDEFINED_LINT", mensagem: "'{0}' is not a recognized lint rule.", correcao: Some("Try using the name of a recognized lint rule."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false };
    pub static UNRECOGNIZED_ERROR_CODE: CodigoNaoDart = CodigoNaoDart { nome: "unrecognized_error_code", unico: "AnalysisOptionsWarningCode.UNRECOGNIZED_ERROR_CODE", mensagem: "'{0}' isn't a recognized error code.", correcao: None, tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false };
    pub static UNSUPPORTED_OPTION_WITHOUT_VALUES: CodigoNaoDart = CodigoNaoDart { nome: "unsupported_option_without_values", unico: "AnalysisOptionsWarningCode.UNSUPPORTED_OPTION_WITHOUT_VALUES", mensagem: "The option '{1}' isn't supported by '{0}'.", correcao: None, tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false };
    pub static UNSUPPORTED_OPTION_WITH_LEGAL_VALUE: CodigoNaoDart = CodigoNaoDart { nome: "unsupported_option_with_legal_value", unico: "AnalysisOptionsWarningCode.UNSUPPORTED_OPTION_WITH_LEGAL_VALUE", mensagem: "The option '{1}' isn't supported by '{0}'. Try using the only supported option: '{2}'.", correcao: None, tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false };
    pub static UNSUPPORTED_OPTION_WITH_LEGAL_VALUES: CodigoNaoDart = CodigoNaoDart { nome: "unsupported_option_with_legal_values", unico: "AnalysisOptionsWarningCode.UNSUPPORTED_OPTION_WITH_LEGAL_VALUES", mensagem: "The option '{1}' isn't supported by '{0}'.", correcao: Some("Try using one of the supported options: {2}."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false };
    pub static UNSUPPORTED_VALUE: CodigoNaoDart = CodigoNaoDart { nome: "unsupported_value", unico: "AnalysisOptionsWarningCode.UNSUPPORTED_VALUE", mensagem: "The value '{1}' isn't supported by '{0}'.", correcao: Some("Try using one of the supported options: {2}."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false };
}

pub mod pubspec {
    use super::*;
    pub static ASSET_DIRECTORY_DOES_NOT_EXIST: CodigoNaoDart = CodigoNaoDart { nome: "asset_directory_does_not_exist", unico: "PubspecWarningCode.ASSET_DIRECTORY_DOES_NOT_EXIST", mensagem: "The asset directory '{0}' doesn't exist.", correcao: Some("Try creating the directory or fixing the path to the directory."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true };
    pub static ASSET_DOES_NOT_EXIST: CodigoNaoDart = CodigoNaoDart { nome: "asset_does_not_exist", unico: "PubspecWarningCode.ASSET_DOES_NOT_EXIST", mensagem: "The asset file '{0}' doesn't exist.", correcao: Some("Try creating the file or fixing the path to the file."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true };
    pub static ASSET_FIELD_NOT_LIST: CodigoNaoDart = CodigoNaoDart { nome: "asset_field_not_list", unico: "PubspecWarningCode.ASSET_FIELD_NOT_LIST", mensagem: "The value of the 'assets' field is expected to be a list of relative file paths.", correcao: Some("Try converting the value to be a list of relative file paths."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true };
    pub static ASSET_MISSING_PATH: CodigoNaoDart = CodigoNaoDart { nome: "asset_missing_path", unico: "PubspecWarningCode.ASSET_MISSING_PATH", mensagem: "Asset map entry must contain a 'path' field.", correcao: Some("Try adding a 'path' field."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true };
    pub static ASSET_NOT_STRING: CodigoNaoDart = CodigoNaoDart { nome: "asset_not_string", unico: "PubspecWarningCode.ASSET_NOT_STRING", mensagem: "Assets are required to be file paths (strings).", correcao: Some("Try converting the value to be a string."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true };
    pub static ASSET_NOT_STRING_OR_MAP: CodigoNaoDart = CodigoNaoDart { nome: "asset_not_string_or_map", unico: "PubspecWarningCode.ASSET_NOT_STRING_OR_MAP", mensagem: "An asset value is required to be a file path (string) or map.", correcao: Some("Try converting the value to be a string or map."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true };
    pub static ASSET_PATH_NOT_STRING: CodigoNaoDart = CodigoNaoDart { nome: "asset_path_not_string", unico: "PubspecWarningCode.ASSET_PATH_NOT_STRING", mensagem: "Asset paths are required to be file paths (strings).", correcao: Some("Try converting the value to be a string."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true };
    pub static DEPENDENCIES_FIELD_NOT_MAP: CodigoNaoDart = CodigoNaoDart { nome: "dependencies_field_not_map", unico: "PubspecWarningCode.DEPENDENCIES_FIELD_NOT_MAP", mensagem: "The value of the '{0}' field is expected to be a map.", correcao: Some("Try converting the value to be a map."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true };
    pub static DEPRECATED_FIELD: CodigoNaoDart = CodigoNaoDart { nome: "deprecated_field", unico: "PubspecWarningCode.DEPRECATED_FIELD", mensagem: "The '{0}' field is no longer used and can be removed.", correcao: Some("Try removing the field."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true };
    pub static FLUTTER_FIELD_NOT_MAP: CodigoNaoDart = CodigoNaoDart { nome: "flutter_field_not_map", unico: "PubspecWarningCode.FLUTTER_FIELD_NOT_MAP", mensagem: "The value of the 'flutter' field is expected to be a map.", correcao: Some("Try converting the value to be a map."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true };
    pub static INVALID_DEPENDENCY: CodigoNaoDart = CodigoNaoDart { nome: "invalid_dependency", unico: "PubspecWarningCode.INVALID_DEPENDENCY", mensagem: "Publishable packages can't have '{0}' dependencies.", correcao: Some("Try adding a 'publish_to: none' entry to mark the package as not for publishing or remove the {0} dependency."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true };
    pub static INVALID_PLATFORMS_FIELD: CodigoNaoDart = CodigoNaoDart { nome: "invalid_platforms_field", unico: "PubspecWarningCode.INVALID_PLATFORMS_FIELD", mensagem: "The 'platforms' field must be a map with platforms as keys.", correcao: Some("Try changing the 'platforms' field to a map with platforms as keys."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true };
    pub static MISSING_DEPENDENCY: CodigoNaoDart = CodigoNaoDart { nome: "missing_dependency", unico: "PubspecWarningCode.MISSING_DEPENDENCY", mensagem: "Missing a dependency on imported package '{0}'.", correcao: Some("Try adding {0}."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true };
    pub static MISSING_NAME: CodigoNaoDart = CodigoNaoDart { nome: "missing_name", unico: "PubspecWarningCode.MISSING_NAME", mensagem: "The 'name' field is required but missing.", correcao: Some("Try adding a field named 'name'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true };
    pub static NAME_NOT_STRING: CodigoNaoDart = CodigoNaoDart { nome: "name_not_string", unico: "PubspecWarningCode.NAME_NOT_STRING", mensagem: "The value of the 'name' field is required to be a string.", correcao: Some("Try converting the value to be a string."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true };
    pub static PATH_DOES_NOT_EXIST: CodigoNaoDart = CodigoNaoDart { nome: "path_does_not_exist", unico: "PubspecWarningCode.PATH_DOES_NOT_EXIST", mensagem: "The path '{0}' doesn't exist.", correcao: Some("Try creating the referenced path or using a path that exists."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true };
    pub static PATH_NOT_POSIX: CodigoNaoDart = CodigoNaoDart { nome: "path_not_posix", unico: "PubspecWarningCode.PATH_NOT_POSIX", mensagem: "The path '{0}' isn't a POSIX-style path.", correcao: Some("Try converting the value to a POSIX-style path."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true };
    pub static PATH_PUBSPEC_DOES_NOT_EXIST: CodigoNaoDart = CodigoNaoDart { nome: "path_pubspec_does_not_exist", unico: "PubspecWarningCode.PATH_PUBSPEC_DOES_NOT_EXIST", mensagem: "The directory '{0}' doesn't contain a pubspec.", correcao: Some("Try creating a pubspec in the referenced directory or using a path that has a pubspec."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true };
    pub static PLATFORM_VALUE_DISALLOWED: CodigoNaoDart = CodigoNaoDart { nome: "platform_value_disallowed", unico: "PubspecWarningCode.PLATFORM_VALUE_DISALLOWED", mensagem: "Keys in the `platforms` field can't have values.", correcao: Some("Try removing the value, while keeping the key."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true };
    pub static UNKNOWN_PLATFORM: CodigoNaoDart = CodigoNaoDart { nome: "unknown_platform", unico: "PubspecWarningCode.UNKNOWN_PLATFORM", mensagem: "The platform '{0}' is not a recognized platform.", correcao: Some("Try correcting the platform name or removing it."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true };
    pub static UNNECESSARY_DEV_DEPENDENCY: CodigoNaoDart = CodigoNaoDart { nome: "unnecessary_dev_dependency", unico: "PubspecWarningCode.UNNECESSARY_DEV_DEPENDENCY", mensagem: "The dev dependency on {0} is unnecessary because there is also a normal dependency on that package.", correcao: Some("Try removing the dev dependency."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true };
    pub static WORKSPACE_FIELD_NOT_LIST: CodigoNaoDart = CodigoNaoDart { nome: "workspace_field_not_list", unico: "PubspecWarningCode.WORKSPACE_FIELD_NOT_LIST", mensagem: "The value of the 'workspace' field is required to be a list of relative file paths.", correcao: Some("Try converting the value to be a list of relative file paths."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false };
    pub static WORKSPACE_VALUE_NOT_STRING: CodigoNaoDart = CodigoNaoDart { nome: "workspace_value_not_string", unico: "PubspecWarningCode.WORKSPACE_VALUE_NOT_STRING", mensagem: "Workspace entries are required to be directory paths (strings).", correcao: Some("Try converting the value to be a string."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false };
    pub static WORKSPACE_VALUE_NOT_SUBDIRECTORY: CodigoNaoDart = CodigoNaoDart { nome: "workspace_value_not_subdirectory", unico: "PubspecWarningCode.WORKSPACE_VALUE_NOT_SUBDIRECTORY", mensagem: "Workspace values must be a relative path of a subdirectory of '{0}'.", correcao: Some("Try using a subdirectory of the directory containing the 'pubspec.yaml' file."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false };
}

pub mod manifesto {
    use super::*;
    pub static CAMERA_PERMISSIONS_INCOMPATIBLE: CodigoNaoDart = CodigoNaoDart { nome: "camera_permissions_incompatible", unico: "ManifestWarningCode.CAMERA_PERMISSIONS_INCOMPATIBLE", mensagem: "Camera permissions make app incompatible for Chrome OS, consider adding optional features \"android.hardware.camera\" and \"android.hardware.camera.autofocus\".", correcao: Some("Try adding `<uses-feature android:name=\"android.hardware.camera\"  android:required=\"false\">` `<uses-feature android:name=\"android.hardware.camera.autofocus\"  android:required=\"false\">`."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false };
    pub static NON_RESIZABLE_ACTIVITY: CodigoNaoDart = CodigoNaoDart { nome: "non_resizable_activity", unico: "ManifestWarningCode.NON_RESIZABLE_ACTIVITY", mensagem: "The `<activity>` element should be allowed to be resized to allow users to take advantage of the multi-window environment on Chrome OS", correcao: Some("Consider declaring the corresponding activity element with `resizableActivity=\"true\"` attribute."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false };
    pub static NO_TOUCHSCREEN_FEATURE: CodigoNaoDart = CodigoNaoDart { nome: "no_touchscreen_feature", unico: "ManifestWarningCode.NO_TOUCHSCREEN_FEATURE", mensagem: "The default \"android.hardware.touchscreen\" needs to be optional for Chrome OS. ", correcao: Some("Consider adding <uses-feature android:name=\"android.hardware.touchscreen\" android:required=\"false\" /> to the manifest."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false };
    pub static PERMISSION_IMPLIES_UNSUPPORTED_HARDWARE: CodigoNaoDart = CodigoNaoDart { nome: "permission_implies_unsupported_hardware", unico: "ManifestWarningCode.PERMISSION_IMPLIES_UNSUPPORTED_HARDWARE", mensagem: "Permission makes app incompatible for Chrome OS, consider adding optional {0} feature tag, ", correcao: Some(" Try adding `<uses-feature android:name=\"{0}\"  android:required=\"false\">`."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false };
    pub static SETTING_ORIENTATION_ON_ACTIVITY: CodigoNaoDart = CodigoNaoDart { nome: "setting_orientation_on_activity", unico: "ManifestWarningCode.SETTING_ORIENTATION_ON_ACTIVITY", mensagem: "The `<activity>` element should not be locked to any orientation so that users can take advantage of the multi-window environments and larger screens on Chrome OS", correcao: Some("Consider declaring the corresponding activity element with `screenOrientation=\"unspecified\"` or `\"fullSensor\"` attribute."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false };
    pub static UNSUPPORTED_CHROME_OS_FEATURE: CodigoNaoDart = CodigoNaoDart { nome: "unsupported_chrome_os_feature", unico: "ManifestWarningCode.UNSUPPORTED_CHROME_OS_FEATURE", mensagem: "The feature {0} isn't supported on Chrome OS, consider making it optional.", correcao: Some("Try changing to `android:required=\"false\"` for this feature."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false };
    pub static UNSUPPORTED_CHROME_OS_HARDWARE: CodigoNaoDart = CodigoNaoDart { nome: "unsupported_chrome_os_hardware", unico: "ManifestWarningCode.UNSUPPORTED_CHROME_OS_HARDWARE", mensagem: "The feature {0} isn't supported on Chrome OS, consider making it optional.", correcao: Some("Try adding `android:required=\"false\"` for this feature."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false };
}

/// Todos os códigos de arquivos não-Dart, na ordem da fonte.
pub static TODOS: [&CodigoNaoDart; 53] = [
    &opcoes::INCLUDED_FILE_PARSE_ERROR,
    &opcoes::PARSE_ERROR,
    &opcoes::DEPRECATED_LINT,
    &opcoes::DEPRECATED_LINT_WITH_REPLACEMENT,
    &opcoes::DUPLICATE_RULE,
    &opcoes::ANALYSIS_OPTION_DEPRECATED,
    &opcoes::ANALYSIS_OPTION_DEPRECATED_WITH_REPLACEMENT,
    &opcoes::INCLUDED_FILE_WARNING,
    &opcoes::INCLUDE_FILE_NOT_FOUND,
    &opcoes::INCOMPATIBLE_LINT,
    &opcoes::INVALID_OPTION,
    &opcoes::INVALID_SECTION_FORMAT,
    &opcoes::MULTIPLE_PLUGINS,
    &opcoes::RECURSIVE_INCLUDE_FILE,
    &opcoes::REMOVED_LINT,
    &opcoes::REPLACED_LINT,
    &opcoes::UNDEFINED_LINT,
    &opcoes::UNRECOGNIZED_ERROR_CODE,
    &opcoes::UNSUPPORTED_OPTION_WITHOUT_VALUES,
    &opcoes::UNSUPPORTED_OPTION_WITH_LEGAL_VALUE,
    &opcoes::UNSUPPORTED_OPTION_WITH_LEGAL_VALUES,
    &opcoes::UNSUPPORTED_VALUE,
    &pubspec::ASSET_DIRECTORY_DOES_NOT_EXIST,
    &pubspec::ASSET_DOES_NOT_EXIST,
    &pubspec::ASSET_FIELD_NOT_LIST,
    &pubspec::ASSET_MISSING_PATH,
    &pubspec::ASSET_NOT_STRING,
    &pubspec::ASSET_NOT_STRING_OR_MAP,
    &pubspec::ASSET_PATH_NOT_STRING,
    &pubspec::DEPENDENCIES_FIELD_NOT_MAP,
    &pubspec::DEPRECATED_FIELD,
    &pubspec::FLUTTER_FIELD_NOT_MAP,
    &pubspec::INVALID_DEPENDENCY,
    &pubspec::INVALID_PLATFORMS_FIELD,
    &pubspec::MISSING_DEPENDENCY,
    &pubspec::MISSING_NAME,
    &pubspec::NAME_NOT_STRING,
    &pubspec::PATH_DOES_NOT_EXIST,
    &pubspec::PATH_NOT_POSIX,
    &pubspec::PATH_PUBSPEC_DOES_NOT_EXIST,
    &pubspec::PLATFORM_VALUE_DISALLOWED,
    &pubspec::UNKNOWN_PLATFORM,
    &pubspec::UNNECESSARY_DEV_DEPENDENCY,
    &pubspec::WORKSPACE_FIELD_NOT_LIST,
    &pubspec::WORKSPACE_VALUE_NOT_STRING,
    &pubspec::WORKSPACE_VALUE_NOT_SUBDIRECTORY,
    &manifesto::CAMERA_PERMISSIONS_INCOMPATIBLE,
    &manifesto::NON_RESIZABLE_ACTIVITY,
    &manifesto::NO_TOUCHSCREEN_FEATURE,
    &manifesto::PERMISSION_IMPLIES_UNSUPPORTED_HARDWARE,
    &manifesto::SETTING_ORIENTATION_ON_ACTIVITY,
    &manifesto::UNSUPPORTED_CHROME_OS_FEATURE,
    &manifesto::UNSUPPORTED_CHROME_OS_HARDWARE,
];
