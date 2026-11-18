# SPDX-License-Identifier: GPL-3.0-or-later
"""A Wayland client for the extension smoke test: copies or reads the clipboard.

    clip.py copy TEXT      offer TEXT as text/plain
    clip.py secret TEXT    offer TEXT with the password-manager hint
    clip.py read           print the clipboard as CLIP:[...]

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
        if mode == 'read':
            def done(c, r):
                try:
                    print('CLIP:[' + (c.read_text_finish(r) or '') + ']')
                except GLib.Error as e:
                    print('CLIP-ERROR:' + e.message)
                app.quit()
            cb.read_text_async(None, done)
        else:
            providers = [Gdk.ContentProvider.new_for_bytes('text/plain;charset=utf-8', GLib.Bytes.new(sys.argv[2].encode()))]
            if mode == 'secret':
                providers.append(Gdk.ContentProvider.new_for_bytes('x-kde-passwordManagerHint', GLib.Bytes.new(b'secret')))
            cb.set_content(Gdk.ContentProvider.new_union(providers))
            GLib.timeout_add(2500, lambda: app.quit())
        return False
    GLib.timeout_add(700, go)
app.connect('activate', activate)
app.run([])
