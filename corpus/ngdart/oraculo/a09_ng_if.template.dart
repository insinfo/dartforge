// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'a09_ng_if.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'a09_ng_if.dart' as import1;
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
import 'package:ngdart/src/core/linker/views/render_view.dart' as import14;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import15;

final List<Object> styles$A09NgIf = const [];

class ViewA09NgIf0 extends import0.ComponentView<import1.A09NgIf> {
  late final ViewContainer _appEl_0;
  late final NgIf _NgIf_0_9;
  static import4.ComponentStyles? _componentStyles;
  ViewA09NgIf0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('a09-ng-if'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/a09_ng_if.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import9.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_A09NgIf1);
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
    this._NgIf_0_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/a09_ng_if.html:5:20 */;
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
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$A09NgIf, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _A09NgIfNgFactory = ComponentFactory<import1.A09NgIf>('a09-ng-if', viewFactory_A09NgIfHost0);
ComponentFactory<import1.A09NgIf> get A09NgIfNgFactory {
  return _A09NgIfNgFactory;
}

ComponentFactory<import1.A09NgIf> createA09NgIfFactory() {
  return ComponentFactory('a09-ng-if', viewFactory_A09NgIfHost0);
}

class _ViewA09NgIf1 extends import13.EmbeddedView<import1.A09NgIf> {
  _ViewA09NgIf1(import14.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('div'));
    final _text_1 = import9.appendText(_el_0, 'oi');
    this.initRootNode(_el_0);
  }
}

import13.EmbeddedView<void> viewFactory_A09NgIf1(import14.RenderView parentView, int parentIndex) {
  return _ViewA09NgIf1(parentView, parentIndex);
}

final List<Object> styles$A09NgIfHost = const [];

class _ViewA09NgIfHost0 extends import15.HostView<import1.A09NgIf> {
  @override
  void build() {
    this.componentView = ViewA09NgIf0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.A09NgIf();
    this.initRootNode(_el_0);
  }
}

import15.HostView<import1.A09NgIf> viewFactory_A09NgIfHost0() {
  return _ViewA09NgIfHost0();
}
