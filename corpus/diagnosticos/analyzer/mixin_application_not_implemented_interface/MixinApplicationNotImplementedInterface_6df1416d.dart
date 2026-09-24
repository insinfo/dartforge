class ioDirectory implements ioFileSystemEntity {}

class ioFileSystemEntity {}

abstract class _LocalDirectory
//             ^^^^^^^^^^^^^^^
// [diag.conflictingGenericInterfaces] The class '_LocalDirectory' can't implement both 'ForwardingFileSystemEntity<_LocalDirectory, ioDirectory>' and 'ForwardingFileSystemEntity<Directory, ioDirectory>' because the type arguments are different.
// [diag.unusedElement] The declaration '_LocalDirectory' isn't referenced.
    extends _LocalFileSystemEntity<_LocalDirectory, ioDirectory>
    with ForwardingDirectory, DirectoryAddOnsMixin {}

abstract class _LocalFileSystemEntity<T extends FileSystemEntity,
  D extends ioFileSystemEntity> extends ForwardingFileSystemEntity<T, D> {}

abstract class FileSystemEntity implements ioFileSystemEntity {}

abstract class ForwardingFileSystemEntity<T extends FileSystemEntity,
  D extends ioFileSystemEntity> implements FileSystemEntity {}


mixin ForwardingDirectory<T extends Directory>
    on ForwardingFileSystemEntity<T, ioDirectory>
    implements Directory {}

abstract class Directory implements FileSystemEntity, ioDirectory {}

mixin DirectoryAddOnsMixin implements Directory {}
