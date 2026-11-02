"""Send synthetic mouse/keyboard input to the X display in $DISPLAY (for GUI screenshots).

Actions: click:X,Y   key:KEYSYM (e.g. key:F9, key:Return)   sleep:SECONDS
Requires `pip install python-xlib`. Used by scripts/gui-screenshot.sh.
Note: without a window manager, keyboard focus may not reach the window; prefer clicks.
"""
import sys
import time

from Xlib import X, XK, display
from Xlib.ext import xtest

d = display.Display()


def key(name):
    code = d.keysym_to_keycode(XK.string_to_keysym(name))
    xtest.fake_input(d, X.KeyPress, code)
    d.sync()
    time.sleep(0.05)
    xtest.fake_input(d, X.KeyRelease, code)
    d.sync()


def click(x, y):
    xtest.fake_input(d, X.MotionNotify, x=x, y=y)
    d.sync()
    time.sleep(0.1)
    xtest.fake_input(d, X.ButtonPress, 1)
    d.sync()
    time.sleep(0.05)
    xtest.fake_input(d, X.ButtonRelease, 1)
    d.sync()


for action in sys.argv[1:]:
    if action.startswith("key:"):
        key(action[4:])
    elif action.startswith("click:"):
        x, y = map(int, action[6:].split(","))
        click(x, y)
    elif action.startswith("sleep:"):
        time.sleep(float(action[6:]))
    else:
        sys.exit(f"unknown action: {action}")
    time.sleep(0.2)
