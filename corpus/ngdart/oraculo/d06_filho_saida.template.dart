// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'd06_filho_saida.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'd06_filho_saida.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$D06FilhoSaida = const [];

class ViewD06FilhoSaida0 extends import0.ComponentView<import1.D06FilhoSaida> {
  static import2.ComponentStyles? _componentStyles;
  ViewD06FilhoSaida0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('d06-filho-saida'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/d06_filho_saida.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.ButtonElement>(doc, parentRenderNode, 'button');
    final _text_1 = import7.appendText(_el_0, 'ok');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$D06FilhoSaida, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _D06FilhoSaidaNgFactory = ComponentFactory<import1.D06FilhoSaida>('d06-filho-saida', viewFactory_D06FilhoSaidaHost0);
ComponentFactory<import1.D06FilhoSaida> get D06FilhoSaidaNgFactory {
  return _D06FilhoSaidaNgFactory;
}

ComponentFactory<import1.D06FilhoSaida> createD06FilhoSaidaFactory() {
  return ComponentFactory('d06-filho-saida', viewFactory_D06FilhoSaidaHost0);
}

final List<Object> styles$D06FilhoSaidaHost = const [];

class _ViewD06FilhoSaidaHost0 extends import9.HostView<import1.D06FilhoSaida> {
  @override
  void build() {
    this.componentView = ViewD06FilhoSaida0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.D06FilhoSaida();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.D06FilhoSaida> viewFactory_D06FilhoSaidaHost0() {
  return _ViewD06FilhoSaidaHost0();
}
