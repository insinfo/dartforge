// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'a22_form_com_forms_directives.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'a22_form_com_forms_directives.dart' as import1;
import 'package:ngforms/src/directives/ng_form.dart' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'dart:html' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/src/devtools.dart' as import9;
import 'package:ngforms/src/directives/control_container.dart' as import10;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import12;

final List<Object> styles$A22FormComFormsDirectives = const [];

class ViewA22FormComFormsDirectives0 extends import0.ComponentView<import1.A22FormComFormsDirectives> {
  late final import2.NgForm _NgForm_0_5;
  static import3.ComponentStyles? _componentStyles;
  ViewA22FormComFormsDirectives0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('a22-form-com-forms-directives'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/a22_form_com_forms_directives.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendElement<import7.FormElement>(doc, parentRenderNode, 'form');
    this._NgForm_0_5 = import2.NgForm(null, this);
    if (import9.isDevToolsEnabled) {
      import9.Inspector.instance.registerDirective(_el_0, this._NgForm_0_5);
    }
    final _el_1 = import8.appendDiv(doc, _el_0);
    final _text_2 = import8.appendText(_el_1, 'x');
    _el_0.addEventListener('submit', this.eventHandler1(this._NgForm_0_5.onSubmit));
    _el_0.addEventListener('reset', this.eventHandler1(this._NgForm_0_5.onReset));
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if (((identical(token, import2.NgForm) || identical(token, import10.ControlContainer)) && (nodeIndex <= 2))) {
      return this._NgForm_0_5;
    }
    return notFoundResult;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$A22FormComFormsDirectives, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _A22FormComFormsDirectivesNgFactory = ComponentFactory<import1.A22FormComFormsDirectives>('a22-form-com-forms-directives', viewFactory_A22FormComFormsDirectivesHost0);
ComponentFactory<import1.A22FormComFormsDirectives> get A22FormComFormsDirectivesNgFactory {
  return _A22FormComFormsDirectivesNgFactory;
}

ComponentFactory<import1.A22FormComFormsDirectives> createA22FormComFormsDirectivesFactory() {
  return ComponentFactory('a22-form-com-forms-directives', viewFactory_A22FormComFormsDirectivesHost0);
}

final List<Object> styles$A22FormComFormsDirectivesHost = const [];

class _ViewA22FormComFormsDirectivesHost0 extends import12.HostView<import1.A22FormComFormsDirectives> {
  @override
  void build() {
    this.componentView = ViewA22FormComFormsDirectives0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.A22FormComFormsDirectives();
    this.initRootNode(_el_0);
  }
}

import12.HostView<import1.A22FormComFormsDirectives> viewFactory_A22FormComFormsDirectivesHost0() {
  return _ViewA22FormComFormsDirectivesHost0();
}
