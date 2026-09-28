// ignore_for_file: uri_has_not_been_generated

import 'package:ngdart/angular.dart';
import 'package:ngforms/ngforms.dart';

import 'j87_injetor.template.dart' as self;

class J87Servico {}

class J87Log {
  J87Log(this.servico);

  final J87Servico servico;
}

abstract class J87Base {}

class J87Impl implements J87Base {
  J87Impl.nomeado(J87Servico s, @Optional() J87Log? log, [int? ignorado]);
}

class J87Alias {}

class J87Config {
  const J87Config(this.a, {this.b = false, this.filho});

  final String a;
  final bool b;
  final J87Filho? filho;
}

class J87Filho {
  const J87Filho(this.n);

  final int n;
}

class J87Dependente {
  J87Dependente(
    @Self() J87Servico a,
    @Host() J87Servico c,
    @SkipSelf() J87Servico e,
    @Optional() @SkipSelf() J87Log? f,
    @Inject(j87Nome) String nome,
    @Optional() @Inject(j87Nome) String? nome2,
  );
}

const j87Nome = OpaqueToken<String>('j87Nome');
const j87Numero = OpaqueToken<int>('j87Numero');
const j87Multi = MultiToken<String>('j87Multi');
const j87Vazio = OpaqueToken<String>();
const j87Apelido = OpaqueToken<String>('j87Apelido');

enum J87Cor { azul, verde }

class J87Outro {}

class J87Enum {}

J87Log criarLog(J87Servico s) => J87Log(s);

int criarNumero(J87Servico s, J87Log? log) => 1;

const j87Modulo = Module(
  include: [
    Module(provide: [ClassProvider(J87Log)]),
  ],
  provide: [
    ValueProvider.forToken(j87Multi, 'dois'),
    ClassProvider(J87Dependente),
  ],
);

@GenerateInjector([
  J87Servico,
  ClassProvider(J87Base, useClass: J87Impl),
  ValueProvider.forToken(j87Nome, "it's \$x\n"),
  ValueProvider(J87Config, J87Config('a', b: true, filho: J87Filho(2))),
  FactoryProvider(J87Log, criarLog),
  FactoryProvider.forToken(j87Numero, criarNumero,
      deps: [J87Servico, [J87Log, Optional()]]),
  ExistingProvider(J87Alias, J87Servico),
  ValueProvider.forToken(j87Multi, 'um'),
  j87Modulo,
  formProviders,
  J87Servico,
  Provider(J87Outro, useValue: 3),
  ValueProvider(J87Enum, J87Cor.verde),
  ValueProvider.forToken(j87Vazio, '\u00e9 \u{1F600} \\'),
  ExistingProvider.forToken(j87Apelido, j87Nome),
  ValueProvider.forToken(j87Multi, 'tres'),
])
final InjectorFactory injetor = self.injetor$Injector;

@GenerateInjector.fromModules([j87Modulo])
final InjectorFactory outro = self.outro$Injector;
