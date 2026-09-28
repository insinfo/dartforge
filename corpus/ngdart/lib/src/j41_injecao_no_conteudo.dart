import 'package:ngdart/angular.dart';

class J41Servico {}

/// Filho visível (`Visibility.all`): o conteúdo o injeta pelo nó dele.
@Component(
  selector: 'j41-aba',
  template: '<div><ng-content></ng-content></div>',
  visibility: Visibility.all,
)
class J41Aba {}

/// Filho de visibilidade local (o padrão): o nó dele não provê nada.
@Component(
  selector: 'j41-local',
  template: '<p><ng-content></ng-content></p>',
)
class J41Local {}

/// No conteúdo de um filho: o filho acima e um serviço de fora da visão
/// (o `li-sweet-alert` dentro de `li-tabx` do limitless_ui).
@Directive(selector: '[j41-dir]')
class J41Dir {
  final J41Aba aba;
  final J41Servico servico;

  J41Dir(this.aba, this.servico);
}

/// O filho local não é achado pelo nó: vem do injetor de fora.
@Directive(selector: '[j41-opc]')
class J41Opc {
  final J41Local? local;

  J41Opc(@Optional() this.local);
}

/// Diretiva local (o padrão) num elemento.
@Directive(selector: '[j41-pai]')
class J41Pai {}

/// Num elemento abaixo: acha a diretiva local do pai no mesmo template.
@Directive(selector: '[j41-filha]')
class J41Filha {
  final J41Pai pai;

  J41Filha(this.pai);
}

@Component(
  selector: 'j41-injecao-no-conteudo',
  templateUrl: 'j41_injecao_no_conteudo.html',
  directives: [J41Aba, J41Local, J41Dir, J41Opc, J41Pai, J41Filha, NgIf],
)
class J41InjecaoNoConteudo {
  bool mostrar = true;
}
