# Contratos iniciais de ABI, FFI e WebAssembly

> O crate `dartforge-abi` e o comando `abi-info` continuam. O caminho
> `@Native` do incremento 16 e o driver AOT citados abaixo eram da trilha
> velha (preservada na branch `exploracao-inicial`); o AOT atual é o
> `emit_native`.

**Atualização:** o [incremento 16](historico/IMPLEMENTACAO-16.md) implementa o caminho
@Native/external escalar com ligação de objetos. As restrições abaixo descrevem
a infraestrutura inicial do incremento 11; ponteiros e FFI dinâmico continuam pendentes.

Alvo de linguagem: Dart **3.6.2**. A crate `dartforge-abi` é infraestrutura inicial;
não interpreta imports `dart:ffi`, não carrega bibliotecas nem expõe ponteiros no Dart.
O driver AOT continua compilando somente para o host.

## Implementado

Perfis `windows-x64`, `linux-x64` e `wasm32` descrevem triple, largura dos ponteiros
e layout escalar. `abi-info` expõe JSON com capacidades de execução explicitamente
falsas. `Signature` valida a correspondência entre tipos Dart e marcadores C
`Void`, `Int32`, `Int64`, `Double` e `Pointer`, rejeitando void em parâmetros e
objetos gerenciados usados como endereços. A presença de Double neste contrato
não adiciona `double` ao frontend Dart atual.

A emissão de declarações LLVM aceita apenas identificadores C ASCII e reserva os
prefixos internos. O teste `native_call` liga IR LLVM a uma função C e verifica
argumentos ponteiro, double, int32 e int64 pela execução do resultado. O mesmo IR
é emitido como objeto wasm32 e seu cabeçalho é verificado. Não é um teste de
execução WebAssembly, de resolução de imports ou de compilação Dart para Wasm.

## Próximos passos e invariantes

1. Resolver `dart:ffi`, tipos nativos e assinaturas com IDs na IR.
2. Implementar lookup, ligação, memória externa e lifetime explícito de ponteiros.
3. Cobrir structs/unions, alinhamento, callbacks, varargs e regras de cada ABI.
4. Definir runtime e imports wasm32; executar testes em engine WebAssembly.

Handles do GC são índices i64 e **não** endereços C. Enums canônicos continuam
objetos gerenciados. Nenhum desses handles deve ser reinterpretado como Pointer.
O perfil wasm32 rejeita o contrato de biblioteca dinâmica nativa: precisará de
imports e mecanismo de ligação próprios. Os cinco tipos escalares não descrevem
toda uma ABI e não autorizam inferir suporte a plataformas não testadas.

## Referências

SDK fixado em `3.6.2` (b0cc5495e0f5e8ae150825a5352e708cb49e65ff):
[sdk/lib/ffi](https://github.com/dart-lang/sdk/tree/3.6.2/sdk/lib/ffi), especialmente
`native_type.dart`, `abi.dart` e `dynamic_library.dart`.
Os clones locais são referência, não dependência de build. Código próprio sob MIT.
