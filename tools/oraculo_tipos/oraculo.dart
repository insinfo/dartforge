// Oráculo de tipos: o `package:analyzer` oficial grava, por expressão, o tipo
// estático e o elemento resolvido. É oráculo de teste da inferência do
// `crates/types`, nunca produto (docs/FRONTEND-NEW-SALI.md, "Oráculo").
//
// Uso (o analyzer vem do package_config do próprio projeto analisado):
//
//   dart --packages=<projeto>/.dart_tool/package_config.json \
//       tools/oraculo_tipos/oraculo.dart <raiz do projeto> <arquivos.txt> <saida.tsv>
//
// `arquivos.txt` é a lista de unidades que o nosso despejo cobriu
// (`cargo run -p dartforge-types --example despejo_tipos`), um caminho por
// linha; a saída tem o mesmo formato do nosso despejo:
//
//   caminho \t offset \t comprimento \t nó \t tipo \t elemento
import 'dart:io';

import 'package:analyzer/dart/analysis/results.dart';
import 'package:analyzer/dart/ast/ast.dart';
import 'package:analyzer/dart/ast/visitor.dart';
import 'package:analyzer/dart/element/element.dart';
// ignore: implementation_imports
import 'package:analyzer/src/dart/analysis/analysis_context_collection.dart';
// ignore: implementation_imports
import 'package:analyzer/src/dart/analysis/byte_store.dart';
// ignore: implementation_imports
import 'package:analyzer/src/dart/analysis/driver_based_analysis_context.dart';
// ignore: implementation_imports
import 'package:analyzer/src/dart/analysis/file_byte_store.dart';
import 'package:path/path.dart' as p;

Future<void> main(List<String> args) async {
  if (args.length != 3) {
    stderr.writeln('uso: oraculo.dart <raiz> <arquivos.txt> <saida.tsv>');
    exit(64);
  }
  final raiz = _abs(args[0]);
  final arquivos = File(args[1])
      .readAsLinesSync()
      .map((l) => l.trim())
      .where((l) => l.isNotEmpty)
      .toList();
  final saida = File(args[2]).openWrite();
  // Resumos em disco (cache do analyzer ao lado da saída), com só 32 MB em
  // memória: o pico fica no que uma biblioteca precisa, não no projeto.
  final cache = p.join(p.dirname(_abs(args[2])), 'cache-analyzer');
  Directory(cache).createSync(recursive: true);
  final colecao = AnalysisContextCollectionImpl(
    includedPaths: [raiz],
    // Compilado com `dart compile exe`, o executável não sabe onde está o
    // SDK: DART_SDK, ou o 3.6.2 da máquina.
    sdkPath: _abs(Platform.environment['DART_SDK'] ?? 'C:/tools/dartsdk-3.6.2'),
    byteStore: MemoryCachingByteStore(EvictingFileByteStore(cache, 1 << 30), 32 << 20),
  );
  final ctx = colecao.contextFor(raiz);
  var ok = 0, falhas = 0;
  for (final arquivo in arquivos) {
    // Gerados: primeiro pelo caminho de origem (o workspace de
    // `package:build` do analyzer os acha lá), depois pelo próprio caminho.
    var r = await ctx.currentSession.getResolvedUnit(_semGerado(arquivo));
    if (r is! ResolvedUnitResult) {
      r = await ctx.currentSession.getResolvedUnit(_abs(arquivo));
    }
    if (r is! ResolvedUnitResult) {
      saida.writeln('#falha\t$arquivo\t${r.runtimeType}');
      falhas++;
      continue;
    }
    // Chave pelo caminho que o nosso despejo usou (o gerado, se for o caso).
    // As linhas saem ordenadas por (offset, comprimento), na ordem da lista
    // (a do nosso despejo): o comparador faz merge em fluxo.
    final v = _Visitante(_norm(arquivo));
    r.unit.accept(v);
    v.linhas.sort((a, b) => a.$1 != b.$1 ? a.$1.compareTo(b.$1) : a.$2.compareTo(b.$2));
    for (final l in v.linhas) {
      saida.writeln(l.$3);
    }
    ok++;
    // Memória: o analyzer guarda o modelo de elementos de tudo o que já
    // resolveu; limpar de tempos em tempos mantém o pico baixo (o que já foi
    // ligado volta dos resumos em cache).
    if (ok % 150 == 0) {
      (ctx as DriverBasedAnalysisContext).driver.clearLibraryContext();
    }
  }
  await saida.close();
  stderr.writeln('oráculo: $ok unidades resolvidas, $falhas falhas');
}

/// `.dart_tool/build/generated/<pacote>/<resto>` → `<raiz do pacote>/<resto>`:
/// o workspace de `package:build` do analyzer encontra o gerado pelo caminho
/// de origem.
String _semGerado(String q) {
  final n = q.replaceAll('\\', '/');
  final i = n.indexOf('/.dart_tool/build/generated/');
  if (i < 0) return _abs(q);
  final raiz = n.substring(0, i);
  final resto = n.substring(i + '/.dart_tool/build/generated/'.length);
  final barra = resto.indexOf('/');
  return _abs('$raiz/${resto.substring(barra + 1)}');
}

String _norm(String x) => x.replaceAll('\\', '/');

String _abs(String x) => p.normalize(p.absolute(x));

class _Visitante extends GeneralizingAstVisitor<void> {
  final String arquivo;
  final linhas = <(int, int, String)>[];
  _Visitante(this.arquivo);

  // URIs de diretivas não são expressões para nós.
  @override
  void visitDirective(Directive node) {}

  @override
  void visitExpression(Expression node) {
    final tipo = node.staticType?.getDisplayString() ?? '-';
    final el = _elemento(node);
    linhas.add((node.offset, node.length,
        '$arquivo\t${node.offset}\t${node.length}\t${node.runtimeType.toString().replaceAll('Impl', '')}\t$tipo\t$el'));
    super.visitExpression(node);
  }

  String _elemento(Expression node) {
    Element? e;
    if (node is SimpleIdentifier) {
      e = node.staticElement;
    } else if (node is PrefixedIdentifier) {
      e = node.staticElement;
    } else if (node is MethodInvocation) {
      e = node.methodName.staticElement;
    } else if (node is PropertyAccess) {
      e = node.propertyName.staticElement;
    } else if (node is InstanceCreationExpression) {
      e = node.constructorName.staticElement;
    } else if (node is AssignmentExpression) {
      e = node.writeElement;
    } else if (node is BinaryExpression) {
      e = node.staticElement;
    } else if (node is IndexExpression) {
      e = node.staticElement;
    }
    if (e == null) return '-';
    final dono = e.enclosingElement3;
    final nomeDono = dono is CompilationUnitElement ? '' : '${dono?.name}.';
    return '${e.kind.name}:$nomeDono${e.name}';
  }
}
