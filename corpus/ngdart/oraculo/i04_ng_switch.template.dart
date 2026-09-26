// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i04_ng_switch.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i04_ng_switch.dart' as import1;
import 'package:ngdart/src/common/directives/ng_switch.dart' as import2;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/src/devtools.dart' as import10;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/runtime/check_binding.dart' as import12;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import14;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import15;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import16;

final List<Object> styles$I04NgSwitch = const [];

class ViewI04NgSwitch0 extends import0.ComponentView<import1.I04NgSwitch> {
  late final import2.NgSwitch _NgSwitch_0_5;
  late final ViewContainer _appEl_1;
  late final import2.NgSwitchWhen _NgSwitchWhen_1_9;
  late final ViewContainer _appEl_2;
  late final import2.NgSwitchWhen _NgSwitchWhen_2_9;
  late final ViewContainer _appEl_3;
  late final import2.NgSwitchDefault _NgSwitchDefault_3_9;
  Object? _expr_0;
  static import4.ComponentStyles? _componentStyles;
  ViewI04NgSwitch0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('i04-ng-switch'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/i04_ng_switch.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import8.document;
    final _el_0 = import9.appendDiv(doc, parentRenderNode);
    this._NgSwitch_0_5 = import2.NgSwitch();
    if (import10.isDevToolsEnabled) {
      import10.Inspector.instance.registerDirective(_el_0, this._NgSwitch_0_5);
    }
    final _anchor_1 = import9.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_I04NgSwitch1);
    this._NgSwitchWhen_1_9 = import2.NgSwitchWhen(this._appEl_1, _TemplateRef_1_8, this._NgSwitch_0_5);
    if (import10.isDevToolsEnabled) {
      import10.Inspector.instance.registerDirective(_anchor_1, this._NgSwitchWhen_1_9);
    }
    final _anchor_2 = import9.appendAnchor(_el_0);
    this._appEl_2 = ViewContainer(2, 0, this, _anchor_2);
    var _TemplateRef_2_8 = TemplateRef(this._appEl_2, viewFactory_I04NgSwitch2);
    this._NgSwitchWhen_2_9 = import2.NgSwitchWhen(this._appEl_2, _TemplateRef_2_8, this._NgSwitch_0_5);
    if (import10.isDevToolsEnabled) {
      import10.Inspector.instance.registerDirective(_anchor_2, this._NgSwitchWhen_2_9);
    }
    final _anchor_3 = import9.appendAnchor(_el_0);
    this._appEl_3 = ViewContainer(3, 0, this, _anchor_3);
    var _TemplateRef_3_8 = TemplateRef(this._appEl_3, viewFactory_I04NgSwitch3);
    this._NgSwitchDefault_3_9 = import2.NgSwitchDefault(this._appEl_3, _TemplateRef_3_8, this._NgSwitch_0_5);
    if (import10.isDevToolsEnabled) {
      import10.Inspector.instance.registerDirective(_anchor_3, this._NgSwitchDefault_3_9);
    }
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((identical(token, import2.NgSwitch) && (nodeIndex <= 3))) {
      return this._NgSwitch_0_5;
    }
    return notFoundResult;
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool firstCheck = this.firstCheck;
    final currVal_0 = _ctx.modo;
    if (import12.checkBinding(this._expr_0, currVal_0, 'modo', 'package:corpus_ngdart/src/i04_ng_switch.html')) {
      if (import10.isDevToolsEnabled) {
        import10.Inspector.instance.recordInput(this._NgSwitch_0_5, 'ngSwitch', currVal_0);
      }
      this._NgSwitch_0_5.ngSwitch = currVal_0 /* REF:package:corpus_ngdart/src/i04_ng_switch.html:5:22 */;
      this._expr_0 = currVal_0;
    }
    if (firstCheck) {
      if (import10.isDevToolsEnabled) {
        import10.Inspector.instance.recordInput(this._NgSwitchWhen_1_9, 'ngSwitchCase', 'a');
      }
      this._NgSwitchWhen_1_9.ngSwitchCase = 'a' /* REF:package:corpus_ngdart/src/i04_ng_switch.html:26:45 */;
      if (import10.isDevToolsEnabled) {
        import10.Inspector.instance.recordInput(this._NgSwitchWhen_2_9, 'ngSwitchCase', 'b');
      }
      this._NgSwitchWhen_2_9.ngSwitchCase = 'b' /* REF:package:corpus_ngdart/src/i04_ng_switch.html:54:73 */;
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

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$I04NgSwitch, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I04NgSwitchNgFactory = ComponentFactory<import1.I04NgSwitch>('i04-ng-switch', viewFactory_I04NgSwitchHost0);
ComponentFactory<import1.I04NgSwitch> get I04NgSwitchNgFactory {
  return _I04NgSwitchNgFactory;
}

ComponentFactory<import1.I04NgSwitch> createI04NgSwitchFactory() {
  return ComponentFactory('i04-ng-switch', viewFactory_I04NgSwitchHost0);
}

class _ViewI04NgSwitch1 extends import14.EmbeddedView<import1.I04NgSwitch> {
  _ViewI04NgSwitch1(import15.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('p'));
    final _text_1 = import9.appendText(_el_0, 'A');
    this.initRootNode(_el_0);
  }
}

import14.EmbeddedView<void> viewFactory_I04NgSwitch1(import15.RenderView parentView, int parentIndex) {
  return _ViewI04NgSwitch1(parentView, parentIndex);
}

class _ViewI04NgSwitch2 extends import14.EmbeddedView<import1.I04NgSwitch> {
  _ViewI04NgSwitch2(import15.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('p'));
    final _text_1 = import9.appendText(_el_0, 'B');
    this.initRootNode(_el_0);
  }
}

import14.EmbeddedView<void> viewFactory_I04NgSwitch2(import15.RenderView parentView, int parentIndex) {
  return _ViewI04NgSwitch2(parentView, parentIndex);
}

class _ViewI04NgSwitch3 extends import14.EmbeddedView<import1.I04NgSwitch> {
  _ViewI04NgSwitch3(import15.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('p'));
    final _text_1 = import9.appendText(_el_0, 'outro');
    this.initRootNode(_el_0);
  }
}

import14.EmbeddedView<void> viewFactory_I04NgSwitch3(import15.RenderView parentView, int parentIndex) {
  return _ViewI04NgSwitch3(parentView, parentIndex);
}

final List<Object> styles$I04NgSwitchHost = const [];

class _ViewI04NgSwitchHost0 extends import16.HostView<import1.I04NgSwitch> {
  @override
  void build() {
    this.componentView = ViewI04NgSwitch0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I04NgSwitch();
    this.initRootNode(_el_0);
  }
}

import16.HostView<import1.I04NgSwitch> viewFactory_I04NgSwitchHost0() {
  return _ViewI04NgSwitchHost0();
}
