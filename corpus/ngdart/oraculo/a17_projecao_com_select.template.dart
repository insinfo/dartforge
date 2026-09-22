// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'a17_projecao_com_select.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'a17_projecao_com_select.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$A17ProjecaoComSelect = const [];

class ViewA17ProjecaoComSelect0 extends import0.ComponentView<import1.A17ProjecaoComSelect> {
  static import2.ComponentStyles? _componentStyles;
  ViewA17ProjecaoComSelect0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('a17-projecao-com-select'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/a17_projecao_com_select.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendDiv(doc, parentRenderNode);
    this.project(_el_0, 0);
    this.project(_el_0, 1);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$A17ProjecaoComSelect, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _A17ProjecaoComSelectNgFactory = ComponentFactory<import1.A17ProjecaoComSelect>('a17-projecao-com-select', viewFactory_A17ProjecaoComSelectHost0);
ComponentFactory<import1.A17ProjecaoComSelect> get A17ProjecaoComSelectNgFactory {
  return _A17ProjecaoComSelectNgFactory;
}

ComponentFactory<import1.A17ProjecaoComSelect> createA17ProjecaoComSelectFactory() {
  return ComponentFactory('a17-projecao-com-select', viewFactory_A17ProjecaoComSelectHost0);
}

final List<Object> styles$A17ProjecaoComSelectHost = const [];

class _ViewA17ProjecaoComSelectHost0 extends import9.HostView<import1.A17ProjecaoComSelect> {
  @override
  void build() {
    this.componentView = ViewA17ProjecaoComSelect0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.A17ProjecaoComSelect();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.A17ProjecaoComSelect> viewFactory_A17ProjecaoComSelectHost0() {
  return _ViewA17ProjecaoComSelectHost0();
}
