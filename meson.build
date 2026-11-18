project(
  'copyveil',
  version: '0.0.0', # x-release-please-version
  meson_version: '>= 1.1',
  license: 'GPL-3.0-or-later',
)

app_id = 'io.github.joaoferrete.Copyveil'
daemon_bus_name = app_id + '.Daemon'

prefix = get_option('prefix')
bindir = prefix / get_option('bindir')
datadir = prefix / get_option('datadir')

subdir('data')

cargo = find_program('cargo')
cargo_build = find_program('build-aux' / 'cargo-build.sh')

custom_target(
  'cargo-build',
  build_by_default: true,
  build_always_stale: true,
  console: true,
  output: ['copyveil', 'copyveil-daemon'],
  install: true,
  install_dir: bindir,
  command: [
    cargo_build,
    cargo,
    meson.project_source_root(),
    meson.project_build_root() / 'target',
    get_option('profile'),
    '@OUTDIR@',
  ],
)
