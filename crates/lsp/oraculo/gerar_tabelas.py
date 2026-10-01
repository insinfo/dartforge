# Gera crates/lsp/src/relevancia_tabelas.rs a partir de
# pkg/analysis_server/lib/src/services/completion/dart/relevance_tables.g.dart
# (tag 3.6.2 do SDK do Dart).
import re, subprocess

fonte = subprocess.run(
    ['git', '-C', r'E:\references\dart-sdk', 'show',
     '3.6.2:pkg/analysis_server/lib/src/services/completion/dart/relevance_tables.g.dart'],
    capture_output=True, text=True, encoding='utf-8').stdout

i_elem = fonte.index('const defaultElementKindRelevance')
i_kw = fonte.index('const defaultKeywordRelevance')
elem_txt = fonte[i_elem:i_kw]
kw_txt = fonte[i_kw:]

ESPECIES = {
    'CLASS': 'Classe', 'CONSTRUCTOR': 'Construtor', 'ENUM': 'Enum', 'FIELD': 'Campo', 'FUNCTION': 'Funcao',
    'FUNCTION_TYPE_ALIAS': 'AliasDeFuncao', 'LOCAL_VARIABLE': 'Local', 'METHOD': 'Metodo', 'MIXIN': 'Mixin',
    'PARAMETER': 'Parametro', 'PREFIX': 'Prefixo', 'TOP_LEVEL_VARIABLE': 'VariavelDeTopo',
    'TYPE_PARAMETER': 'ParametroDeTipo', 'UNKNOWN': 'Desconhecido',
}


def blocos(txt):
    for m in re.finditer(r"\n  '([^']+)': \{(.*?)\n  \},", txt, re.S):
        yield m.group(1), m.group(2)


saida = ['''//! Tabelas de relevância do completar do Dart 3.6.2, geradas de
//! `pkg/analysis_server/lib/src/services/completion/dart/relevance_tables.g.dart`
//! (tag `3.6.2`) por `crates/lsp/oraculo/gerar_tabelas.py`. Não editar à mão.

use super::relevancia::Especie;

/// Faixa de probabilidade (`ProbabilityRange`) de cada espécie de elemento
/// num local de completar.
pub(crate) fn especies(local: &str) -> &'static [(Especie, f64, f64)] {
    match local {''']
for local, corpo in blocos(elem_txt):
    itens = re.findall(r"ElementKind\.([A-Z_]+):\s*ProbabilityRange\(lower: ([0-9.]+), upper: ([0-9.]+)\)", corpo)
    linhas = ', '.join(f"(Especie::{ESPECIES[k]}, {lo}, {up})" for k, lo, up in itens)
    saida.append(f'        "{local}" => &[{linhas}],')
saida.append('''        _ => &[],
    }
}

/// Faixa de probabilidade de cada palavra-chave num local de completar.
pub(crate) fn palavras(local: &str) -> &'static [(&'static str, f64, f64)] {
    match local {''')
for local, corpo in blocos(kw_txt):
    itens = re.findall(r"'([^']+)':\s*ProbabilityRange\(lower: ([0-9.]+), upper: ([0-9.]+)\)", corpo)
    linhas = ', '.join(f'("{k}", {lo}, {up})' for k, lo, up in itens)
    saida.append(f'        "{local}" => &[{linhas}],')
saida.append('''        _ => &[],
    }
}
''')
open(r'E:\MyRustProjects\dartforge\crates\lsp\src\relevancia_tabelas.rs', 'w', encoding='utf-8').write('\n'.join(saida))
print('ok')
