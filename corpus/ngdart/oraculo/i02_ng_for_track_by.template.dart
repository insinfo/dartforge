// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i02_ng_for_track_by.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i02_ng_for_track_by.dart' as import1;
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

final List<Object> styles$I02NgForTrackBy = const [];

class ViewI02NgForTrackBy0 extends import0.ComponentView<import1.I02NgForTrackBy> {
  late final ViewContainer _appEl_1;
  late final import3.NgFor _NgFor_1_9;
  Object? _expr_0;
  static import4.ComponentStyles? _componentStyles;
  ViewI02NgForTrackBy0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('i02-ng-for-track-by'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/i02_ng_for_track_by.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import8.document;
    final _el_0 = import9.appendElement<import8.UListElement>(doc, parentRenderNode, 'ul');
    final _anchor_1 = import9.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_I02NgForTrackBy1);
    this._NgFor_1_9 = import3.NgFor(this._appEl_1, _TemplateRef_1_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_1, this._NgFor_1_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool firstCheck = this.firstCheck;
    if (firstCheck) {
      if ((_ctx.rastrear != null)) {
        if (import11.isDevToolsEnabled) {
          import11.Inspector.instance.recordInput(this._NgFor_1_9, 'ngForTrackBy', _ctx.rastrear);
        }
        this._NgFor_1_9.ngForTrackBy = _ctx.rastrear /* REF:package:corpus_ngdart/src/i02_ng_for_track_by.html:8:53 */;
      }
    }
    final currVal_0 = _ctx.itens;
    if (import12.checkBinding(this._expr_0, currVal_0, 'itens', 'package:corpus_ngdart/src/i02_ng_for_track_by.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgFor_1_9, 'ngForOf', currVal_0);
      }
      this._NgFor_1_9.ngForOf = currVal_0 /* REF:package:corpus_ngdart/src/i02_ng_for_track_by.html:8:53 */;
      this._expr_0 = currVal_0;
    }
    if ((!import12.debugThrowIfChanged)) {
      this._NgFor_1_9.ngDoCheck();
    }
    this._appEl_1.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$I02NgForTrackBy, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I02NgForTrackByNgFactory = ComponentFactory<import1.I02NgForTrackBy>('i02-ng-for-track-by', viewFactory_I02NgForTrackByHost0);
ComponentFactory<import1.I02NgForTrackBy> get I02NgForTrackByNgFactory {
  return _I02NgForTrackByNgFactory;
}

ComponentFactory<import1.I02NgForTrackBy> createI02NgForTrackByFactory() {
  return ComponentFactory('i02-ng-for-track-by', viewFactory_I02NgForTrackByHost0);
}

class _ViewI02NgForTrackBy1 extends import14.EmbeddedView<import1.I02NgForTrackBy> {
  final import15.TextBinding _textBinding_1 = import15.TextBinding();
  _ViewI02NgForTrackBy1(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('li'));
    _el_0.append(this._textBinding_1.element);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_item = import7.unsafeCast<String>(this.locals['\$implicit']);
    this._textBinding_1.updateText(import18.interpolateString0(local_item)) /* REF:package:corpus_ngdart/src/i02_ng_for_track_by.html:54:62 */;
  }
}

import14.EmbeddedView<void> viewFactory_I02NgForTrackBy1(import16.RenderView parentView, int parentIndex) {
  return _ViewI02NgForTrackBy1(parentView, parentIndex);
}

final List<Object> styles$I02NgForTrackByHost = const [];

class _ViewI02NgForTrackByHost0 extends import19.HostView<import1.I02NgForTrackBy> {
  @override
  void build() {
    this.componentView = ViewI02NgForTrackBy0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I02NgForTrackBy();
    this.initRootNode(_el_0);
  }
}

import19.HostView<import1.I02NgForTrackBy> viewFactory_I02NgForTrackByHost0() {
  return _ViewI02NgForTrackByHost0();
}
