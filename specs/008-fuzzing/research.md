# Research: Fuzzing The Untrusted Boundaries

## D1: the parser first, the C boundary second

`nsi-parse` takes `&[u8]`, needs no renderer, and has a sink that can
do nothing, so a target is a dozen lines and every finding reproduces
anywhere. `nsi-ffi-wrap`'s boundary would need synthesised `NSIParam`
arrays -- a target that must be as correct as the code it tests -- and
its failure mode is memory corruption rather than a panic, which wants
the sanitizer build as well. It is the next target, not this one.

## D2: bytes in unchanged

The target does not build a `String`, a `&str` or any structure from
its input. Doing so would limit the fuzzer to valid UTF-8, and ɴsɪ
string *values* are bytes -- a Latin-1 file name is legal -- so the
constraint would hide exactly the paths `contracts/recording.md` cares
about.

## D3: build in CI, run by hand

A fuzzer run has no end state. Bounding it in CI either makes it too
short to find anything or long enough to be the slowest job, and a
finding there reads as a flaky failure. Building it in CI is what keeps
it honest: a target that no longer compiles is a target nobody runs.

## D4: `cargo-fuzz init` guessed the wrong crate

Run inside `crates/nsi-parse`, it wrote a manifest depending on
`nsi-3delight` -- apparently the first workspace member it found -- and
named the package after it. The manifest was written by hand instead.

## D5: the host target, explicitly

On this machine `cargo fuzz build` defaulted to
`x86_64-unknown-linux-musl`, whose standard library is not installed,
and failed in `libc`. Passing `--target x86_64-unknown-linux-gnu` builds
it. CI's runner has no such default, so the workflow does not pass one.

## First Run

Thirty-one seconds, 1 988 922 executions, about 64 000 per second, from
the ten seeds: coverage 1099 edges, corpus grown to 1099 inputs, 452 MB
resident at the end, **no crash, no timeout, no leak report**. A first
attempt at four minutes was killed by the OOM killer while another
session was building; the memory limit is set explicitly now
(`-rss_limit_mb=1024`).
