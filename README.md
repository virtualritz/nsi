# ɴsɪ

High level Rust bindings for Illumination Research’s Nodal Scene Interface – ɴsɪ.

## Dependencies

This crate depends on [nsi-sys](https://github.com/virtualritz/nsi-sys) which in term requires a renderer that implements the ɴsɪ API. Currently the only renderer that does is [3Delight](https://www.3delight.com/).

Before you start, [download a 3Delight package](https://www.3delight.com/download) for your platform & install it.
This will set the `$DELIGHT` environment variable that the build script is looking for to find headers and the library to link against.

> **_Note:_** The free version of 3Delight will render with up to 12 cores on your machine. For crazier projects you can use their cheap cloud rendering service that gives you access to unlimited CPU cores. When you register, you get 1,000 cloud minutes for free which ain't too shabby.

## Building

No suprises here. The crate works with stable, beta & nightly.

```
> cargo build
```

## Documentation

Docs for the C, C++, Lua & Python bindings as well as an introduction and deep dive into the API [can be found here](https://nsi.readthedocs.io).

Crate documentation is coming soon.
