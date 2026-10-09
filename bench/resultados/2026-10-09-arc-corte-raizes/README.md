# Corte de raízes protegidas no trial ARC

Diagnóstico Windows sobre o caso `corpus/nativo/60_closures_de_ambiente_direto.dart`,
reduzido de 20 mil para mil iterações e com acesso `fs[123]`. Os executáveis
foram compilados em AOT, sem otimização de produção, pelo harness com LLVM
embutido. Antes: fontes de `ac010711` (runtime de `ef122930`); depois: apenas
`alteracao.patch`. Não é a comparação ARC/A0 dos 32 kernels de desempenho.

Sete pares alternados, processos novos, afinidade `0x4`, sem outro teste local
ativo. Ambos usam estresse, ciclos em toda drenagem e rastro; auditoria desligada.
Os 14 processos retornaram zero e produziram stdout idêntico. Dados brutos,
incluindo stderr, hashes e configuração, estão em `amostras.json.gz`.

| Mediana | Antes | Depois |
| --- | ---: | ---: |
| Processo, segundos (inclui rastro) | 1,056755 | 0,348021 |
| Drenagens registradas, µs acumulados | 846.023 | 153.583 |
| Trial, µs acumulados | 836.393 | 144.988 |
| Nós examinados ao final | 8.757.350 | 1.351.029 |
| Drenagens por execução | 4.039 | 4.037 |

O corte conserva a raiz candidata e deixa suas arestas como entradas externas
na região. A queda de trabalho neste diagnóstico não demonstra ARC ≥ A0.
O caso original continua excedendo o limite de 60 s, tanto com auditoria
quanto sem ela; esses timeouts permanecem falhas.

`comparar.py` repete as medidas com os executáveis preservados em
`target/diferencial/nativo/60_closures_perfil/`: `60_closures_perfil-antes.exe`
e `60_closures_perfil.exe`. Requer suas DLLs do SDK no mesmo diretório.
O executável anterior tem SHA-256
`D83FF48FC984AE65887625B8563C5183E0134211AF80C2A156600FBA80B697F0`;
o posterior, `F714D0174B226BBA5EBFA058721809C761D31B18B6F6D062EEAA6EA5E10B560A`.
Os binários não fazem parte deste registro. `diagnostico.dart` preserva a
entrada efetiva, e `resultado.txt` as medianas calculadas dos dados brutos.
