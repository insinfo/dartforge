import 'package:ngdart/angular.dart';

/// Componente `onPush` cujos `@Input` estão todos na base: o `hasInputs`
/// do oficial conta os herdados, e a hospedeira escreve o `changed` e o
/// `markAsCheckOnce` (o `MaterialButtonComponent` do ngcomponents).
class J110Base {
  @Input()
  bool desligado = false;

  @HostBinding('class.desligado')
  bool get classeDesligado => desligado;
}

@Component(
  selector: 'j110-botao',
  template: '<i>x</i>',
  changeDetection: ChangeDetectionStrategy.onPush,
)
class J110Botao extends J110Base {}

class J110SemHost {
  @Input()
  String? texto;
}

@Component(
  selector: 'j110-simples',
  template: '<i>{{ texto }}</i>',
  changeDetection: ChangeDetectionStrategy.onPush,
)
class J110Simples extends J110SemHost {}

@Component(
  selector: 'j110-usa',
  template: '<j110-botao [desligado]="d"></j110-botao><j110-simples texto="a"></j110-simples>',
  directives: [J110Botao, J110Simples],
)
class J110Usa {
  bool d = false;
}
