// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'b17_host_listener_com_args.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'b17_host_listener_com_args.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$B17HostListenerComArgs = const [];

class ViewB17HostListenerComArgs0 extends import0.ComponentView<import1.B17HostListenerComArgs> {
  static import2.ComponentStyles? _componentStyles;
  ViewB17HostListenerComArgs0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('b17-host-listener-com-args'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/b17_host_listener_com_args.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendDiv(doc, parentRenderNode);
    final _text_1 = import7.appendText(_el_0, 'oi');
    parentRenderNode.addEventListener('input', this.eventHandler1(this._handleEvent_0));
  }

  void _handleEvent_0($event) {
    final _ctx = this.ctx;
    _ctx.mudou($event.target);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$B17HostListenerComArgs, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _B17HostListenerComArgsNgFactory = ComponentFactory<import1.B17HostListenerComArgs>('b17-host-listener-com-args', viewFactory_B17HostListenerComArgsHost0);
ComponentFactory<import1.B17HostListenerComArgs> get B17HostListenerComArgsNgFactory {
  return _B17HostListenerComArgsNgFactory;
}

ComponentFactory<import1.B17HostListenerComArgs> createB17HostListenerComArgsFactory() {
  return ComponentFactory('b17-host-listener-com-args', viewFactory_B17HostListenerComArgsHost0);
}

final List<Object> styles$B17HostListenerComArgsHost = const [];

class _ViewB17HostListenerComArgsHost0 extends import9.HostView<import1.B17HostListenerComArgs> {
  @override
  void build() {
    this.componentView = ViewB17HostListenerComArgs0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.B17HostListenerComArgs();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.B17HostListenerComArgs> viewFactory_B17HostListenerComArgsHost0() {
  return _ViewB17HostListenerComArgsHost0();
}
