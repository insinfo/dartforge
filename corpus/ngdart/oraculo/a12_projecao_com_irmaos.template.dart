// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'a12_projecao_com_irmaos.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'a12_projecao_com_irmaos.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$A12ProjecaoComIrmaos = const [];

class ViewA12ProjecaoComIrmaos0 extends import0.ComponentView<import1.A12ProjecaoComIrmaos> {
  static import2.ComponentStyles? _componentStyles;
  ViewA12ProjecaoComIrmaos0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('a12-projecao-com-irmaos'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/a12_projecao_com_irmaos.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendDiv(doc, parentRenderNode);
    final _el_1 = import7.appendSpan(doc, _el_0);
    final _text_2 = import7.appendText(_el_1, 'a');
    this.project(_el_0, 0);
    final _el_3 = import7.appendSpan(doc, _el_0);
    final _text_4 = import7.appendText(_el_3, 'b');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$A12ProjecaoComIrmaos, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _A12ProjecaoComIrmaosNgFactory = ComponentFactory<import1.A12ProjecaoComIrmaos>('a12-projecao-com-irmaos', viewFactory_A12ProjecaoComIrmaosHost0);
ComponentFactory<import1.A12ProjecaoComIrmaos> get A12ProjecaoComIrmaosNgFactory {
  return _A12ProjecaoComIrmaosNgFactory;
}

ComponentFactory<import1.A12ProjecaoComIrmaos> createA12ProjecaoComIrmaosFactory() {
  return ComponentFactory('a12-projecao-com-irmaos', viewFactory_A12ProjecaoComIrmaosHost0);
}

final List<Object> styles$A12ProjecaoComIrmaosHost = const [];

class _ViewA12ProjecaoComIrmaosHost0 extends import9.HostView<import1.A12ProjecaoComIrmaos> {
  @override
  void build() {
    this.componentView = ViewA12ProjecaoComIrmaos0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.A12ProjecaoComIrmaos();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.A12ProjecaoComIrmaos> viewFactory_A12ProjecaoComIrmaosHost0() {
  return _ViewA12ProjecaoComIrmaosHost0();
}
