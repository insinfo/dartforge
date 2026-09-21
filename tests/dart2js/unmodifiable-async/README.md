# dart2js: `TypeError: Cannot set properties of undefined (setting '$flags')`

Divergência de comportamento entre backends oficiais do Dart, reproduzida e
verificada nesta máquina **no Dart 3.6.2** e **já corrigida no 3.13.4**. Em
3.6.2 o mesmo programa roda correto na Dart VM e falha em tempo de execução
quando compilado com `dart compile js`, com e sem `--minify`.

Origem: travamento real em produção num conversor Quill Delta → PDF
(`PdfService.blockGenerators`), onde `Paragraph.lines` era
`List<Line>.unmodifiable(_lines)` e um método `async` iterava
`paragraph.lines` com `await` dentro do laço.

Este diretório não faz parte da suíte de conformidade do DartForge: é
**evidência sobre o dart2js**. O valor para o projeto está registrado na seção
final.

## Arquivos

| arquivo | descrição |
| --- | --- |
| `main.dart` | a reprodução, sem pacotes |
| `main_fixed.dart` | o mesmo código com o getter devolvendo `UnmodifiableListView`; não falha |
| `run.js` | runner Node; a saída do dart2js mira o navegador e sem os ganchos o erro assíncrono é engolido |

## Reprodução

```sh
dart run main.dart
# processed lines: 2

dart compile js --minify -o bug.js main.dart
node run.js bug.js
# UNCAUGHT: Error: Cannot set properties of undefined (setting '$flags')

dart compile js -o bug_unminified.js main.dart
node run.js bug_unminified.js
# UNCAUGHT: Error: Cannot set properties of undefined (setting '$flags')

dart compile js --minify -o bug_fixed.js main_fixed.dart
node run.js bug_fixed.js
# processed lines: 2
```

## Causa, confirmada no código gerado

`List.unmodifiable` é inlinado pelo dart2js como "cópia + `$flags = 3`"
(`ArrayFlags.unmodifiable`). Dentro da máquina de estados do `async` o
compilador reaproveita um único temporário (`result`) para várias cópias
inlinadas do getter e emite um `result.$flags = 3` pendurado num caminho de
junção onde `result` nunca foi atribuído.

Trecho literal de `bug_unminified.js` (Dart 3.6.2), em
`blockGenerators$body$FakePdfService`:

```js
case 3:
  // for condition
  paragraph = paragraphs[_i];
  blockAttributes = A.LinkedHashMap_LinkedHashMap$_empty(t3, t4);
  if (paragraph.type === B.ParagraphType_1) {
    result = A.List_List$from(paragraph._lines, false, t1);   // única atribuição
    result.$flags = 3;
    t5 = B.JSArray_methods.get$first(result).attributes.containsKey$1("embed");
  } else
    t5 = false;
  $async$goto = t5 ? 6 : 7;
  break;
...
case 7:
  // join
  isHeader = blockAttributes.containsKey$1("header");
  isCodeBlock = blockAttributes.containsKey$1("code-block");
  result.$flags = 3;            // <<< result é undefined quando type != embed
  t5 = paragraph._lines, t6 = !isHeader, k = 0;
```

No primeiro parágrafo cujo `type != ParagraphType.embed` o ramo `else` deixa
`result` sem atribuição, e a junção do `case 7` executa `result.$flags = 3`
sobre `undefined`. Se uma iteração anterior tiver atribuído `result`, a escrita
cai num array obsoleto e nada visível acontece — por isso o travamento em
produção parecia intermitente e dependente do conteúdo.

O mesmo trecho mostra um segundo problema, de eficiência e não de correção: no
`case 9` o getter é inlinado **duas vezes por iteração**, uma na condição do
laço e outra no corpo, cada uma copiando a lista inteira.

## Contornos

- Devolver `UnmodifiableListView(_lista)` — uma visão, sem `$flags` — em vez de
  `List.unmodifiable(_lista)` em getters usados dentro de métodos `async`; ou
- capturar o resultado do getter numa variável local antes do laço, em vez de
  reler `paragraph.lines` na condição e no corpo.

## Por que isto está no repositório do DartForge

O [PLANO](../../../PLANO.md) exige suites diferenciais que executem a mesma
entrada Dart em compiladores oficiais e no DartForge, comparando saída, erros e
comportamento observável. Este caso é um lembrete concreto de dois princípios
já registrados:

1. **Correção antes de alegações de velocidade.** Um backend que reaproveita
   temporários entre cópias inlinadas precisa provar que cada caminho de fluxo
   atribui o temporário antes de escrevê-lo. Quando o DartForge implementar
   inlining e reuso de slots — o incremento 9 já reaproveita slots estáticos
   para locais e temporários — este programa entra na suíte de regressão.
2. **Não representar coleções Dart por equivalentes JS sem testes semânticos.**
   `List.unmodifiable` não é uma cópia comum: carrega um contrato de
   imutabilidade que o backend materializa numa marca no array.

O DartForge ainda não compila esta entrada: faltam `async` com laços, enums com
membros, `Map`, `UnmodifiableListView` e interpolação. Enquanto isso, o arquivo
fica como caso pendente da suíte diferencial.

## Resultado em duas versões do SDK

| SDK | VM | dart2js `--minify` | dart2js sem `--minify` |
| --- | --- | --- | --- |
| 3.6.2 (stable, 2025-01-29) | `processed lines: 2` | **TypeError** | **TypeError** |
| 3.13.4 (stable, 2026-09-15) | `processed lines: 2` | `processed lines: 2` | `processed lines: 2` |

**O bug já está corrigido no stable mais recente.** Não se trata do repro ter
perdido a forma: o JavaScript gerado pelo 3.13.4 mantém a mesma estrutura, os
mesmos nomes de temporários e os mesmos blocos `case`. O que mudou é exatamente
a instrução defeituosa.

3.6.2, bloco de junção:

```js
case 7:
  // join
  isHeader = blockAttributes.containsKey$1("header");
  isCodeBlock = blockAttributes.containsKey$1("code-block");
  result.$flags = 3;            // escrita órfã, sem atribuição no caminho else
  t5 = paragraph._lines, t6 = !isHeader, k = 0;
```

3.13.4, o mesmo bloco:

```js
case 7:
  // join
  isHeader = blockAttributes.containsKey$1("header");
  isCodeBlock = blockAttributes.containsKey$1("code-block");
  t5 = paragraph._lines;
  t6 = !isHeader;
  k = 0;
```

Em 3.6.2 a escrita de `$flags` da cópia usada na condição do laço tinha sido
deslocada para a junção, longe da alocação correspondente. Em 3.13.4 cada
escrita fica imediatamente após a sua alocação:

```js
case 9:
  // for condition
  result = A.List_List$from(t5, false, t1);
  result.$flags = 3;
  if (!(k < result.length)) { ... }
  result = A.List_List$from(t5, false, t1);
  result.$flags = 3;
  t7 = result;
```

### Consequência para o relato

A nota do relatório pedia confirmar na última stable antes de abrir a issue em
`dart-lang/sdk`. **Não reproduz mais**, portanto não há bug novo a relatar. O
que resta é orientação de atualização: projetos presos ao 3.6.2 precisam de um
dos contornos acima.

### O que continua em ambas as versões

A cópia da lista acontece **duas vezes por iteração** — uma na condição do laço
e outra no corpo —, além da cópia do `isEmbed`. São três `List_List$from` no
corpo assíncrono nas duas versões. Não é defeito de correção, e sim custo:
`List.unmodifiable` num getter lido dentro de laço copia a lista a cada leitura.
O contorno de capturar o resultado numa variável local antes do laço continua
valendo por desempenho, mesmo depois da correção.

## Ambiente verificado

- Dart SDK 3.6.2 (stable) em `windows_x64` — reproduz, minificado e não minificado.
- Dart SDK 3.13.4 (stable) em `windows_x64`, baixado em `D:/DartSDKs/3.13.4` —
  não reproduz em nenhuma das duas formas.
- Node.js v22.17.0, usado apenas como runtime JS. O travamento original ocorre
  no Chrome, com a saída de `build_web_compilers` / `dart compile js --minify`.
