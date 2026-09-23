// @dart=3.6
// ignore_for_file: directives_ordering
// ignore_for_file: no_leading_underscores_for_library_prefixes
import 'package:build_runner_core/build_runner_core.dart' as _i1;
import '../../../tool/builders.dart' as _i2;
import 'package:build_config/build_config.dart' as _i3;
import 'package:build/build.dart' as _i4;
import 'package:apoio_cadeia/carimbo.dart' as _i5;
import 'package:build_resolvers/builder.dart' as _i6;
import 'dart:isolate' as _i7;
import 'package:build_runner/build_runner.dart' as _i8;
import 'dart:io' as _i9;

final _builders = <_i1.BuilderApplication>[
  _i1.apply(
    r'corpus_cadeia_configuracao:espelho',
    [_i2.espelho],
    _i1.toNoneByDefault(),
    hideOutput: false,
    defaultGenerateFor: const _i3.InputSet(include: [r'lib/**']),
    defaultOptions: const _i4.BuilderOptions(<String, dynamic>{
      r'origem': r'defaults',
      r'so_defaults': r'sim',
    }),
  ),
  _i1.apply(
    r'corpus_cadeia_configuracao:maiusculas',
    [_i2.maiusculas],
    _i1.toRoot(),
    hideOutput: true,
    defaultGenerateFor: const _i3.InputSet(
      include: [r'lib/**'],
      exclude: [r'lib/rascunho/**'],
    ),
    defaultOptions: const _i4.BuilderOptions(<String, dynamic>{
      r'nivel': 1,
      r'origem': r'defaults',
      r'modo': r'defaults',
    }),
    defaultDevOptions: const _i4.BuilderOptions(<String, dynamic>{
      r'modo': r'dev_defaults',
      r'so_dev': r'sim',
    }),
    defaultReleaseOptions: const _i4.BuilderOptions(
        <String, dynamic>{r'modo': r'release_defaults'}),
    appliesBuilders: const [r'corpus_cadeia_configuracao:limpeza'],
  ),
  _i1.apply(
    r'apoio_cadeia:carimbo',
    [_i5.carimbo],
    _i1.toDependentsOf(r'apoio_cadeia'),
    hideOutput: true,
    defaultGenerateFor: const _i3.InputSet(include: [r'lib/**']),
    defaultOptions: const _i4.BuilderOptions(
        <String, dynamic>{r'origem': r'defaults_apoio'}),
  ),
  _i1.apply(
    r'corpus_cadeia_configuracao:indice',
    [_i2.indice],
    _i1.toNoneByDefault(),
    hideOutput: false,
  ),
  _i1.apply(
    r'corpus_cadeia_configuracao:ignorado',
    [_i2.ignorado],
    _i1.toDependentsOf(r'corpus_cadeia_configuracao'),
    hideOutput: true,
  ),
  _i1.apply(
    r'corpus_cadeia_configuracao:desligado',
    [_i2.desligado],
    _i1.toRoot(),
    hideOutput: true,
  ),
  _i1.apply(
    r'corpus_cadeia_configuracao:contagem',
    [
      _i2.contarLinhas,
      _i2.contarPalavras,
    ],
    _i1.toAllPackages(),
    hideOutput: true,
  ),
  _i1.apply(
    r'build_resolvers:transitive_digests',
    [_i6.transitiveDigestsBuilder],
    _i1.toAllPackages(),
    isOptional: true,
    hideOutput: true,
    appliesBuilders: const [r'build_resolvers:transitive_digest_cleanup'],
  ),
  _i1.applyPostProcess(
    r'build_resolvers:transitive_digest_cleanup',
    _i6.transitiveDigestCleanup,
  ),
  _i1.applyPostProcess(
    r'corpus_cadeia_configuracao:limpeza',
    _i2.limpeza,
    defaultOptions:
        const _i4.BuilderOptions(<String, dynamic>{r'apagar': false}),
    defaultReleaseOptions:
        const _i4.BuilderOptions(<String, dynamic>{r'apagar': true}),
  ),
];
void main(
  List<String> args, [
  _i7.SendPort? sendPort,
]) async {
  var result = await _i8.run(
    args,
    _builders,
  );
  sendPort?.send(result);
  _i9.exitCode = result;
}
