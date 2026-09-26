// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i18_host_listener_componente.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i18_host_listener_componente.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$I18HostListenerComponente = const [];

class ViewI18HostListenerComponente0 extends import0.ComponentView<import1.I18HostListenerComponente> {
  static import2.ComponentStyles? _componentStyles;
  ViewI18HostListenerComponente0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('i18-host-listener-componente'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/i18_host_listener_componente.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'p');
    final _text_1 = import7.appendText(_el_0, 'x');
    parentRenderNode.addEventListener('click', this.eventHandler1(_ctx.clicou));
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$I18HostListenerComponente, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I18HostListenerComponenteNgFactory = ComponentFactory<import1.I18HostListenerComponente>('i18-host-listener-componente', viewFactory_I18HostListenerComponenteHost0);
ComponentFactory<import1.I18HostListenerComponente> get I18HostListenerComponenteNgFactory {
  return _I18HostListenerComponenteNgFactory;
}

ComponentFactory<import1.I18HostListenerComponente> createI18HostListenerComponenteFactory() {
  return ComponentFactory('i18-host-listener-componente', viewFactory_I18HostListenerComponenteHost0);
}

final List<Object> styles$I18HostListenerComponenteHost = const [];

class _ViewI18HostListenerComponenteHost0 extends import9.HostView<import1.I18HostListenerComponente> {
  @override
  void build() {
    this.componentView = ViewI18HostListenerComponente0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I18HostListenerComponente();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.I18HostListenerComponente> viewFactory_I18HostListenerComponenteHost0() {
  return _ViewI18HostListenerComponenteHost0();
}
