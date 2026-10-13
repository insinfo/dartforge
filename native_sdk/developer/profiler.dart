// Copyright (c) 2014, the Dart project authors.  Please see the AUTHORS file
// for details. All rights reserved. Use of this source code is governed by a
// BSD-style license that can be found in the LICENSE file.

// DartForge: the VM's `profiler.dart` with the tags (`UserTag`) in Dart,
// without the profiler's natives (`UserTag_*`, `Profiler_getCurrentTag`). The
// observable semantics are the VM's: the same tag for the same label, at most
// `UserTag.maxUserTags` tags, `makeCurrent` returns the previous one and the
// current tag is per isolate (the statics are per isolate).

part of "developer.dart";

@patch
class UserTag {
  @patch
  factory UserTag(String label) {
    return new _UserTag(label);
  }
  @patch
  static UserTag get defaultTag => _getDefaultTag();
}

@pragma("vm:entry-point")
final class _UserTag implements UserTag {
  static final Map<String, _UserTag> _tags = <String, _UserTag>{};
  static final _UserTag _defaultTag = new _UserTag._('Default');
  static _UserTag? _currentTag;

  final String label;

  _UserTag._(this.label);

  factory _UserTag(String label) {
    final existing = _tags[label];
    if (existing != null) return existing;
    if (_tags.length >= UserTag.maxUserTags) {
      throw new UnsupportedError(
          'UserTag instance limit (${UserTag.maxUserTags}) reached.');
    }
    return _tags[label] = new _UserTag._(label);
  }

  UserTag makeCurrent() {
    final previous = _currentTag ?? _defaultTag;
    _currentTag = this;
    return previous;
  }
}

@patch
UserTag getCurrentTag() => _getCurrentTag();

UserTag _getCurrentTag() => _UserTag._currentTag ?? _UserTag._defaultTag;

UserTag _getDefaultTag() => _UserTag._defaultTag;
