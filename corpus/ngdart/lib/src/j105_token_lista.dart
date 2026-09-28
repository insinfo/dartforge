import 'package:ngdart/angular.dart';

import 'j105_posicao.dart';

/// `OpaqueToken` com argumento de tipo concreto (`List<J105Posicao>`, o
/// `defaultPopupPositions` do ngcomponents): o `fromDartType` escreve os
/// argumentos, o `List` sem prefixo (alocando o `dart:core`) e a classe pelo
/// URI `package:` com import próprio.
const j105Posicoes = OpaqueToken<List<J105Posicao>>('j105Posicoes');
const j105Mapa = OpaqueToken<Map<String, bool>>('j105Mapa');

@Component(
  selector: 'j105-popup',
  template: '<i>x</i>',
)
class J105Popup {
  J105Popup(
    @Inject(j105Posicoes) this.posicoes,
    @Optional() @Inject(j105Mapa) Map<String, bool>? mapa,
  );

  final List<J105Posicao> posicoes;
}

@Component(
  selector: 'j105-usa',
  template: '<j105-popup></j105-popup>',
  directives: [J105Popup],
)
class J105Usa {}
