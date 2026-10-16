#!/usr/bin/env python3
"""Developer-only regeneration of four pinned SVGs; never a build/runtime step."""

import ctypes as c
import ctypes.util
import hashlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent / "icons"
SOURCES = {
    "archive": "ce0af5c71277130bdad501ed06249213e9805b0dd445e3da97af3bc6d32909ce",
    "trash-2": "958240c9bacb4dca8f2ea66e08a817eab4ace45c9e72086472095edaa7cc3a9f",
    "check": "f5914af9d3a5dc1e77ebf278af6488dc17b1935d96f50f1f8dae78e625b1c5b1",
    "x": "046d06a319a47b847e1053e557f1cddf3fad2f53abad2150e563cec9ad404d92",
}


def library(name):
    path = ctypes.util.find_library(name)
    if not path:
        raise RuntimeError(f"Developer renderer requires {name}; do not install on recipients")
    return c.CDLL(path)


def function(lib, name, result, *args):
    fn = getattr(lib, name)
    fn.restype, fn.argtypes = result, args
    return fn


def main():
    rsvg, pixbuf, gobject = (library(n) for n in ("rsvg-2", "gdk_pixbuf-2.0", "gobject-2.0"))
    load = function(rsvg, "rsvg_handle_new_from_data", c.c_void_p, c.c_char_p, c.c_size_t, c.c_void_p)
    render = function(rsvg, "rsvg_handle_get_pixbuf", c.c_void_p, c.c_void_p)
    width = function(pixbuf, "gdk_pixbuf_get_width", c.c_int, c.c_void_p)
    height = function(pixbuf, "gdk_pixbuf_get_height", c.c_int, c.c_void_p)
    save = function(pixbuf, "gdk_pixbuf_savev", c.c_int, c.c_void_p, c.c_char_p, c.c_char_p,
                    c.c_void_p, c.c_void_p, c.c_void_p)
    unref = function(gobject, "g_object_unref", None, c.c_void_p)
    version = [c.c_uint.in_dll(rsvg, f"rsvg_{part}_version").value for part in ("major", "minor", "micro")]
    print("librsvg", ".".join(map(str, version)))
    for name, expected in SOURCES.items():
        source = (ROOT / "source" / f"{name}.svg").read_bytes()
        if hashlib.sha256(source).hexdigest() != expected:
            raise RuntimeError(f"Unreviewed source: {name}")
        # Keep reviewed geometry unchanged; rasterize with the design's text color.
        source = source.replace(b'stroke="#FFFFFF"', b'stroke="#E3F0EE"')
        handle = load(source, len(source), None)
        if not handle:
            raise RuntimeError(f"Cannot load {name}")
        try:
            image = render(handle)
            if not image:
                raise RuntimeError(f"Cannot render {name}")
            try:
                if (width(image), height(image)) != (24, 24):
                    raise RuntimeError(f"Unexpected size: {name}")
                destination = ROOT / f"{name}.png"
                if not save(image, str(destination).encode(), b"png", None, None, None):
                    raise RuntimeError(f"Cannot save {name}")
                print(hashlib.sha256(destination.read_bytes()).hexdigest(), f"icons/{name}.png")
            finally:
                unref(image)
        finally:
            unref(handle)


if __name__ == "__main__":
    main()
