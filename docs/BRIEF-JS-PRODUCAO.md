# Brief — backend JavaScript de produção (o nosso dart2js)

Você vai construir o **perfil de produção** do backend JavaScript do
DartForge. Hoje o emissor produz módulos ES no contrato do DDC, que é o
perfil de **desenvolvimento**: um módulo por biblioteca, sem otimização
entre bibliotecas, carregado pelo navegador com `dart_sdk.js` ao lado.

O perfil de produção é outro programa: um único arquivo, com o programa
inteiro visível, otimizado e minificado — o que o `dart2js` faz.

---

## 0. Regras inegociáveis

1. **Worktree própria.** Não trabalhe em `D:/Projects/dartforge`:
   ```
   git worktree add -b js-producao D:/Projects/dartforge-js main
   ```
   Commite lá; integre em `main` por fast-forward só quando compilar e o
   corpus passar. Com dois agentes na mesma árvore, um estado intermediário
   que não compila é revertido por quem "conserta o build" — já custou uma
   conversão inteira de trabalho.

2. **Não toque nestes arquivos**, que são de outro agente:
   `crates/gerador_ng/**`, `crates/elements/src/gerado.rs`, `crates/dev/**`,
   `crates/cli/src/main.rs`, `corpus/ngdart/**`, `scripts/corpus-ngdart.ps1`.
   Seu território: **um crate novo** `crates/emit_js_producao` (ou o nome que
   o desenho pedir), `corpus/js/**` só para acrescentar casos, e
   `docs/JS-PRODUCAO.md`.
   Em `crates/emit_js` **só acrescente** — não mude assinatura nem
   comportamento do que já existe, porque o `dartforge serve` depende dele.

3. **Estudar antes de implementar.** Não descubra regra por tentativa e
   erro: leia a referência, entenda o algoritmo, escreva o plano, e só então
   escreva código. Compilar é **confirmação**, não método de descoberta.
   Cada ciclo de compilar-e-ver custa minutos e memória da máquina.

4. **Medir contra o oficial.** O `dart2js` do SDK 3.6.2
   (`C:/tools/dartsdk-3.6.2`) é o oráculo de **comportamento** e a régua de
   **tamanho e velocidade**. Saída byte a byte igual à dele não é o alvo
   (nem seria possível); saída com o mesmo `stdout` e menor ou igual em
   tamanho, sim.

---

## 1. O que existe hoje

| peça | onde | o que faz |
| --- | --- | --- |
| trilha nova | `crates/frontend` → `elements` → `types` | léxico, árvore, elementos, tipos |
| emissor DDC | `crates/emit_js` | um módulo ES por biblioteca, contrato do DDC |
| runtime | `runtime/ddc/dart_sdk.js` | gerado do `ddc_platform.dill` |
| harness | `crates/diferencial` | 214 programas de `corpus/js`, comparados com a VM |

O emissor de desenvolvimento passa **214/214** no corpus e roda dois
projetos reais no navegador (`new_sali/frontend` e `limitless_ui/example`).
Leia `docs/EMISSAO-DDC.md`, `docs/CONTRATO-DDC.md` e `ESTADO.md` §1.3.

**Medição que motiva o trabalho** (em `ESTADO.md`): a compilação de
produção do `limitless_ui/example` pelo `build_web_compilers --release`
(que é o `dart2js`) leva **3m33s**; a nossa de desenvolvimento leva 4m11s
mas não otimiza nada. Não há hoje perfil de produção nosso.

---

## 2. O que estudar antes de escrever código

Nesta ordem, com o objetivo escrito ao lado:

1. **`references/dart-sdk/pkg/compiler/`** — é o `dart2js`. Leia, em
   particular:
   - `lib/src/js_backend/` — como ele nomeia, minifica e emite;
   - `lib/src/inferrer/` — inferência de tipos global, que é o que
     habilita as otimizações;
   - `lib/src/universe/` — o "mundo fechado": o conjunto de membros
     alcançáveis, base do tree shaking;
   - `lib/src/js_emitter/` — a montagem do arquivo final, os fragmentos e
     o carregamento diferido.
   Objetivo: entender **o que** ele otimiza e **por que** cada otimização é
   segura, não copiar código.

2. **`docs/DART2JS-REFERENCIA.md`** e `docs/OTIMIZACAO.md` no nosso repo —
   o que já foi levantado.

3. **`references/oxc`** — infraestrutura JS em Rust (parser, minificador,
   mangler). Se formos minificar, não escreva um minificador do zero antes
   de ver o que ele resolve.

4. **`crates/emit_js/src/module.rs`** — como a emissão atual agrupa
   bibliotecas em módulos e resolve nomes. O perfil de produção parte da
   mesma trilha semântica.

---

## 3. O plano que se espera de você

**Escreva-o em `docs/JS-PRODUCAO.md` antes de codar**, e deixe explícito:

1. **Mundo fechado.** Como calcular o conjunto alcançável a partir de
   `main` (classes instanciadas, membros chamados, seletores dinâmicos).
   Sem isso não há tree shaking, e sem tree shaking não há produção.

2. **Despacho.** Hoje o desenvolvimento usa o despacho dinâmico do DDC.
   Em produção, com mundo fechado, um seletor com um único alvo vira
   chamada direta. Diga como vai decidir isso e o que acontece quando o
   receptor é `dynamic`.

3. **Nomes.** Minificação de membros exige saber quais nomes escapam
   (`dart:js`, `@JS`, `noSuchMethod`, serialização). Liste as fugas e o
   critério de segurança.

4. **Saída.** Um arquivo, com o runtime embutido — não o `dart_sdk.js`
   inteiro, só o que o mundo fechado alcança.

5. **Verificação.** O corpus `corpus/js` roda igual, com o **mesmo
   stdout**, pelo perfil de produção. Acrescente ao `crates/diferencial`
   um modo `--producao` que compare os três: VM, nosso desenvolvimento,
   nosso produção. E meça tamanho e tempo contra o `dart2js` nos dois
   projetos reais.

6. **Ordem de trabalho**, por quanto cada passo destrava, com o corpus como
   placar em cada um.

Não comece a implementar antes de o plano estar escrito. Se o plano
mostrar que um passo é maior do que parecia, diga isso no plano — é para
isso que ele serve.

---

## 4. Como verificar

```powershell
# o corpus de desenvolvimento, que não pode regredir
cargo run --release -p dartforge-diferencial

# a VM, que é o oráculo de comportamento
dart run --enable-asserts corpus\js\001_hello.dart

# o dart2js, que é a régua de tamanho e velocidade
C:\tools\dartsdk-3.6.2\bin\dart.exe compile js -O4 -o saida.js corpus\js\001_hello.dart
```

---

## 5. O que entregar

- `docs/JS-PRODUCAO.md` com o plano, **antes** do primeiro commit de código.
- Um commit por etapa, mensagem em português explicando **por que** a
  decisão foi essa (qual referência, qual medição).
- O placar do corpus no perfil de produção, atualizado em `ESTADO.md`.
- Nada de "quase funciona": um programa que passa, passa com o mesmo
  `stdout` da VM, byte a byte.
