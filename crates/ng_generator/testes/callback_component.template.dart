// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'callback_component.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'callback_component.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$CallbackComponent = const [];

class ViewCallbackComponent0 extends import0.ComponentView<import1.CallbackComponent> {
  static import2.ComponentStyles? _componentStyles;
  ViewCallbackComponent0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('callback-page'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:new_sali_frontend/lib/src/modules/auth/pages/callback/callback_component.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendDiv(doc, parentRenderNode);
    final _text_1 = import7.appendText(_el_0, 'Processando login...');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$CallbackComponent, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _CallbackComponentNgFactory = ComponentFactory<import1.CallbackComponent>('callback-page', viewFactory_CallbackComponentHost0);
ComponentFactory<import1.CallbackComponent> get CallbackComponentNgFactory {
  return _CallbackComponentNgFactory;
}

ComponentFactory<import1.CallbackComponent> createCallbackComponentFactory() {
  return ComponentFactory('callback-page', viewFactory_CallbackComponentHost0);
}

final List<Object> styles$CallbackComponentHost = const [];

class _ViewCallbackComponentHost0 extends import9.HostView<import1.CallbackComponent> {
  @override
  void build() {
    this.componentView = ViewCallbackComponent0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.CallbackComponent();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.CallbackComponent> viewFactory_CallbackComponentHost0() {
  return _ViewCallbackComponentHost0();
}
