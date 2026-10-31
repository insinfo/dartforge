"""A package with git dependencies on a local repository (absolute path URL).

usage: make_git.py <dir>
"""
import os, shutil, subprocess, sys
root = sys.argv[1]
shutil.rmtree(root, ignore_errors=True)
repo = os.path.join(root, 'repos', 'tools_repo.git')
os.makedirs(repo)
env = dict(os.environ, GIT_AUTHOR_NAME='x', GIT_AUTHOR_EMAIL='x@x', GIT_COMMITTER_NAME='x', GIT_COMMITTER_EMAIL='x@x')


def git(*args, cwd=repo):
    subprocess.run(['git', '-c', 'init.defaultBranch=main', '-c', 'commit.gpgsign=false', *args],
                   cwd=cwd, env=env, check=True, capture_output=True)


def write(rel, text, base=repo):
    p = os.path.join(base, rel)
    os.makedirs(os.path.dirname(p), exist_ok=True)
    open(p, 'w').write(text)


git('init', '-q')
write('pubspec.yaml', "name: tool_root\nversion: 1.0.0\nenvironment:\n  sdk: ^3.0.0\ndependencies:\n  meta: ^1.15.0\n")
write('lib/tool_root.dart', 'int x = 1;\n')
write('packages/helper/pubspec.yaml', "name: helper\nversion: 0.3.0\nenvironment:\n  sdk: ^3.0.0\n")
git('add', '-A')
git('commit', '-q', '-m', 'one')
git('tag', 'v1')
write('pubspec.yaml', "name: tool_root\nversion: 1.1.0\nenvironment:\n  sdk: ^3.0.0\ndependencies:\n  meta: ^1.15.0\n")
git('commit', '-q', '-am', 'two')
app = os.path.join(root, 'app')
write('pubspec.yaml', f"""name: app
environment:
  sdk: ^3.6.0
dependencies:
  tool_root:
    git: {repo}
  helper:
    git:
      url: {repo}
      path: packages/helper
      ref: v1
""", base=app)
print(root)
