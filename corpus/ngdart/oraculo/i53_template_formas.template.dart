// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i53_template_formas.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i53_template_formas.dart' as import1;
import 'package:ngdart/src/common/directives/ng_switch.dart' as import2;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import5;
import 'package:ngdart/src/core/linker/views/view.dart' as import6;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import7;
import 'package:ngdart/src/utilities.dart' as import8;
import 'dart:html' as import9;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import10;
import 'package:ngdart/src/devtools.dart' as import11;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/runtime/check_binding.dart' as import13;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import15;
import 'package:ngdart/src/runtime/text_binding.dart' as import16;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import17;
import 'package:ngdart/src/runtime/interpolate.dart' as import18;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import19;

final List<Object> styles$I53TemplateFormas = const [];

class ViewI53TemplateFormas0 extends import0.ComponentView<import1.I53TemplateFormas> {
  late final import2.NgSwitch _NgSwitch_0_5;
  late final ViewContainer _appEl_1;
  late final import2.NgSwitchWhen _NgSwitchWhen_1_9;
  late final ViewContainer _appEl_2;
  late final import2.NgSwitchDefault _NgSwitchDefault_2_9;
  late final ViewContainer _appEl_3;
  late final NgIf _NgIf_3_9;
  Object? _expr_0;
  static import5.ComponentStyles? _componentStyles;
  ViewI53TemplateFormas0(import6.View parentView, int parentIndex) : super(parentView, parentIndex, import7.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import8.unsafeCast(import9.document.createElement('i53-template-formas'));
  }
  static String? get _debugComponentUrl {
    return (import8.isDevMode ? 'asset:corpus_ngdart/lib/src/i53_template_formas.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import9.document;
    final _el_0 = import10.appendDiv(doc, parentRenderNode);
    this._NgSwitch_0_5 = import2.NgSwitch();
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_el_0, this._NgSwitch_0_5);
    }
    final _anchor_1 = import10.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_I53TemplateFormas1);
    this._NgSwitchWhen_1_9 = import2.NgSwitchWhen(this._appEl_1, _TemplateRef_1_8, this._NgSwitch_0_5);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_1, this._NgSwitchWhen_1_9);
    }
    final _anchor_2 = import10.appendAnchor(_el_0);
    this._appEl_2 = ViewContainer(2, 0, this, _anchor_2);
    var _TemplateRef_2_8 = TemplateRef(this._appEl_2, viewFactory_I53TemplateFormas2);
    this._NgSwitchDefault_2_9 = import2.NgSwitchDefault(this._appEl_2, _TemplateRef_2_8, this._NgSwitch_0_5);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_2, this._NgSwitchDefault_2_9);
    }
    final _anchor_3 = import10.appendAnchor(parentRenderNode);
    this._appEl_3 = ViewContainer(3, null, this, _anchor_3);
    var _TemplateRef_3_8 = TemplateRef(this._appEl_3, viewFactory_I53TemplateFormas3);
    this._NgIf_3_9 = NgIf(this._appEl_3, _TemplateRef_3_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_3, this._NgIf_3_9);
    }
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((identical(token, import2.NgSwitch) && (nodeIndex <= 2))) {
      return this._NgSwitch_0_5;
    }
    return notFoundResult;
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool firstCheck = this.firstCheck;
    final currVal_0 = _ctx.modo;
    if (import13.checkBinding(this._expr_0, currVal_0, 'modo', 'package:corpus_ngdart/src/i53_template_formas.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgSwitch_0_5, 'ngSwitch', currVal_0);
      }
      this._NgSwitch_0_5.ngSwitch = currVal_0 /* REF:package:corpus_ngdart/src/i53_template_formas.html:5:22 */;
      this._expr_0 = currVal_0;
    }
    if (firstCheck) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgSwitchWhen_1_9, 'ngSwitchCase', 'a');
      }
      this._NgSwitchWhen_1_9.ngSwitchCase = 'a' /* REF:package:corpus_ngdart/src/i53_template_formas.html:33:53 */;
    }
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_3_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_3_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/i53_template_formas.html:148:164 */;
    this._appEl_1.detectChangesInNestedViews();
    this._appEl_2.detectChangesInNestedViews();
    this._appEl_3.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
    this._appEl_2.destroyNestedViews();
    this._appEl_3.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import5.ComponentStyles.unscoped(styles$I53TemplateFormas, _debugComponentUrl));
      if (import8.isDevMode) {
        import5.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I53TemplateFormasNgFactory = ComponentFactory<import1.I53TemplateFormas>('i53-template-formas', viewFactory_I53TemplateFormasHost0);
ComponentFactory<import1.I53TemplateFormas> get I53TemplateFormasNgFactory {
  return _I53TemplateFormasNgFactory;
}

ComponentFactory<import1.I53TemplateFormas> createI53TemplateFormasFactory() {
  return ComponentFactory('i53-template-formas', viewFactory_I53TemplateFormasHost0);
}

class _ViewI53TemplateFormas1 extends import15.EmbeddedView<import1.I53TemplateFormas> {
  final import16.TextBinding _textBinding_1 = import16.TextBinding();
  _ViewI53TemplateFormas1(import17.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final _text_0 = import10.createText('A ');
    final doc = import9.document;
    final _el_2 = import8.unsafeCast(doc.createElement('b'));
    final _text_3 = import10.appendText(_el_2, '!');
    this.initRootNodesAndSubscriptions(import8.unsafeCast(<Object>[_text_0, this._textBinding_1.element, _el_2]), null);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateText(import18.interpolateString0(_ctx.modo)) /* REF:package:corpus_ngdart/src/i53_template_formas.html:56:64 */;
  }
}

import15.EmbeddedView<void> viewFactory_I53TemplateFormas1(import17.RenderView parentView, int parentIndex) {
  return _ViewI53TemplateFormas1(parentView, parentIndex);
}

class _ViewI53TemplateFormas2 extends import15.EmbeddedView<import1.I53TemplateFormas> {
  _ViewI53TemplateFormas2(import17.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import9.document;
    final _el_0 = import8.unsafeCast(doc.createElement('i'));
    final _text_1 = import10.appendText(_el_0, 'outro');
    this.initRootNode(_el_0);
  }
}

import15.EmbeddedView<void> viewFactory_I53TemplateFormas2(import17.RenderView parentView, int parentIndex) {
  return _ViewI53TemplateFormas2(parentView, parentIndex);
}

class _ViewI53TemplateFormas3 extends import15.EmbeddedView<import1.I53TemplateFormas> {
  _ViewI53TemplateFormas3(import17.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final _text_0 = import10.createText('texto');
    this.initRootNode(_text_0);
  }
}

import15.EmbeddedView<void> viewFactory_I53TemplateFormas3(import17.RenderView parentView, int parentIndex) {
  return _ViewI53TemplateFormas3(parentView, parentIndex);
}

final List<Object> styles$I53TemplateFormasHost = const [];

class _ViewI53TemplateFormasHost0 extends import19.HostView<import1.I53TemplateFormas> {
  @override
  void build() {
    this.componentView = ViewI53TemplateFormas0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I53TemplateFormas();
    this.initRootNode(_el_0);
  }
}

import19.HostView<import1.I53TemplateFormas> viewFactory_I53TemplateFormasHost0() {
  return _ViewI53TemplateFormasHost0();
}
