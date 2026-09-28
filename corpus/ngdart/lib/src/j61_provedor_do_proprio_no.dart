import 'package:ngdart/angular.dart';

class J61Config {
  int n = 0;
}

class J61Servico {}

class J61Externo {}

/// Filho que injeta o provedor que ele mesmo declara (o
/// `TypeaheadGlobalConfigDemoComponent` do limitless_ui) e um serviço de
/// fora: o provedor é criado antes dele, que fica no índice seguinte.
@Component(
  selector: 'j61-com-config',
  template: '<span>{{config.n}}</span>',
  providers: [ClassProvider(J61Config)],
)
class J61ComConfig {
  final J61Config config;
  final J61Externo externo;

  J61ComConfig(this.externo, this.config);
}

/// Dois provedores do nó, pedidos fora da ordem de `providers:`, e nada de
/// fora (a criação não é embrulhada).
@Component(
  selector: 'j61-com-dois',
  template: '<b>{{config.n}}</b>',
  providers: [ClassProvider(J61Config), ClassProvider(J61Servico)],
)
class J61ComDois {
  final J61Servico servico;
  final J61Config config;

  J61ComDois(this.servico, this.config);
}

@Component(
  selector: 'j61-provedor-do-proprio-no',
  template: '<div><j61-com-config></j61-com-config></div>'
      '<j61-com-dois></j61-com-dois><j61-com-config></j61-com-config>',
  directives: [J61ComConfig, J61ComDois],
  providers: [ClassProvider(J61Externo)],
)
class J61ProvedorDoProprioNo {}
