// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'f01_if_com_for.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'f01_if_com_for.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import11;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import13;
import 'package:ngdart/src/common/directives/ng_for.dart' as import14;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import15;
import 'package:ngdart/src/runtime/check_binding.dart' as import16;
import 'package:ngdart/src/runtime/text_binding.dart' as import17;
import 'dart:core';
import 'package:ngdart/src/runtime/interpolate.dart' as import19;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import20;

final List<Object> styles$F01IfComFor = const [];

class ViewF01IfComFor0 extends import0.ComponentView<import1.F01IfComFor> {
  late final ViewContainer _appEl_0;
  late final NgIf _NgIf_0_9;
  static import4.ComponentStyles? _componentStyles;
  ViewF01IfComFor0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('f01-if-com-for'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/f01_if_com_for.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import9.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_F01IfComFor1);
    this._NgIf_0_9 = NgIf(this._appEl_0, _TemplateRef_0_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_0, this._NgIf_0_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_0_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_0_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/f01_if_com_for.html:5:20 */;
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
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$F01IfComFor, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _F01IfComForNgFactory = ComponentFactory<import1.F01IfComFor>('f01-if-com-for', viewFactory_F01IfComForHost0);
ComponentFactory<import1.F01IfComFor> get F01IfComForNgFactory {
  return _F01IfComForNgFactory;
}

ComponentFactory<import1.F01IfComFor> createF01IfComForFactory() {
  return ComponentFactory('f01-if-com-for', viewFactory_F01IfComForHost0);
}

class _ViewF01IfComFor1 extends import13.EmbeddedView<import1.F01IfComFor> {
  late final ViewContainer _appEl_1;
  late final import14.NgFor _NgFor_1_9;
  Object? _expr_0;
  _ViewF01IfComFor1(import15.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('div'));
    final _anchor_1 = import9.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_F01IfComFor2);
    this._NgFor_1_9 = import14.NgFor(this._appEl_1, _TemplateRef_1_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_1, this._NgFor_1_9);
    }
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.itens;
    if (import16.checkBinding(this._expr_0, currVal_0, 'itens', 'package:corpus_ngdart/src/f01_if_com_for.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgFor_1_9, 'ngForOf', currVal_0);
      }
      this._NgFor_1_9.ngForOf = currVal_0 /* REF:package:corpus_ngdart/src/f01_if_com_for.html:27:53 */;
      this._expr_0 = currVal_0;
    }
    if ((!import16.debugThrowIfChanged)) {
      this._NgFor_1_9.ngDoCheck();
    }
    this._appEl_1.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
  }
}

import13.EmbeddedView<void> viewFactory_F01IfComFor1(import15.RenderView parentView, int parentIndex) {
  return _ViewF01IfComFor1(parentView, parentIndex);
}

class _ViewF01IfComFor2 extends import13.EmbeddedView<import1.F01IfComFor> {
  final import17.TextBinding _textBinding_1 = import17.TextBinding();
  _ViewF01IfComFor2(import15.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('span'));
    _el_0.append(this._textBinding_1.element);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_item = import7.unsafeCast<String>(this.locals['\$implicit']);
    this._textBinding_1.updateText(import19.interpolateString0(local_item)) /* REF:package:corpus_ngdart/src/f01_if_com_for.html:54:62 */;
  }
}

import13.EmbeddedView<void> viewFactory_F01IfComFor2(import15.RenderView parentView, int parentIndex) {
  return _ViewF01IfComFor2(parentView, parentIndex);
}

final List<Object> styles$F01IfComForHost = const [];

class _ViewF01IfComForHost0 extends import20.HostView<import1.F01IfComFor> {
  @override
  void build() {
    this.componentView = ViewF01IfComFor0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.F01IfComFor();
    this.initRootNode(_el_0);
  }
}

import20.HostView<import1.F01IfComFor> viewFactory_F01IfComForHost0() {
  return _ViewF01IfComForHost0();
}
