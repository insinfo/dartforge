# Resolução de packages — Dart 3.6.2

## Referências consultadas antes da implementação

- Especificação references/dart-language/accepted/2.8/language-versioning/package-config-file-v2.md, revisão 9cdc5a5e8ecfbedca4e10d3ae880e82258dc346b, versão textual 1.1: rootUri relativo à URI da configuração; packageUri relativa à raiz e opcional; nomes/versões/overlaps inválidos; busca ascendente .dart_tool/package_config.json.
- Implementação local C:/Users/pmro/AppData/Local/Pub/Cache/hosted/pub.dev/package_config-2.2.0/lib/src/package_config_json.dart, SHA256 3542ABCBCA87E571BA3A3AE7E25FEDBADC0DAF90A991BD47410896820329A8B2: parsePackageConfigJson/parsePackage resolve URI e acrescenta / depois de resolver. Versão do pacote 2.2.0.
- SDK clonado references/dart-sdk, revisão 6c4009687d2c881f880127fc12b9ad7c72111d33, tools/VERSION **3.14.0 main**, não confundir com alvo.
- Oracle executável C:/tools/dartsdk-3.6.2/bin/dart.exe, versão 3.6.2 stable.
- Combinadores: SDK tests/language/import/combinators_test.dart, tests/language/export/cyclic_test.dart; oracle 3.6.2 aceita combinadores repetidos sequenciais (show/interseção). Experimentos single_combinators novos não se aplicam.

## Decisões

A API load autodetecta a configuração mais próxima dos ancestrais da entrada e a relê em toda execução. load_with_config permite caminho explícito. SourceGraph conserva fontes e caminhos canônicos; imports e exports possuem combinadores ordenados e igualdade estrutural, para cache comparar o grafo resolvido.

Usamos serde_json para JSON e url para resolução de URIs file, percent-encoding e caminhos Windows; não concatenamos rootUri/packageUri como paths. packageUri ausente significa raiz, não lib/. Diretórios resolvidos recebem barra final. Esquemas remotos, queries, fragments, escapes Dart na URI e formatos .packages legados continuam diagnosticados. Imports condicionais e prefixos `as` são suportados; um pacote ausente nomeia o pacote **e** o arquivo de configuração consultado, e uma biblioteca `dart:` fora do subconjunto nomeia a biblioteca pedida, conforme [IMPORTS.md](IMPORTS.md).

O carregador não transfere arquivos nem executa pub get. Configurações v2 inválidas, nomes duplicados, raízes sobrepostas incompatíveis e versões de linguagem fora do subconjunto suportado são rejeitados. Resolução de exports e combinadores em namespaces pertence ao linker.

## Verificação com o alvo

Executado o oracle Dart 3.6.2 em target/package-oracle: rootUri dep%20espa%C3%A7o sem barra e packageUri lib sem barra resolvem a função que imprime 42. Omitir packageUri e apontar rootUri diretamente a lib também imprime 42. Não se presume lib quando packageUri está ausente.

O subconjunto valida languageVersion omitida ou exatamente 3.6; versões anteriores/futuras recebem diagnóstico até haver semântica verificada. Diretivas de fonte // @dart também passam por esse limite. Não há compatibilidade automática com modo legado sem null safety.

A revisão estável do SDK está disponível no clone como tag 3.6.2, commit b0cc5495e0f5e8ae150825a5352e708cb49e65ff. O HEAD 3.14 não foi usado como autoridade para versão de linguagem.

Marcadores de linguagem: consultado git show 3.6.2:pkg/_fe_analyzer_shared/lib/src/scanner/abstract_scanner.dart, método tokenizeLanguageVersionOrSingleLineComment. A validação pública validate_language_version examina apenas o preâmbulo léxico, respeita comentários de bloco aninhados, ignora doc comments /// e texto posterior à declaração. Marcadores malformados ou com texto extra são comentários comuns como no scanner de referência.

Validação concluída: 11 testes unitários packages (configuração, remapeamento, file URI Windows, percentencoding/espaços/Unicode, nesting, combinadores, versões), 3 doctests e Clippy -D warnings. A dependência url 2.5.8 ficou resolvida em Cargo.lock, junto de serde_json.
