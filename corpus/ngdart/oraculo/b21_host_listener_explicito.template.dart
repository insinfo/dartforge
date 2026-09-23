// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'b21_host_listener_explicito.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'b21_host_listener_explicito.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$B21HostListenerExplicito = const [];

class ViewB21HostListenerExplicito0 extends import0.ComponentView<import1.B21HostListenerExplicito> {
  static import2.ComponentStyles? _componentStyles;
  ViewB21HostListenerExplicito0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('b21-host-listener-explicito'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/b21_host_listener_explicito.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendDiv(doc, parentRenderNode);
    final _text_1 = import7.appendText(_el_0, 'oi');
    _el_0.addEventListener('click', this.eventHandler0(_ctx.clicou));
    parentRenderNode.addEventListener('keydown', this.eventHandler1(_ctx.tecla));
    parentRenderNode.addEventListener('blur', this.eventHandler0(_ctx.saiu));
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$B21HostListenerExplicito, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _B21HostListenerExplicitoNgFactory = ComponentFactory<import1.B21HostListenerExplicito>('b21-host-listener-explicito', viewFactory_B21HostListenerExplicitoHost0);
ComponentFactory<import1.B21HostListenerExplicito> get B21HostListenerExplicitoNgFactory {
  return _B21HostListenerExplicitoNgFactory;
}

ComponentFactory<import1.B21HostListenerExplicito> createB21HostListenerExplicitoFactory() {
  return ComponentFactory('b21-host-listener-explicito', viewFactory_B21HostListenerExplicitoHost0);
}

final List<Object> styles$B21HostListenerExplicitoHost = const [];

class _ViewB21HostListenerExplicitoHost0 extends import9.HostView<import1.B21HostListenerExplicito> {
  @override
  void build() {
    this.componentView = ViewB21HostListenerExplicito0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.B21HostListenerExplicito();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.B21HostListenerExplicito> viewFactory_B21HostListenerExplicitoHost0() {
  return _ViewB21HostListenerExplicitoHost0();
}
