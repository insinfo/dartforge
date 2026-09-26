// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i40_ng_container_texto.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i40_ng_container_texto.dart' as import1;
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

final List<Object> styles$I40NgContainerTexto = const [];

class ViewI40NgContainerTexto0 extends import0.ComponentView<import1.I40NgContainerTexto> {
  late final ViewContainer _appEl_0;
  late final NgIf _NgIf_0_9;
  static import4.ComponentStyles? _componentStyles;
  ViewI40NgContainerTexto0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('i40-ng-container-texto'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/i40_ng_container_texto.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import9.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_I40NgContainerTexto1);
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
    this._NgIf_0_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/i40_ng_container_texto.html:14:29 */;
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
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$I40NgContainerTexto, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I40NgContainerTextoNgFactory = ComponentFactory<import1.I40NgContainerTexto>('i40-ng-container-texto', viewFactory_I40NgContainerTextoHost0);
ComponentFactory<import1.I40NgContainerTexto> get I40NgContainerTextoNgFactory {
  return _I40NgContainerTextoNgFactory;
}

ComponentFactory<import1.I40NgContainerTexto> createI40NgContainerTextoFactory() {
  return ComponentFactory('i40-ng-container-texto', viewFactory_I40NgContainerTextoHost0);
}

class _ViewI40NgContainerTexto1 extends import13.EmbeddedView<import1.I40NgContainerTexto> {
  _ViewI40NgContainerTexto1(import14.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final _text_0 = import9.createText('texto ');
    final doc = import8.document;
    final _el_1 = import7.unsafeCast(doc.createElement('b'));
    final _text_2 = import9.appendText(_el_1, 'x');
    this.initRootNodesAndSubscriptions(import7.unsafeCast(<Object>[_text_0, _el_1]), null);
  }
}

import13.EmbeddedView<void> viewFactory_I40NgContainerTexto1(import14.RenderView parentView, int parentIndex) {
  return _ViewI40NgContainerTexto1(parentView, parentIndex);
}

final List<Object> styles$I40NgContainerTextoHost = const [];

class _ViewI40NgContainerTextoHost0 extends import15.HostView<import1.I40NgContainerTexto> {
  @override
  void build() {
    this.componentView = ViewI40NgContainerTexto0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I40NgContainerTexto();
    this.initRootNode(_el_0);
  }
}

import15.HostView<import1.I40NgContainerTexto> viewFactory_I40NgContainerTextoHost0() {
  return _ViewI40NgContainerTextoHost0();
}
