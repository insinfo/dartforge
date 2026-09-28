// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j30_caixa.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j30_caixa.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/runtime/check_binding.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;
import 'package:ngforms/src/directives/control_value_accessor.dart' as import11;
import 'package:ngdart/src/meta/di_tokens.dart' as import12;
import 'package:ngforms/src/directives/control_value_accessor.dart' as import13;

final List<Object> styles$J30Caixa = const [];

class ViewJ30Caixa0 extends import0.ComponentView<import1.J30Caixa> {
  Object? _expr_0;
  static import2.ComponentStyles? _componentStyles;
  ViewJ30Caixa0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j30-caixa'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j30_caixa.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendSpan(doc, parentRenderNode);
    final _text_1 = import7.appendText(_el_0, 'campo');
  }

  void detectHostChanges(bool firstCheck) {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.hostClass;
    if (import8.checkBinding(this._expr_0, currVal_0, null, null)) {
      import7.updateClassBindingNonHtml(this.rootElement, 'd-block', currVal_0);
      this._expr_0 = currVal_0;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J30Caixa, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J30CaixaNgFactory = ComponentFactory<import1.J30Caixa>('j30-caixa', viewFactory_J30CaixaHost0);
ComponentFactory<import1.J30Caixa> get J30CaixaNgFactory {
  return _J30CaixaNgFactory;
}

ComponentFactory<import1.J30Caixa> createJ30CaixaFactory() {
  return ComponentFactory('j30-caixa', viewFactory_J30CaixaHost0);
}

final List<Object> styles$J30CaixaHost = const [];

class _ViewJ30CaixaHost0 extends import10.HostView<import1.J30Caixa> {
  late List<import11.ControlValueAccessor<dynamic>> _NgValueAccessor_0_6 = [this.component];
  @override
  void build() {
    this.componentView = ViewJ30Caixa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J30Caixa();
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((identical(token, const import12.MultiToken<import13.ControlValueAccessor<dynamic>>('NgValueAccessor')) && (0 == nodeIndex))) {
      return this._NgValueAccessor_0_6;
    }
    return notFoundResult;
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    this.componentView.detectHostChanges(firstCheck);
    this.componentView.detectChanges();
  }
}

import10.HostView<import1.J30Caixa> viewFactory_J30CaixaHost0() {
  return _ViewJ30CaixaHost0();
}
