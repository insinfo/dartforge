// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'a11_projecao.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'a11_projecao.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$A11Projecao = const [];

class ViewA11Projecao0 extends import0.ComponentView<import1.A11Projecao> {
  static import2.ComponentStyles? _componentStyles;
  ViewA11Projecao0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('a11-projecao'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/a11_projecao.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendDiv(doc, parentRenderNode);
    this.project(_el_0, 0);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$A11Projecao, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _A11ProjecaoNgFactory = ComponentFactory<import1.A11Projecao>('a11-projecao', viewFactory_A11ProjecaoHost0);
ComponentFactory<import1.A11Projecao> get A11ProjecaoNgFactory {
  return _A11ProjecaoNgFactory;
}

ComponentFactory<import1.A11Projecao> createA11ProjecaoFactory() {
  return ComponentFactory('a11-projecao', viewFactory_A11ProjecaoHost0);
}

final List<Object> styles$A11ProjecaoHost = const [];

class _ViewA11ProjecaoHost0 extends import9.HostView<import1.A11Projecao> {
  @override
  void build() {
    this.componentView = ViewA11Projecao0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.A11Projecao();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.A11Projecao> viewFactory_A11ProjecaoHost0() {
  return _ViewA11ProjecaoHost0();
}
