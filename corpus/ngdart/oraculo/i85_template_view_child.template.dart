// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i85_template_view_child.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i85_template_view_child.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import11;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import12;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import13;

final List<Object> styles$I85TemplateViewChild = const [];

class ViewI85TemplateViewChild0 extends import0.ComponentView<import1.I85TemplateViewChild> {
  late final ViewContainer _appEl_0;
  late final TemplateRef _TemplateRef_0_7;
  static import4.ComponentStyles? _componentStyles;
  ViewI85TemplateViewChild0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('i85-template-view-child'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/i85_template_view_child.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import9.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    this._TemplateRef_0_7 = TemplateRef(this._appEl_0, viewFactory_I85TemplateViewChild1);
    _ctx.molde = this._TemplateRef_0_7;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$I85TemplateViewChild, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I85TemplateViewChildNgFactory = ComponentFactory<import1.I85TemplateViewChild>('i85-template-view-child', viewFactory_I85TemplateViewChildHost0);
ComponentFactory<import1.I85TemplateViewChild> get I85TemplateViewChildNgFactory {
  return _I85TemplateViewChildNgFactory;
}

ComponentFactory<import1.I85TemplateViewChild> createI85TemplateViewChildFactory() {
  return ComponentFactory('i85-template-view-child', viewFactory_I85TemplateViewChildHost0);
}

class _ViewI85TemplateViewChild1 extends import11.EmbeddedView<import1.I85TemplateViewChild> {
  _ViewI85TemplateViewChild1(import12.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('p'));
    final _text_1 = import9.appendText(_el_0, 'x');
    this.initRootNode(_el_0);
  }
}

import11.EmbeddedView<void> viewFactory_I85TemplateViewChild1(import12.RenderView parentView, int parentIndex) {
  return _ViewI85TemplateViewChild1(parentView, parentIndex);
}

final List<Object> styles$I85TemplateViewChildHost = const [];

class _ViewI85TemplateViewChildHost0 extends import13.HostView<import1.I85TemplateViewChild> {
  @override
  void build() {
    this.componentView = ViewI85TemplateViewChild0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I85TemplateViewChild();
    this.initRootNode(_el_0);
  }
}

import13.HostView<import1.I85TemplateViewChild> viewFactory_I85TemplateViewChildHost0() {
  return _ViewI85TemplateViewChildHost0();
}
