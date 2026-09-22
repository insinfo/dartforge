// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'b05_host_binding.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'b05_host_binding.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/runtime/check_binding.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$B05HostBinding = const [];

class ViewB05HostBinding0 extends import0.ComponentView<import1.B05HostBinding> {
  Object? _expr_0;
  static import2.ComponentStyles? _componentStyles;
  ViewB05HostBinding0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('b05-host-binding'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/b05_host_binding.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendDiv(doc, parentRenderNode);
    final _text_1 = import7.appendText(_el_0, 'oi');
  }

  void detectHostChanges(bool firstCheck) {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.ativo;
    if (import8.checkBinding(this._expr_0, currVal_0, null, null)) {
      import7.updateClassBindingNonHtml(this.rootElement, 'ativo', currVal_0);
      this._expr_0 = currVal_0;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$B05HostBinding, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _B05HostBindingNgFactory = ComponentFactory<import1.B05HostBinding>('b05-host-binding', viewFactory_B05HostBindingHost0);
ComponentFactory<import1.B05HostBinding> get B05HostBindingNgFactory {
  return _B05HostBindingNgFactory;
}

ComponentFactory<import1.B05HostBinding> createB05HostBindingFactory() {
  return ComponentFactory('b05-host-binding', viewFactory_B05HostBindingHost0);
}

final List<Object> styles$B05HostBindingHost = const [];

class _ViewB05HostBindingHost0 extends import10.HostView<import1.B05HostBinding> {
  @override
  void build() {
    this.componentView = ViewB05HostBinding0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.B05HostBinding();
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    this.componentView.detectHostChanges(firstCheck);
    this.componentView.detectChanges();
  }
}

import10.HostView<import1.B05HostBinding> viewFactory_B05HostBindingHost0() {
  return _ViewB05HostBindingHost0();
}
