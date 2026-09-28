import 'package:ngdart/angular.dart';

/// `@SkipSelf()` para um token que o próprio nó provê não é ciclo: o
/// `_getDependency` do oficial não procura no nó (`provider_parser.dart:325`)
/// e vai aos elementos acima ou ao injetor de fora. Os padrões do
/// `tooltipControllerBinding` e do `ModalComponent` do ngcomponents.
class J108Controle {
  J108Controle(this.pai);
  final J108Controle? pai;
}

J108Controle criarControle(J108Controle? pai) => J108Controle(pai);

abstract class J108Janela {}

@Component(
  selector: 'j108-dica',
  template: '<i>x</i>',
  providers: [
    FactoryProvider(J108Controle, criarControle, deps: [
      [J108Controle, Optional(), SkipSelf()]
    ]),
  ],
)
class J108Dica {
  J108Dica(this.controle);
  final J108Controle controle;
}

@Component(
  selector: 'j108-janela',
  template: '<i>y</i>',
  providers: [ExistingProvider(J108Janela, J108JanelaComponente)],
)
class J108JanelaComponente implements J108Janela {
  J108JanelaComponente(@Optional() @SkipSelf() this.pai);
  final J108Janela? pai;
}

@Component(
  selector: 'j108-usa',
  template: '<j108-dica></j108-dica><j108-janela></j108-janela>',
  directives: [J108Dica, J108JanelaComponente],
)
class J108Usa {}
