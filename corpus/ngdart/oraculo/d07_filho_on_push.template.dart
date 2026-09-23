// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'd07_filho_on_push.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'd07_filho_on_push.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'dart:html' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/src/runtime/interpolate.dart' as import9;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import11;

final List<Object> styles$D07FilhoOnPush = const [];

class ViewD07FilhoOnPush0 extends import0.ComponentView<import1.D07FilhoOnPush> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  static import3.ComponentStyles? _componentStyles;
  ViewD07FilhoOnPush0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkOnce) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('d07-filho-on-push'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/d07_filho_on_push.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendSpan(doc, parentRenderNode);
    _el_0.append(this._textBinding_1.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateText(import9.interpolateString0(_ctx.rotulo)) /* REF:package:corpus_ngdart/src/d07_filho_on_push.html:6:16 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$D07FilhoOnPush, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _D07FilhoOnPushNgFactory = ComponentFactory<import1.D07FilhoOnPush>('d07-filho-on-push', viewFactory_D07FilhoOnPushHost0);
ComponentFactory<import1.D07FilhoOnPush> get D07FilhoOnPushNgFactory {
  return _D07FilhoOnPushNgFactory;
}

ComponentFactory<import1.D07FilhoOnPush> createD07FilhoOnPushFactory() {
  return ComponentFactory('d07-filho-on-push', viewFactory_D07FilhoOnPushHost0);
}

final List<Object> styles$D07FilhoOnPushHost = const [];

class _ViewD07FilhoOnPushHost0 extends import11.HostView<import1.D07FilhoOnPush> {
  @override
  void build() {
    this.componentView = ViewD07FilhoOnPush0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.D07FilhoOnPush();
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    bool changed = false;
    if (changed) {
      this.componentView.markAsCheckOnce();
    }
    this.componentView.detectChanges();
  }
}

import11.HostView<import1.D07FilhoOnPush> viewFactory_D07FilhoOnPushHost0() {
  return _ViewD07FilhoOnPushHost0();
}
