import 'package:ngdart/angular.dart';

/// Provedor preguiçoso do filho pedido por uma diretiva do conteúdo: o
/// `_getDependency` dela o transforma ansioso durante a visita, antes do
/// `afterElement` do filho — ele sai logo depois dos ansiosos, com o índice
/// dessa posição (o `PopupRef_0_9` do `paper_tooltip`).
class J116Hierarquia {}

class J116Ref {}

J116Hierarquia hierarquia(J116Popup p) => J116Hierarquia();
J116Ref ref(J116Popup p) => J116Ref();

@Component(
  selector: 'j116-popup',
  template: '<ng-content></ng-content>',
  providers: [
    FactoryProvider(J116Hierarquia, hierarquia),
    FactoryProvider(J116Ref, ref),
  ],
)
class J116Popup {}

@Directive(selector: '[j116Foco]')
class J116Foco {
  J116Foco(@Optional() J116Ref? ref);
}

@Component(
  selector: 'j116-usa',
  template: '''
<j116-popup *ngIf="mostra">
  <div j116Foco>x</div>
</j116-popup>''',
  directives: [J116Popup, J116Foco, NgIf],
)
class J116Usa {
  bool mostra = true;
}
