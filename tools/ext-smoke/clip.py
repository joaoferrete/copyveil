# SPDX-License-Identifier: GPL-3.0-or-later
"""A Wayland client for the extension smoke test: copies or reads the clipboard.

    clip.py copy TEXT      offer TEXT as text/plain
    clip.py secret TEXT    offer TEXT with the password-manager hint
    clip.py read           print the clipboard as CLIP:[...]
    clip.py rich TEXT      offer <b>TEXT</b> as text/html, with TEXT as text/plain
    clip.py rich-hold TEXT the same, staying the clipboard's owner for 8 s, as
                           the app does (Mutter keeps only plain text once an
                           owner exits)
    clip.py formats        print the offered MIME types as CLIP-FORMATS:[...]

Only ever run with synthetic markers, inside tools/ext-smoke/run.sh.
"""
import sys, gi
gi.require_version('Gtk', '4.0'); gi.require_version('Gdk', '4.0')
from gi.repository import Gtk, Gdk, GLib
mode = sys.argv[1]
app = Gtk.Application()
def activate(app):
    win = Gtk.ApplicationWindow(application=app)
    win.present()
    def go():
        cb = win.get_clipboard()
        if mode == 'formats':
            print('CLIP-FORMATS:[' + ' '.join(sorted(cb.get_formats().get_mime_types() or [])) + ']')
            app.quit()
        elif mode == 'read':
            def done(c, r):
                try:
                    print('CLIP:[' + (c.read_text_finish(r) or '') + ']')
                except GLib.Error as e:
                    print('CLIP-ERROR:' + e.message)
                app.quit()
            cb.read_text_async(None, done)
        else:
            providers = [Gdk.ContentProvider.new_for_bytes('text/plain;charset=utf-8', GLib.Bytes.new(sys.argv[2].encode()))]
            if mode in ('rich', 'rich-hold'):
                html = ('<b>' + sys.argv[2] + '</b>').encode()
                providers.insert(0, Gdk.ContentProvider.new_for_bytes('text/html', GLib.Bytes.new(html)))
            if mode == 'secret':
                providers.append(Gdk.ContentProvider.new_for_bytes('x-kde-passwordManagerHint', GLib.Bytes.new(b'secret')))
            cb.set_content(Gdk.ContentProvider.new_union(providers))
            GLib.timeout_add(8000 if mode == 'rich-hold' else 2500, lambda: app.quit())
        return False
    GLib.timeout_add(700, go)
app.connect('activate', activate)
app.run([])
