import 'package:ngdart/angular.dart';

class J79Servico {}

@Component(
  selector: 'j79-linha',
  template: '<ng-content></ng-content>',
)
class J79Linha {}

/// Filho que pede o componente de cima com `@Host() @Optional()` (o
/// `LiTimelineItemComponent` do limitless_ui), mais `@Self()`,
/// `@SkipSelf()` e `@Host()` sem provedor na visão.
@Component(
  selector: 'j79-item',
  template: '<i></i>',
)
class J79Item {
  J79Item(
    @Host() @Optional() this.linha,
    @Self() @Optional() this.proprio,
    @SkipSelf() @Optional() this.acima,
    @Host() @Optional() this.servico,
  );

  final J79Linha? linha;
  final J79Servico? proprio;
  final J79Servico? acima;
  final J79Servico? servico;
}

@Component(
  selector: 'j79-host-e-self',
  templateUrl: 'j79_host_e_self.html',
  directives: [J79Linha, J79Item],
)
class J79HostESelf {}
