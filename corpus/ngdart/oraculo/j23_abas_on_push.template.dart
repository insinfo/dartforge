// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j23_abas_on_push.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j23_abas_on_push.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import8;

final List<Object> styles$J23AbasOnPush = const [];

class ViewJ23AbasOnPush0 extends import0.ComponentView<import1.J23AbasOnPush> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ23AbasOnPush0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j23-abas-on-push'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j23_abas_on_push.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this.project(parentRenderNode, 0);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J23AbasOnPush, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J23AbasOnPushNgFactory = ComponentFactory<import1.J23AbasOnPush>('j23-abas-on-push', viewFactory_J23AbasOnPushHost0);
ComponentFactory<import1.J23AbasOnPush> get J23AbasOnPushNgFactory {
  return _J23AbasOnPushNgFactory;
}

ComponentFactory<import1.J23AbasOnPush> createJ23AbasOnPushFactory() {
  return ComponentFactory('j23-abas-on-push', viewFactory_J23AbasOnPushHost0);
}

final List<Object> styles$J23AbasOnPushHost = const [];

class _ViewJ23AbasOnPushHost0 extends import8.HostView<import1.J23AbasOnPush> {
  @override
  void build() {
    this.componentView = ViewJ23AbasOnPush0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J23AbasOnPush();
    this.component.abas = [];
    this.initRootNode(_el_0);
  }
}

import8.HostView<import1.J23AbasOnPush> viewFactory_J23AbasOnPushHost0() {
  return _ViewJ23AbasOnPushHost0();
}
