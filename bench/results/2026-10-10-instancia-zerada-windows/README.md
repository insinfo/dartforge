# Instância zerada Owned com métodos — Windows, 2026-10-10

Fonte registrada em `7ab36e2450bfeb590b4ad8ef3d547651bbb84221`.
Execução baseada em `4fe197ef`, com alterações locais explicitadas no
manifesto. Vínculos registram SHA256 dos arquivos de trabalho e dos blobs
do commit; a assinatura Rust unsafe e a declaração LLVM estão incluídas.

São 16 execuções nativas: ARC/tracing × O0/O2, quatro positivas e 12
controles negativos. Todas as gerações terminaram com sucesso. Positivos
saíram zero com `1\n`; negativos produziram o trap Windows exigido pelo
script, com stdout vazio. A prova confere owner, campo inicial zero,
registro preguiçoso pelo thunk real do emissor, lookup/chamada por seletor
e morte física após uma liberação. Retirar owner, tabela ou release falha.

Há 80 arquivos brutos da matriz e seis logs: biblioteca/exemplo do emissor
(253 + 1 testes), runtime (182), exemplos públicos (52 + 42), matriz e CLI.
O log da CLI cobre oito execuções consultando ARC ativo pela ABI via FFI,
com ambiente ARC herdado: AOT/JIT/reload com/sem reinício usam tracing por
padrão e ARC somente com opção explícita. Os sete/três testes ignorados das
bibliotecas não foram executados; os testes ignorados da CLI foram executados.
`evidencia.json` conserva os hashes dos 86 arquivos. Executáveis não são
arquivados. `.gitattributes` preserva os bytes, inclusive finais de linha.

Reprodução da matriz, com Clang/ligador disponíveis:

```powershell
python scripts/provar-arc-aot.py arc_instancia_zerada --debug
cargo test --locked -p dartforge-cli --features nativo,jit --test memoria_cli -- --include-ignored
```

A construção do exemplo é HIR explícita, na forma da fábrica zerada usada
pelo lowering. Tracing usa tokens explícitos somente no harness. A prova
não conclui preparação automática de owners/cleanup da fonte, erro de
construção parcial, vida semântica de Finalizable, versões/pins, obrigações
restantes dos §§27–34 ou o gate de desempenho ARC/A0. A CI desta fonte ainda
aguarda publicação; a rodada ativa é a anterior, `124624db`.
