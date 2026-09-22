// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'b01_ciclo_de_vida.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'b01_ciclo_de_vida.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;
import 'package:ngdart/src/runtime/check_binding.dart' as import10;

final List<Object> styles$B01CicloDeVida = const [];

class ViewB01CicloDeVida0 extends import0.ComponentView<import1.B01CicloDeVida> {
  static import2.ComponentStyles? _componentStyles;
  ViewB01CicloDeVida0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('b01-ciclo-de-vida'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/b01_ciclo_de_vida.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendDiv(doc, parentRenderNode);
    final _text_1 = import7.appendText(_el_0, 'oi');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$B01CicloDeVida, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _B01CicloDeVidaNgFactory = ComponentFactory<import1.B01CicloDeVida>('b01-ciclo-de-vida', viewFactory_B01CicloDeVidaHost0);
ComponentFactory<import1.B01CicloDeVida> get B01CicloDeVidaNgFactory {
  return _B01CicloDeVidaNgFactory;
}

ComponentFactory<import1.B01CicloDeVida> createB01CicloDeVidaFactory() {
  return ComponentFactory('b01-ciclo-de-vida', viewFactory_B01CicloDeVidaHost0);
}

final List<Object> styles$B01CicloDeVidaHost = const [];

class _ViewB01CicloDeVidaHost0 extends import9.HostView<import1.B01CicloDeVida> {
  @override
  void build() {
    this.componentView = ViewB01CicloDeVida0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.B01CicloDeVida();
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    if (((!import10.debugThrowIfChanged) && firstCheck)) {
      this.component.ngOnInit();
    }
    this.componentView.detectChanges();
  }

  @override
  void destroyInternal() {
    this.component.ngOnDestroy();
  }
}

import9.HostView<import1.B01CicloDeVida> viewFactory_B01CicloDeVidaHost0() {
  return _ViewB01CicloDeVidaHost0();
}
