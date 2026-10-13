# Generates crates/lsp/src/relevance_tables.rs from
# pkg/analysis_server/lib/src/services/completion/dart/relevance_tables.g.dart
# (tag 3.6.2 of the Dart SDK).
import re, subprocess

source = subprocess.run(
    ['git', '-C', r'E:\references\dart-sdk', 'show',
     '3.6.2:pkg/analysis_server/lib/src/services/completion/dart/relevance_tables.g.dart'],
    capture_output=True, text=True, encoding='utf-8').stdout

i_elem = source.index('const defaultElementKindRelevance')
i_kw = source.index('const defaultKeywordRelevance')
elem_txt = source[i_elem:i_kw]
kw_txt = source[i_kw:]

KINDS = {
    'CLASS': 'Class', 'CONSTRUCTOR': 'Constructor', 'ENUM': 'Enum', 'FIELD': 'Field', 'FUNCTION': 'Function',
    'FUNCTION_TYPE_ALIAS': 'FunctionAlias', 'LOCAL_VARIABLE': 'Local', 'METHOD': 'Method', 'MIXIN': 'Mixin',
    'PARAMETER': 'Parameter', 'PREFIX': 'Prefix', 'TOP_LEVEL_VARIABLE': 'TopLevelVariable',
    'TYPE_PARAMETER': 'TypeParameter', 'UNKNOWN': 'Unknown',
}


def blocks(txt):
    for m in re.finditer(r"\n  '([^']+)': \{(.*?)\n  \},", txt, re.S):
        yield m.group(1), m.group(2)


output = ['''//! Dart 3.6.2 completion relevance tables, generated from
//! `pkg/analysis_server/lib/src/services/completion/dart/relevance_tables.g.dart`
//! (tag `3.6.2`) by `crates/lsp/oracle/generate_tables.py`. Do not edit by hand.

use super::relevance::Kind;

/// Probability range (`ProbabilityRange`) of each element kind at a
/// completion location.
pub(crate) fn kinds(local: &str) -> &'static [(Kind, f64, f64)] {
    match local {''']
for local, body in blocks(elem_txt):
    items = re.findall(r"ElementKind\.([A-Z_]+):\s*ProbabilityRange\(lower: ([0-9.]+), upper: ([0-9.]+)\)", body)
    lines = ', '.join(f"(Kind::{KINDS[k]}, {lo}, {up})" for k, lo, up in items)
    output.append(f'        "{local}" => &[{lines}],')
output.append('''        _ => &[],
    }
}

/// Probability range of each keyword at a completion location.
pub(crate) fn words(local: &str) -> &'static [(&'static str, f64, f64)] {
    match local {''')
for local, body in blocks(kw_txt):
    items = re.findall(r"'([^']+)':\s*ProbabilityRange\(lower: ([0-9.]+), upper: ([0-9.]+)\)", body)
    lines = ', '.join(f'("{k}", {lo}, {up})' for k, lo, up in items)
    output.append(f'        "{local}" => &[{lines}],')
output.append('''        _ => &[],
    }
}
''')
open(r'E:\MyRustProjects\dartforge\crates\lsp\src\relevance_tables.rs', 'w', encoding='utf-8').write('\n'.join(output))
print('ok')
