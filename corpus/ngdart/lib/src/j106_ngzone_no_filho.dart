import 'package:ngdart/angular.dart';

/// Classe do ngdart que não é embutida do nó (`NgZone`) é serviço comum no
/// construtor do filho: `injectorGet` pela visão de cima, como qualquer
/// outro (o `FixedMaterialTabStripComponent` do ngcomponents).
@Component(
  selector: 'j106-faixa',
  template: '<i>x</i>',
)
class J106Faixa {
  J106Faixa(this._cd, this._zona, @Optional() ApplicationRef? app);

  final ChangeDetectorRef _cd;
  final NgZone _zona;
}

@Component(
  selector: 'j106-usa',
  template: '<j106-faixa></j106-faixa>',
  directives: [J106Faixa],
)
class J106Usa {}
