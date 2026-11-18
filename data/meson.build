conf = configuration_data()
conf.set('APP_ID', app_id)
conf.set('BINDIR', bindir)
conf.set('DAEMON_BUS_NAME', daemon_bus_name)

# The daemon starts with the graphical session (systemd user unit), and a
# capture source or the app can also start it through D-Bus activation.
configure_file(
  input: 'copyveil-daemon.service.in',
  output: 'copyveil-daemon.service',
  configuration: conf,
  install: true,
  install_dir: prefix / 'lib' / 'systemd' / 'user',
)

configure_file(
  input: 'dbus.service.in',
  output: daemon_bus_name + '.service',
  configuration: conf,
  install: true,
  install_dir: datadir / 'dbus-1' / 'services',
)

desktop_file = configure_file(
  input: 'io.github.joaoferrete.Copyveil.desktop.in',
  output: app_id + '.desktop',
  configuration: conf,
  install: true,
  install_dir: datadir / 'applications',
)

metainfo_file = configure_file(
  input: 'io.github.joaoferrete.Copyveil.metainfo.xml.in',
  output: app_id + '.metainfo.xml',
  configuration: conf,
  install: true,
  install_dir: datadir / 'metainfo',
)

install_data(
  'icons/hicolor/scalable/apps/io.github.joaoferrete.Copyveil.svg',
  install_dir: datadir / 'icons' / 'hicolor' / 'scalable' / 'apps',
)
install_data(
  'icons/hicolor/symbolic/apps/io.github.joaoferrete.Copyveil-symbolic.svg',
  install_dir: datadir / 'icons' / 'hicolor' / 'symbolic' / 'apps',
)

desktop_file_validate = find_program('desktop-file-validate', required: false)
if desktop_file_validate.found()
  test('validate-desktop', desktop_file_validate, args: [desktop_file])
endif

appstreamcli = find_program('appstreamcli', required: false)
if appstreamcli.found()
  test('validate-metainfo', appstreamcli, args: ['validate', '--no-net', '--explain', metainfo_file])
endif
