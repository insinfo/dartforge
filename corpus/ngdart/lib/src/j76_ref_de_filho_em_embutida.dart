import 'package:ngdart/angular.dart';

@Component(
  selector: 'j76-tabela',
  template: '<i>{{rotulo}}</i>',
  changeDetection: ChangeDetectionStrategy.onPush,
)
class J76Tabela {
  @Input()
  String? rotulo;

  final _linha = Object();
  Object get linha => _linha;
}

@Component(
  selector: 'j76-cartao',
  template: '<b></b>',
)
class J76Cartao {}

/// `#ref` de um filho dentro de `*ngIf`, lido por `@ViewChild` (o
/// `#datatable` do `li-datatable-select`): consulta dinâmica com o
/// resultado de componente `onPush`.
@Component(
  selector: 'j76-ref-de-filho-em-embutida',
  templateUrl: 'j76_ref_de_filho_em_embutida.html',
  directives: [J76Tabela, J76Cartao, NgIf],
)
class J76RefDeFilhoEmEmbutida {
  bool mostrar = true;
  String nome = 'n';

  @ViewChild('tabela')
  J76Tabela? tabela;

  @ViewChild('cartao')
  J76Cartao? cartao;
}
