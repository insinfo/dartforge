import 'dart:html';

import 'package:ngdart/angular.dart';

/// Componente que injeta `ViewContainerRef` (o `li-tooltip`/`li-popover`
/// do limitless_ui): a hospedeira cria um `ViewContainer` no elemento dele.
@Component(
  selector: 'j47-simples',
  template: '<b>s</b>',
)
class J47Simples {
  final ViewContainerRef container;

  J47Simples(this.container);
}

/// Com ganchos e `@HostBinding`: a ordem das chamadas na detecção.
@Component(
  selector: 'j47-completo',
  template: '<i>c</i>',
)
class J47Completo implements OnInit, AfterContentInit, AfterViewInit, OnDestroy {
  final Element elemento;
  final ViewContainerRef container;

  J47Completo(this.elemento, this.container);

  @HostBinding('class.ativo')
  bool ativo = true;

  @override
  void ngOnInit() {}

  @override
  void ngAfterContentInit() {}

  @override
  void ngAfterViewInit() {}

  @override
  void ngOnDestroy() {}
}

/// Quem usa: o nó do filho que injeta `ViewContainerRef` ganha o
/// `ViewContainer` na visão de quem o usa.
@Component(
  selector: 'j47-usa',
  template: '<j47-simples></j47-simples><p *ngIf="mostrar"><j47-simples></j47-simples></p>',
  directives: [J47Simples, NgIf],
)
class J47Usa {
  bool mostrar = true;
}
