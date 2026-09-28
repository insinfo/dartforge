import 'package:ngdart/angular.dart';

/// `#x="valor"` no elemento de um componente filho vale a diretiva do nó
/// com `exportAs` igual (`identifierForReference`), lida na mesma visão,
/// numa embutida (`unsafeCast<ViewX0>(parentView)._Dir_n_k`) e por
/// `@ViewChild`; `#ref` sem valor em embutida que nenhuma consulta alcança
/// é só um nome (o `material_menu`/`icon_tooltip`/`expansionpanel`).
@Directive(selector: '[j109Fonte]', exportAs: 'fonte')
class J109Fonte {
  String nome = 'f';
}

@Component(
  selector: 'j109-botao',
  template: '<i>b</i>',
  changeDetection: ChangeDetectionStrategy.onPush,
)
class J109Botao {}

@Component(
  selector: 'j109-alvo',
  template: '<i>{{ fonte?.nome }}</i>',
)
class J109Alvo {
  @Input()
  J109Fonte? fonte;
}

@Component(
  selector: 'j109-usa',
  template: '''
<j109-botao j109Fonte #origem="fonte" #botao></j109-botao>
<j109-alvo [fonte]="origem"></j109-alvo>
<div *ngIf="mostra">
  <j109-alvo [fonte]="origem"></j109-alvo>
  <j109-botao #botao></j109-botao>
</div>''',
  directives: [J109Botao, J109Fonte, J109Alvo, NgIf],
)
class J109Usa {
  bool mostra = true;

  @ViewChild('origem')
  J109Fonte? fonte;

  @ViewChild('botao')
  J109Botao? botao;
}
