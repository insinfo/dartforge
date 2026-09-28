import 'package:ngdart/angular.dart';

/// `@HostBinding('class')` num componente (o `hostClass` do
/// `li-timeline`): a classe inteira pela visão dele.
@Component(
  selector: 'j80-linha-do-tempo',
  template: '<ng-content></ng-content>',
)
class J80LinhaDoTempo {
  @Input()
  String modo = 'a';

  @HostBinding('class')
  String get classeDoHospedeiro => 'timeline timeline-$modo';

  @HostBinding('style.--cor')
  String? cor;

  @HostBinding('attr.role')
  String papel = 'list';
}

@Component(
  selector: 'j80-host-binding-class',
  template: '<j80-linha-do-tempo [modo]="modo">x</j80-linha-do-tempo>',
  directives: [J80LinhaDoTempo],
)
class J80HostBindingClass {
  String modo = 'b';
}
