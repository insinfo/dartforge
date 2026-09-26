// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i42_ng_switch_formas.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i42_ng_switch_formas.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_for.dart' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import11;
import 'package:ngdart/src/runtime/check_binding.dart' as import12;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import14;
import 'package:ngdart/src/common/directives/ng_switch.dart' as import15;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import16;
import 'dart:core';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/runtime/text_binding.dart' as import19;
import 'package:ngdart/src/runtime/interpolate.dart' as import20;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import21;

final List<Object> styles$I42NgSwitchFormas = const [];

class ViewI42NgSwitchFormas0 extends import0.ComponentView<import1.I42NgSwitchFormas> {
  late final ViewContainer _appEl_0;
  late final import3.NgFor _NgFor_0_9;
  Object? _expr_0;
  static import4.ComponentStyles? _componentStyles;
  ViewI42NgSwitchFormas0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('i42-ng-switch-formas'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/i42_ng_switch_formas.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import9.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_I42NgSwitchFormas1);
    this._NgFor_0_9 = import3.NgFor(this._appEl_0, _TemplateRef_0_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_0, this._NgFor_0_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.itens;
    if (import12.checkBinding(this._expr_0, currVal_0, 'itens', 'package:corpus_ngdart/src/i42_ng_switch_formas.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgFor_0_9, 'ngForOf', currVal_0);
      }
      this._NgFor_0_9.ngForOf = currVal_0 /* REF:package:corpus_ngdart/src/i42_ng_switch_formas.html:5:28 */;
      this._expr_0 = currVal_0;
    }
    if ((!import12.debugThrowIfChanged)) {
      this._NgFor_0_9.ngDoCheck();
    }
    this._appEl_0.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_0.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$I42NgSwitchFormas, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I42NgSwitchFormasNgFactory = ComponentFactory<import1.I42NgSwitchFormas>('i42-ng-switch-formas', viewFactory_I42NgSwitchFormasHost0);
ComponentFactory<import1.I42NgSwitchFormas> get I42NgSwitchFormasNgFactory {
  return _I42NgSwitchFormasNgFactory;
}

ComponentFactory<import1.I42NgSwitchFormas> createI42NgSwitchFormasFactory() {
  return ComponentFactory('i42-ng-switch-formas', viewFactory_I42NgSwitchFormasHost0);
}

class _ViewI42NgSwitchFormas1 extends import14.EmbeddedView<import1.I42NgSwitchFormas> {
  late final import15.NgSwitch _NgSwitch_0_5;
  late final ViewContainer _appEl_1;
  late final import15.NgSwitchWhen _NgSwitchWhen_1_9;
  late final ViewContainer _appEl_2;
  late final import15.NgSwitchWhen _NgSwitchWhen_2_9;
  late final ViewContainer _appEl_3;
  late final import15.NgSwitchDefault _NgSwitchDefault_3_9;
  Object? _expr_0;
  Object? _expr_1;
  _ViewI42NgSwitchFormas1(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('div'));
    this._NgSwitch_0_5 = import15.NgSwitch();
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_el_0, this._NgSwitch_0_5);
    }
    final _anchor_1 = import9.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_I42NgSwitchFormas2);
    this._NgSwitchWhen_1_9 = import15.NgSwitchWhen(this._appEl_1, _TemplateRef_1_8, this._NgSwitch_0_5);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_1, this._NgSwitchWhen_1_9);
    }
    final _anchor_2 = import9.appendAnchor(_el_0);
    this._appEl_2 = ViewContainer(2, 0, this, _anchor_2);
    var _TemplateRef_2_8 = TemplateRef(this._appEl_2, viewFactory_I42NgSwitchFormas3);
    this._NgSwitchWhen_2_9 = import15.NgSwitchWhen(this._appEl_2, _TemplateRef_2_8, this._NgSwitch_0_5);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_2, this._NgSwitchWhen_2_9);
    }
    final _anchor_3 = import9.appendAnchor(_el_0);
    this._appEl_3 = ViewContainer(3, 0, this, _anchor_3);
    var _TemplateRef_3_8 = TemplateRef(this._appEl_3, viewFactory_I42NgSwitchFormas5);
    this._NgSwitchDefault_3_9 = import15.NgSwitchDefault(this._appEl_3, _TemplateRef_3_8, this._NgSwitch_0_5);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_3, this._NgSwitchDefault_3_9);
    }
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((identical(token, import15.NgSwitch) && (nodeIndex <= 3))) {
      return this._NgSwitch_0_5;
    }
    return notFoundResult;
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool firstCheck = this.firstCheck;
    final local_x = import7.unsafeCast<String>(this.locals['\$implicit']);
    final currVal_0 = local_x;
    if (import12.checkBinding(this._expr_0, currVal_0, 'x', 'package:corpus_ngdart/src/i42_ng_switch_formas.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgSwitch_0_5, 'ngSwitch', currVal_0);
      }
      this._NgSwitch_0_5.ngSwitch = currVal_0 /* REF:package:corpus_ngdart/src/i42_ng_switch_formas.html:29:43 */;
      this._expr_0 = currVal_0;
    }
    final currVal_1 = _ctx.modo;
    if (import12.checkBinding(this._expr_1, currVal_1, 'modo', 'package:corpus_ngdart/src/i42_ng_switch_formas.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgSwitchWhen_1_9, 'ngSwitchCase', currVal_1);
      }
      this._NgSwitchWhen_1_9.ngSwitchCase = currVal_1 /* REF:package:corpus_ngdart/src/i42_ng_switch_formas.html:50:70 */;
      this._expr_1 = currVal_1;
    }
    if (firstCheck) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgSwitchWhen_2_9, 'ngSwitchWhen', 1);
      }
      this._NgSwitchWhen_2_9.ngSwitchWhen = 1 /* REF:package:corpus_ngdart/src/i42_ng_switch_formas.html:86:103 */;
    }
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
}

import14.EmbeddedView<void> viewFactory_I42NgSwitchFormas1(import16.RenderView parentView, int parentIndex) {
  return _ViewI42NgSwitchFormas1(parentView, parentIndex);
}

class _ViewI42NgSwitchFormas2 extends import14.EmbeddedView<import1.I42NgSwitchFormas> {
  _ViewI42NgSwitchFormas2(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('span'));
    final _text_1 = import9.appendText(_el_0, 'igual');
    this.initRootNode(_el_0);
  }
}

import14.EmbeddedView<void> viewFactory_I42NgSwitchFormas2(import16.RenderView parentView, int parentIndex) {
  return _ViewI42NgSwitchFormas2(parentView, parentIndex);
}

class _ViewI42NgSwitchFormas3 extends import14.EmbeddedView<import1.I42NgSwitchFormas> {
  late final ViewContainer _appEl_1;
  late final NgIf _NgIf_1_9;
  _ViewI42NgSwitchFormas3(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('p'));
    final _anchor_1 = import9.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_I42NgSwitchFormas4);
    this._NgIf_1_9 = NgIf(this._appEl_1, _TemplateRef_1_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_1, this._NgIf_1_9);
    }
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_1_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_1_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/i42_ng_switch_formas.html:107:122 */;
    this._appEl_1.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
  }
}

import14.EmbeddedView<void> viewFactory_I42NgSwitchFormas3(import16.RenderView parentView, int parentIndex) {
  return _ViewI42NgSwitchFormas3(parentView, parentIndex);
}

class _ViewI42NgSwitchFormas4 extends import14.EmbeddedView<import1.I42NgSwitchFormas> {
  final import19.TextBinding _textBinding_1 = import19.TextBinding();
  _ViewI42NgSwitchFormas4(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('b'));
    _el_0.append(this._textBinding_1.element);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_x = import7.unsafeCast<String>(import7.unsafeCast<_ViewI42NgSwitchFormas1>(((this.parentView!).parentView!)).locals['\$implicit']);
    this._textBinding_1.updateText(import20.interpolateString0(local_x)) /* REF:package:corpus_ngdart/src/i42_ng_switch_formas.html:123:128 */;
  }
}

import14.EmbeddedView<void> viewFactory_I42NgSwitchFormas4(import16.RenderView parentView, int parentIndex) {
  return _ViewI42NgSwitchFormas4(parentView, parentIndex);
}

class _ViewI42NgSwitchFormas5 extends import14.EmbeddedView<import1.I42NgSwitchFormas> {
  final import19.TextBinding _textBinding_1 = import19.TextBinding();
  _ViewI42NgSwitchFormas5(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('i'));
    _el_0.append(this._textBinding_1.element);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_x = import7.unsafeCast<String>(import7.unsafeCast<_ViewI42NgSwitchFormas1>((this.parentView!)).locals['\$implicit']);
    this._textBinding_1.updateText(import20.interpolateString0(local_x)) /* REF:package:corpus_ngdart/src/i42_ng_switch_formas.html:156:161 */;
  }
}

import14.EmbeddedView<void> viewFactory_I42NgSwitchFormas5(import16.RenderView parentView, int parentIndex) {
  return _ViewI42NgSwitchFormas5(parentView, parentIndex);
}

final List<Object> styles$I42NgSwitchFormasHost = const [];

class _ViewI42NgSwitchFormasHost0 extends import21.HostView<import1.I42NgSwitchFormas> {
  @override
  void build() {
    this.componentView = ViewI42NgSwitchFormas0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I42NgSwitchFormas();
    this.initRootNode(_el_0);
  }
}

import21.HostView<import1.I42NgSwitchFormas> viewFactory_I42NgSwitchFormasHost0() {
  return _ViewI42NgSwitchFormasHost0();
}
