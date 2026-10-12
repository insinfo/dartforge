# Execução AOT de três caminhos de erro com cleanup automático

Base `359c1a48`, acrescentando a ABI tipada `dartforge_arc_lancar_ref_v1`,
contratos de lançamento/clear e variantes `retorno-mortal-erro-1`, `-2`, `-3`.
O exemplo substitui a observação escolhida por lançamento Ref, confere a
pendência e limpa o global e a exceção antes dos drops inseridos pelo passe.

Para cada modo (`arc`, `tracing`) e índice (1, 2, 3):

```powershell
cargo run --locked -p dartforge-emit-native --example arc_slots_fortes -- target/prova-excecao-ref-arc-1.exe arc retorno-mortal-erro-1
$env:DARTFORGE_ARC_CONFERIR='1'
$env:DARTFORGE_ARC_BERCARIO='0'
$env:DARTFORGE_GC_STRESS='1'
& target/prova-excecao-ref-arc-1.exe
```

As seis execuções terminaram com código 0. Índice 1: stdout vazio;
índice 2: `9223372036854775807`; índice 3: esse Mint e `42`.
O caminho normal imprimiria também `null`; a ausência das linhas posteriores
confere a seleção do retorno antecipado. As saídas foram conferidas antes
de copiar os bytes e registrar SHA-256 em `evidencias.json`.

O passe exige exatamente zero retenções e três liberações nesta variante.
Nos seis LLVM, o bloco selecionado b1/b3/b5 limpa a exceção e libera
respectivamente v8/v14/v18 antes do retorno. Exes permanecem em `target`.

O lançamento recebe Mint, Smi e null em representação Ref. O terceiro caso
testa o protocolo interno de pendência; não certifica a semântica Dart de
`throw null`, que continua responsabilidade do lowering.

O teste do runtime confirma em ARC e tracing que a raiz da exceção mantém
o Mint vivo após release do código e que clear seguido de coleta permite
sua morte. O AOT não observa essa morte final nem certifica ausência geral
de vazamentos. Também não conclui integração no pipeline padrão, invoke
de callee Dart nativo, finally, cancelamento ou suspensão.
