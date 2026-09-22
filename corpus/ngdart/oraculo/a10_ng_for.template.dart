// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'a10_ng_for.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'a10_ng_for.dart' as import1;
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
import 'package:ngdart/src/runtime/text_binding.dart' as import15;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import16;
import 'dart:core';
import 'package:ngdart/src/runtime/interpolate.dart' as import18;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import19;

final List<Object> styles$A10NgFor = const [];

class ViewA10NgFor0 extends import0.ComponentView<import1.A10NgFor> {
  late final ViewContainer _appEl_0;
  late final import3.NgFor _NgFor_0_9;
  Object? _expr_0;
  static import4.ComponentStyles? _componentStyles;
  ViewA10NgFor0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('a10-ng-for'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/a10_ng_for.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import9.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_A10NgFor1);
    this._NgFor_0_9 = import3.NgFor(this._appEl_0, _TemplateRef_0_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_0, this._NgFor_0_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.itens;
    if (import12.checkBinding(this._expr_0, currVal_0, 'itens', 'package:corpus_ngdart/src/a10_ng_for.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgFor_0_9, 'ngForOf', currVal_0);
      }
      this._NgFor_0_9.ngForOf = currVal_0 /* REF:package:corpus_ngdart/src/a10_ng_for.html:5:31 */;
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
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$A10NgFor, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _A10NgForNgFactory = ComponentFactory<import1.A10NgFor>('a10-ng-for', viewFactory_A10NgForHost0);
ComponentFactory<import1.A10NgFor> get A10NgForNgFactory {
  return _A10NgForNgFactory;
}

ComponentFactory<import1.A10NgFor> createA10NgForFactory() {
  return ComponentFactory('a10-ng-for', viewFactory_A10NgForHost0);
}

class _ViewA10NgFor1 extends import14.EmbeddedView<import1.A10NgFor> {
  final import15.TextBinding _textBinding_1 = import15.TextBinding();
  _ViewA10NgFor1(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('div'));
    _el_0.append(this._textBinding_1.element);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_item = import7.unsafeCast<String>(this.locals['\$implicit']);
    this._textBinding_1.updateText(import18.interpolateString0(local_item)) /* REF:package:corpus_ngdart/src/a10_ng_for.html:32:40 */;
  }
}

import14.EmbeddedView<void> viewFactory_A10NgFor1(import16.RenderView parentView, int parentIndex) {
  return _ViewA10NgFor1(parentView, parentIndex);
}

final List<Object> styles$A10NgForHost = const [];

class _ViewA10NgForHost0 extends import19.HostView<import1.A10NgFor> {
  @override
  void build() {
    this.componentView = ViewA10NgFor0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.A10NgFor();
    this.initRootNode(_el_0);
  }
}

import19.HostView<import1.A10NgFor> viewFactory_A10NgForHost0() {
  return _ViewA10NgForHost0();
}
