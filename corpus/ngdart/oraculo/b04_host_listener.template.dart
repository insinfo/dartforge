// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'b04_host_listener.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'b04_host_listener.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$B04HostListener = const [];

class ViewB04HostListener0 extends import0.ComponentView<import1.B04HostListener> {
  static import2.ComponentStyles? _componentStyles;
  ViewB04HostListener0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('b04-host-listener'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/b04_host_listener.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendDiv(doc, parentRenderNode);
    final _text_1 = import7.appendText(_el_0, 'oi');
    parentRenderNode.addEventListener('click', this.eventHandler1(_ctx.clicou));
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$B04HostListener, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _B04HostListenerNgFactory = ComponentFactory<import1.B04HostListener>('b04-host-listener', viewFactory_B04HostListenerHost0);
ComponentFactory<import1.B04HostListener> get B04HostListenerNgFactory {
  return _B04HostListenerNgFactory;
}

ComponentFactory<import1.B04HostListener> createB04HostListenerFactory() {
  return ComponentFactory('b04-host-listener', viewFactory_B04HostListenerHost0);
}

final List<Object> styles$B04HostListenerHost = const [];

class _ViewB04HostListenerHost0 extends import9.HostView<import1.B04HostListener> {
  @override
  void build() {
    this.componentView = ViewB04HostListener0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.B04HostListener();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.B04HostListener> viewFactory_B04HostListenerHost0() {
  return _ViewB04HostListenerHost0();
}
