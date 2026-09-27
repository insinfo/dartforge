// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j13_ref_com_membro.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j13_ref_com_membro.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'dart:html' as import5;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import6;
import 'package:ngdart/src/core/linker/views/view.dart' as import7;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import8;
import 'package:ngdart/src/utilities.dart' as import9;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import10;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import12;
import 'package:ngdart/src/runtime/interpolate.dart' as import13;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import15;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import16;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import17;

final List<Object> styles$J13RefComMembro = const [];

class ViewJ13RefComMembro0 extends import0.ComponentView<import1.J13RefComMembro> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  late final ViewContainer _appEl_2;
  late final NgIf _NgIf_2_9;
  late final import5.InputElement _el_0;
  static import6.ComponentStyles? _componentStyles;
  ViewJ13RefComMembro0(import7.View parentView, int parentIndex) : super(parentView, parentIndex, import8.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import9.unsafeCast(import5.document.createElement('j13-ref-com-membro'));
  }
  static String? get _debugComponentUrl {
    return (import9.isDevMode ? 'asset:corpus_ngdart/lib/src/j13_ref_com_membro.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import5.document;
    this._el_0 = import10.appendElement<import5.InputElement>(doc, parentRenderNode, 'input');
    import10.setAttribute(this._el_0, 'value', 'no');
    parentRenderNode.append(this._textBinding_1.element);
    final _anchor_2 = import10.appendAnchor(parentRenderNode);
    this._appEl_2 = ViewContainer(2, null, this, _anchor_2);
    var _TemplateRef_2_8 = TemplateRef(this._appEl_2, viewFactory_J13RefComMembro1);
    this._NgIf_2_9 = NgIf(this._appEl_2, _TemplateRef_2_8);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_anchor_2, this._NgIf_2_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final local_campo = this._el_0;
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.recordInput(this._NgIf_2_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_2_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/j13_ref_com_membro.html:37:52 */;
    this._appEl_2.detectChangesInNestedViews();
    this._textBinding_1.updateText(import13.interpolate0(local_campo)) /* REF:package:corpus_ngdart/src/j13_ref_com_membro.html:25:34 */;
  }

  @override
  void destroyInternal() {
    this._appEl_2.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import6.ComponentStyles.unscoped(styles$J13RefComMembro, _debugComponentUrl));
      if (import9.isDevMode) {
        import6.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J13RefComMembroNgFactory = ComponentFactory<import1.J13RefComMembro>('j13-ref-com-membro', viewFactory_J13RefComMembroHost0);
ComponentFactory<import1.J13RefComMembro> get J13RefComMembroNgFactory {
  return _J13RefComMembroNgFactory;
}

ComponentFactory<import1.J13RefComMembro> createJ13RefComMembroFactory() {
  return ComponentFactory('j13-ref-com-membro', viewFactory_J13RefComMembroHost0);
}

class _ViewJ13RefComMembro1 extends import15.EmbeddedView<import1.J13RefComMembro> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  _ViewJ13RefComMembro1(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import5.document;
    final _el_0 = import9.unsafeCast(doc.createElement('p'));
    _el_0.append(this._textBinding_1.element);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_campo = import9.unsafeCast<ViewJ13RefComMembro0>((this.parentView!))._el_0;
    this._textBinding_1.updateText(import13.interpolate0(local_campo)) /* REF:package:corpus_ngdart/src/j13_ref_com_membro.html:53:62 */;
  }
}

import15.EmbeddedView<void> viewFactory_J13RefComMembro1(import16.RenderView parentView, int parentIndex) {
  return _ViewJ13RefComMembro1(parentView, parentIndex);
}

final List<Object> styles$J13RefComMembroHost = const [];

class _ViewJ13RefComMembroHost0 extends import17.HostView<import1.J13RefComMembro> {
  @override
  void build() {
    this.componentView = ViewJ13RefComMembro0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J13RefComMembro();
    this.initRootNode(_el_0);
  }
}

import17.HostView<import1.J13RefComMembro> viewFactory_J13RefComMembroHost0() {
  return _ViewJ13RefComMembroHost0();
}
