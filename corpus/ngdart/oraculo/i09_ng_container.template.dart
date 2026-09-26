// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i09_ng_container.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i09_ng_container.dart' as import1;
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

final List<Object> styles$I09NgContainer = const [];

class ViewI09NgContainer0 extends import0.ComponentView<import1.I09NgContainer> {
  late final ViewContainer _appEl_0;
  late final NgIf _NgIf_0_9;
  static import4.ComponentStyles? _componentStyles;
  ViewI09NgContainer0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('i09-ng-container'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/i09_ng_container.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import9.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_I09NgContainer1);
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
    this._NgIf_0_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/i09_ng_container.html:14:29 */;
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
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$I09NgContainer, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I09NgContainerNgFactory = ComponentFactory<import1.I09NgContainer>('i09-ng-container', viewFactory_I09NgContainerHost0);
ComponentFactory<import1.I09NgContainer> get I09NgContainerNgFactory {
  return _I09NgContainerNgFactory;
}

ComponentFactory<import1.I09NgContainer> createI09NgContainerFactory() {
  return ComponentFactory('i09-ng-container', viewFactory_I09NgContainerHost0);
}

class _ViewI09NgContainer1 extends import13.EmbeddedView<import1.I09NgContainer> {
  _ViewI09NgContainer1(import14.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('p'));
    final _text_1 = import9.appendText(_el_0, 'a');
    final _el_2 = import7.unsafeCast(doc.createElement('p'));
    final _text_3 = import9.appendText(_el_2, 'b');
    this.initRootNodesAndSubscriptions(import7.unsafeCast(<Object>[_el_0, _el_2]), null);
  }
}

import13.EmbeddedView<void> viewFactory_I09NgContainer1(import14.RenderView parentView, int parentIndex) {
  return _ViewI09NgContainer1(parentView, parentIndex);
}

final List<Object> styles$I09NgContainerHost = const [];

class _ViewI09NgContainerHost0 extends import15.HostView<import1.I09NgContainer> {
  @override
  void build() {
    this.componentView = ViewI09NgContainer0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I09NgContainer();
    this.initRootNode(_el_0);
  }
}

import15.HostView<import1.I09NgContainer> viewFactory_I09NgContainerHost0() {
  return _ViewI09NgContainerHost0();
}
