import 'package:ngdart/angular.dart';

/// `@HostBinding` junto de ganchos de ciclo de vida: a ordem na
/// visão-hospedeira (o `LiTabsComponent` do limitless_ui).
@Component(
  selector: 'j32-hospedeiro-com-ganchos',
  template: '<span>x</span>',
)
class J32HospedeiroComGanchos
    implements OnInit, DoCheck, AfterContentInit, AfterContentChecked, AfterViewInit, AfterViewChecked, OnDestroy {
  @HostBinding('class.ativo')
  bool ativo = true;

  @HostBinding('attr.data-x')
  String get x => 'x';

  @override
  void ngOnInit() {}

  @override
  void ngDoCheck() {}

  @override
  void ngAfterContentInit() {}

  @override
  void ngAfterContentChecked() {}

  @override
  void ngAfterViewInit() {}

  @override
  void ngAfterViewChecked() {}

  @override
  void ngOnDestroy() {}
}
