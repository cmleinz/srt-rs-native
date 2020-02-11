# srt-rs

[![codecov][codecov badge]][codecov]

| OS      | Status                                          |
| ------- | ----------------------------------------------- |
| Linux   | [![Linux Build Status][linux badge]][build]     |
| macOS   | [![macOS Build Status][macos badge]][build]     |
| Windows | [![Windows Build Status][windows badge]][build] |


> NOTE: THIS IS NOT PRODUCTION READY.

Pure rust implementation of SRT (Secure Reliable Transport), without unsafe code.

Reference implementation is available at https://github.com/haivision/srt

# Features

- Fast (heap allocations are rare, uses async IO)
- Full safety garuntees of rust

# What works

- [x] Listen server connecting
- [x] Client (connect) connecting
- [x] Rendezvous connecting
- [x] Receiving
- [x] Sending
- [x] Special SRT packets (partial)
- [x] Actual SRT (TSBPD)
- [ ] Timestamp drift recovery
- [ ] Encryption
- [x] Bidirectional

# Thread Efficiency

The reference implementation of SRT requires 3 threads per sender and 5 threads per receiver. 

With srt-rs, you can assign as many connections to exactly as many threads as you want (usually as many as you have cores) using
[tokio's][tokio] futures scheduling. This should allow for handing of many more connectinos.

[codecov]: https://codecov.io/gh/russelltg/srt-rs
[codecov badge]: https://codecov.io/gh/russelltg/srt-rs/branch/master/graph/badge.svg

[build]: https://russelltg.visualstudio.com/srt-rs/_build/latest?definitionId=2&branchName=master
[linux badge]: https://russelltg.visualstudio.com/srt-rs/_apis/build/status/russelltg.srt-rs?branchName=master&stageName=Multi%20OS%20native%20tests&jobName=Cargo%20test&configuration=Cargo%20test%20Linux
[macos badge]: https://russelltg.visualstudio.com/srt-rs/_apis/build/status/russelltg.srt-rs?branchName=master&stageName=Multi%20OS%20native%20tests&jobName=Cargo%20test&configuration=Cargo%20test%20MacOS
[windows badge]: https://russelltg.visualstudio.com/srt-rs/_apis/build/status/russelltg.srt-rs?branchName=master&stageName=Multi%20OS%20native%20tests&jobName=Cargo%20test&configuration=Cargo%20test%20Windows

[tokio]: https://tokio.rs