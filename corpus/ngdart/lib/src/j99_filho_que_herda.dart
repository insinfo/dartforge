import 'dart:async';

import 'package:ngdart/angular.dart';

/// Filho que herda (`extends` e `with`): `@Input`, `@Output`,
/// `@HostBinding` e ganchos dos supertipos, coletados como o
/// `_collectInheritableMetadata` do oficial (supertipos primeiro, o
/// derivado sobrescreve).
abstract class J99Base implements OnInit, OnDestroy {
  @Input()
  String? titulo;

  @Input('rotulo')
  String? legenda;

  final _mudou = StreamController<String>();

  @Output()
  Stream<String> get mudou => _mudou.stream;

  @HostBinding('class.ativo')
  bool ativo = false;

  @override
  void ngOnInit() {}

  @override
  void ngOnDestroy() {}
}

mixin J99Mixin {
  @Input()
  bool desligado = false;

  @Output('fechou')
  Stream<void> get fechar => const Stream.empty();
}

@Component(
  selector: 'j99-filho',
  template: '<b>{{ titulo }}</b><i>{{ legenda }}</i>',
)
class J99Filho extends J99Base with J99Mixin implements AfterViewInit {
  @Input()
  int contador = 0;

  @override
  void ngAfterViewInit() {}
}

@Component(
  selector: 'j99-usa',
  template: '<j99-filho [titulo]="nome" rotulo="fixo" [desligado]="ligado" '
      '[contador]="n" (mudou)="ouvir(\$event)" (fechou)="fechou()"></j99-filho>',
  directives: [J99Filho],
)
class J99Usa {
  String nome = 'a';
  bool ligado = false;
  int n = 1;
  void ouvir(String s) {}
  void fechou() {}
}
