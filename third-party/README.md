# Third-party notices

GlassTodo's own code is MIT-licensed. Dependencies retain their upstream licenses. This inventory is generated from Cargo.lock and package license files; it includes resolved platform/build dependencies, including some not linked for every target. It is an inventory, not a new license grant.

Regenerate with `python scripts/collect_licenses.py` after changing dependencies/toolchains. CI regenerates the inventory for its package.

Compiler: `rustc 1.99.1 (58cf8f9c3 2026-10-13)`. This minimal toolchain does not include local Rust notice files; see the [upstream Rust license texts](https://github.com/rust-lang/rust#license).

SQLite is bundled by libsqlite3-sys; its upstream [public-domain statement](https://sqlite.org/copyright.html) applies in addition to the wrapper crate's notice. Windows APIs, Rich Edit and system fonts are used from the OS, not redistributed here.

| Package | Locked version | Declared SPDX expression | Included upstream text |
| --- | --- | --- | --- |
| [android_system_properties](https://crates.io/crates/android_system_properties/0.1.6) | 0.1.6 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/216486f29671a4262efe32af6d84a75b.txt), [LICENSE-MIT](licenses/80f275e90d799911ed3830a7f242a2ef.txt) |
| [anyhow](https://crates.io/crates/anyhow/1.0.104) | 1.0.104 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/62c7a1e35f56406896d7aa7ca52d0cc0.txt), [LICENSE-MIT](licenses/23f18e03dc49df91622fe2a761764974.txt) |
| [async-trait](https://crates.io/crates/async-trait/0.1.92) | 0.1.92 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/62c7a1e35f56406896d7aa7ca52d0cc0.txt), [LICENSE-MIT](licenses/23f18e03dc49df91622fe2a761764974.txt) |
| [atomic-waker](https://crates.io/crates/atomic-waker/1.1.2) | 1.1.2 | Apache-2.0 OR MIT | [LICENSE-APACHE](licenses/a60eea817514531668d7e00765731449.txt), [LICENSE-MIT](licenses/23f18e03dc49df91622fe2a761764974.txt), [LICENSE-THIRD-PARTY](licenses/6226d0632e2e1a80c23597e964da9812.txt) |
| [autocfg](https://crates.io/crates/autocfg/1.5.1) | 1.5.1 | Apache-2.0 OR MIT | [LICENSE-APACHE](licenses/a60eea817514531668d7e00765731449.txt), [LICENSE-MIT](licenses/27995d58ad5c1145c1a8cd86244ce844.txt) |
| [axum](https://crates.io/crates/axum/0.8.9) | 0.8.9 | MIT | [LICENSE](licenses/6a13bc24a100a6812f053879ec51b126.txt) |
| [axum-core](https://crates.io/crates/axum-core/0.5.6) | 0.5.6 | MIT | [LICENSE](licenses/008c87afcd2e626eaf564093250bed06.txt) |
| [base64](https://crates.io/crates/base64/0.23.1) | 0.23.1 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/a60eea817514531668d7e00765731449.txt), [LICENSE-MIT](licenses/ab499c75a0f0da8e0fe83bf9ba3a27e5.txt) |
| [bitflags](https://crates.io/crates/bitflags/2.13.2) | 2.13.2 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/a60eea817514531668d7e00765731449.txt), [LICENSE-MIT](licenses/6485b8ed310d3f0340bf1ad1f4764506.txt) |
| [bumpalo](https://crates.io/crates/bumpalo/3.20.3) | 3.20.3 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/a60eea817514531668d7e00765731449.txt), [LICENSE-MIT](licenses/65f94e99ddaf4f5d1782a6dae23f35d4.txt) |
| [bytes](https://crates.io/crates/bytes/1.12.1) | 1.12.1 | MIT | [LICENSE](licenses/45f522cacecb1023856e46df79ca625d.txt) |
| [cc](https://crates.io/crates/cc/1.10.0) | 1.10.0 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/a60eea817514531668d7e00765731449.txt), [LICENSE-MIT](licenses/378f5840b258e2779c39418f3f2d7b2b.txt) |
| [cfg-if](https://crates.io/crates/cfg-if/1.0.5) | 1.0.5 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/a60eea817514531668d7e00765731449.txt), [LICENSE-MIT](licenses/378f5840b258e2779c39418f3f2d7b2b.txt) |
| [chacha20](https://crates.io/crates/chacha20/0.10.2) | 0.10.2 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/a9040321c3712d8fd0b09cf52b17445d.txt), [LICENSE-MIT](licenses/b8c6939380a400f53e11923d50fcc4dd.txt) |
| [chrono](https://crates.io/crates/chrono/0.4.45) | 0.4.45 | MIT OR Apache-2.0 | [LICENSE.txt](licenses/946c9835d8034d24404f8cfec5f4654c.txt) |
| [core-foundation-sys](https://crates.io/crates/core-foundation-sys/0.8.7) | 0.8.7 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/a60eea817514531668d7e00765731449.txt), [LICENSE-MIT](licenses/62065228e42caebca7e7d7db1204cbb8.txt) |
| [cpufeatures](https://crates.io/crates/cpufeatures/0.3.1) | 0.3.1 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/a9040321c3712d8fd0b09cf52b17445d.txt), [LICENSE-MIT](licenses/73b9dc2e79c7308998dd30296e073aef.txt) |
| [darling](https://crates.io/crates/darling/0.24.1) | 0.24.1 | MIT | [LICENSE](licenses/8ea93490d74a5a1b1af3ff71d786271b.txt) |
| [darling_core](https://crates.io/crates/darling_core/0.24.1) | 0.24.1 | MIT | [LICENSE](licenses/8ea93490d74a5a1b1af3ff71d786271b.txt) |
| [darling_macro](https://crates.io/crates/darling_macro/0.24.1) | 0.24.1 | MIT | [LICENSE](licenses/8ea93490d74a5a1b1af3ff71d786271b.txt) |
| [dyn-clone](https://crates.io/crates/dyn-clone/1.0.20) | 1.0.20 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/62c7a1e35f56406896d7aa7ca52d0cc0.txt), [LICENSE-MIT](licenses/23f18e03dc49df91622fe2a761764974.txt) |
| [equivalent](https://crates.io/crates/equivalent/1.0.3) | 1.0.3 | Apache-2.0 OR MIT | [LICENSE-APACHE](licenses/a60eea817514531668d7e00765731449.txt), [LICENSE-MIT](licenses/7365cc8878a1d7ce155a58c4ca09c3d7.txt) |
| [fallible-iterator](https://crates.io/crates/fallible-iterator/0.3.0) | 0.3.0 | MIT/Apache-2.0 | [LICENSE-APACHE](licenses/c6596eb7be8581c18be736c846fb9173.txt), [LICENSE-MIT](licenses/0816e154b159ba255c563f7c8c7df5bb.txt) |
| [fallible-streaming-iterator](https://crates.io/crates/fallible-streaming-iterator/0.1.9) | 0.1.9 | MIT/Apache-2.0 | [LICENSE-APACHE](licenses/c6596eb7be8581c18be736c846fb9173.txt), [LICENSE-MIT](licenses/8dcec5569a9be5b0e086c80faed6f1ae.txt) |
| [find-msvc-tools](https://crates.io/crates/find-msvc-tools/0.1.15) | 0.1.15 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/a60eea817514531668d7e00765731449.txt), [LICENSE-MIT](licenses/378f5840b258e2779c39418f3f2d7b2b.txt) |
| [futures](https://crates.io/crates/futures/0.3.34) | 0.3.34 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/275c491d6d1160553c32fd6127061d7f.txt), [LICENSE-MIT](licenses/6652c868f35dfe5e8ef636810a4e576b.txt) |
| [futures-channel](https://crates.io/crates/futures-channel/0.3.34) | 0.3.34 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/275c491d6d1160553c32fd6127061d7f.txt), [LICENSE-MIT](licenses/6652c868f35dfe5e8ef636810a4e576b.txt) |
| [futures-core](https://crates.io/crates/futures-core/0.3.34) | 0.3.34 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/275c491d6d1160553c32fd6127061d7f.txt), [LICENSE-MIT](licenses/6652c868f35dfe5e8ef636810a4e576b.txt) |
| [futures-executor](https://crates.io/crates/futures-executor/0.3.34) | 0.3.34 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/275c491d6d1160553c32fd6127061d7f.txt), [LICENSE-MIT](licenses/6652c868f35dfe5e8ef636810a4e576b.txt) |
| [futures-io](https://crates.io/crates/futures-io/0.3.34) | 0.3.34 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/275c491d6d1160553c32fd6127061d7f.txt), [LICENSE-MIT](licenses/6652c868f35dfe5e8ef636810a4e576b.txt) |
| [futures-macro](https://crates.io/crates/futures-macro/0.3.34) | 0.3.34 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/275c491d6d1160553c32fd6127061d7f.txt), [LICENSE-MIT](licenses/6652c868f35dfe5e8ef636810a4e576b.txt) |
| [futures-sink](https://crates.io/crates/futures-sink/0.3.34) | 0.3.34 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/275c491d6d1160553c32fd6127061d7f.txt), [LICENSE-MIT](licenses/6652c868f35dfe5e8ef636810a4e576b.txt) |
| [futures-task](https://crates.io/crates/futures-task/0.3.34) | 0.3.34 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/275c491d6d1160553c32fd6127061d7f.txt), [LICENSE-MIT](licenses/6652c868f35dfe5e8ef636810a4e576b.txt) |
| [futures-util](https://crates.io/crates/futures-util/0.3.34) | 0.3.34 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/275c491d6d1160553c32fd6127061d7f.txt), [LICENSE-MIT](licenses/6652c868f35dfe5e8ef636810a4e576b.txt) |
| [getrandom](https://crates.io/crates/getrandom/0.3.4) | 0.3.4 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/aaff376532ea30a0cd5330b9502ad4a4.txt), [LICENSE-MIT](licenses/29e9fe5074bd27e0e5d5d110394fbbcd.txt) |
| [getrandom](https://crates.io/crates/getrandom/0.4.3) | 0.4.3 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/aaff376532ea30a0cd5330b9502ad4a4.txt), [LICENSE-MIT](licenses/523a42c25d245dde9c015f882cec7f45.txt) |
| [hashbrown](https://crates.io/crates/hashbrown/0.17.1) | 0.17.1 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/a60eea817514531668d7e00765731449.txt), [LICENSE-MIT](licenses/ff8f68cb076caf8cefe7a6430d4ac086.txt) |
| [http](https://crates.io/crates/http/1.5.0) | 1.5.0 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/8bb1b50b0e5c9399ae33bd35fab27690.txt), [LICENSE-MIT](licenses/dc91f8200e4b2a1f9261035d4c18c33c.txt) |
| [http-body](https://crates.io/crates/http-body/1.1.0) | 1.1.0 | MIT | [LICENSE](licenses/248378d0a3383c173fb925f17141b88e.txt) |
| [http-body-util](https://crates.io/crates/http-body-util/0.1.5) | 0.1.5 | MIT | [LICENSE](licenses/248378d0a3383c173fb925f17141b88e.txt) |
| [httparse](https://crates.io/crates/httparse/1.10.1) | 1.10.1 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/a60eea817514531668d7e00765731449.txt), [LICENSE-MIT](licenses/391a5396cec6230bfabd4ef4eb2350eb.txt) |
| [httpdate](https://crates.io/crates/httpdate/1.0.3) | 1.0.3 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/4d10fe5f3aa176b05b229a248866bad7.txt), [LICENSE-MIT](licenses/934887691e05d69d7c86ad3f2c360980.txt) |
| [hyper](https://crates.io/crates/hyper/1.12.0) | 1.12.0 | MIT | [LICENSE](licenses/2d01890414494742ba4a509fcec8efa4.txt) |
| [hyper-util](https://crates.io/crates/hyper-util/0.1.21) | 0.1.21 | MIT | [LICENSE](licenses/9e0a97848ea543aef745c98e84fde696.txt) |
| [iana-time-zone](https://crates.io/crates/iana-time-zone/0.1.65) | 0.1.65 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/696759d65dfe558ff7d9f031c76db19e.txt), [LICENSE-MIT](licenses/da28ccc6b158fc2d8cccc74e99794b1c.txt) |
| [iana-time-zone-haiku](https://crates.io/crates/iana-time-zone-haiku/0.1.2) | 0.1.2 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/696759d65dfe558ff7d9f031c76db19e.txt), [LICENSE-MIT](licenses/da28ccc6b158fc2d8cccc74e99794b1c.txt) |
| [ident_case](https://crates.io/crates/ident_case/1.0.1) | 1.0.1 | MIT/Apache-2.0 | [LICENSE](licenses/508a77d2e7b51d98adeed32648ad124b.txt) |
| [indexmap](https://crates.io/crates/indexmap/2.14.2) | 2.14.2 | Apache-2.0 OR MIT | [LICENSE-APACHE](licenses/a60eea817514531668d7e00765731449.txt), [LICENSE-MIT](licenses/ecc269ef87fd38a1d98e30bfac9ba964.txt) |
| [itoa](https://crates.io/crates/itoa/1.0.18) | 1.0.18 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/62c7a1e35f56406896d7aa7ca52d0cc0.txt), [LICENSE-MIT](licenses/23f18e03dc49df91622fe2a761764974.txt) |
| [js-sys](https://crates.io/crates/js-sys/0.3.106) | 0.3.106 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/a60eea817514531668d7e00765731449.txt), [LICENSE-MIT](licenses/378f5840b258e2779c39418f3f2d7b2b.txt) |
| [libc](https://crates.io/crates/libc/0.2.191) | 0.2.191 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/62c7a1e35f56406896d7aa7ca52d0cc0.txt), [LICENSE-MIT](licenses/123a331b5dbf04c30097fa43b8f858bc.txt) |
| [libsqlite3-sys](https://crates.io/crates/libsqlite3-sys/0.39.1) | 0.39.1 | MIT | [LICENSE](licenses/c10c1f27337546471e5f7e4e97fdd398.txt) |
| [log](https://crates.io/crates/log/0.4.34) | 0.4.34 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/a60eea817514531668d7e00765731449.txt), [LICENSE-MIT](licenses/6485b8ed310d3f0340bf1ad1f4764506.txt) |
| [matchit](https://crates.io/crates/matchit/0.8.4) | 0.8.4 | MIT AND BSD-3-Clause | [LICENSE](licenses/de701d0618d694feb1af90f02181a176.txt), [LICENSE.httprouter](licenses/162ce11ad71338d0a0c6ebaf5c48af72.txt) |
| [memchr](https://crates.io/crates/memchr/2.8.3) | 2.8.3 | Unlicense OR MIT | [COPYING](licenses/01c266bced4a434da0051174d6bee16a.txt), [LICENSE-MIT](licenses/0f96a83840e146e43c0ec96a22ec1f39.txt) |
| [mime](https://crates.io/crates/mime/0.3.17) | 0.3.17 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/a60eea817514531668d7e00765731449.txt), [LICENSE-MIT](licenses/df9cfd06d8a44d9a671eadd39ffd97f1.txt) |
| [mio](https://crates.io/crates/mio/1.2.4) | 1.2.4 | MIT | [LICENSE](licenses/07919255c7e04793d8ea760d6c2ce32d.txt) |
| [num-traits](https://crates.io/crates/num-traits/0.2.19) | 0.2.19 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/a60eea817514531668d7e00765731449.txt), [LICENSE-MIT](licenses/6485b8ed310d3f0340bf1ad1f4764506.txt) |
| [once_cell](https://crates.io/crates/once_cell/1.21.4) | 1.21.4 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/a60eea817514531668d7e00765731449.txt), [LICENSE-MIT](licenses/23f18e03dc49df91622fe2a761764974.txt) |
| [pastey](https://crates.io/crates/pastey/0.2.3) | 0.2.3 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/62c7a1e35f56406896d7aa7ca52d0cc0.txt), [LICENSE-MIT](licenses/23f18e03dc49df91622fe2a761764974.txt) |
| [percent-encoding](https://crates.io/crates/percent-encoding/2.3.2) | 2.3.2 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/a60eea817514531668d7e00765731449.txt), [LICENSE-MIT](licenses/b38f11f6096706e6de553dabe2a7ed14.txt) |
| [pin-project-lite](https://crates.io/crates/pin-project-lite/0.2.17) | 0.2.17 | Apache-2.0 OR MIT | [LICENSE-APACHE](licenses/0d542e0c8804e39aa7f37eb00da5a762.txt), [LICENSE-MIT](licenses/23f18e03dc49df91622fe2a761764974.txt) |
| [pkg-config](https://crates.io/crates/pkg-config/0.3.34) | 0.3.34 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/a60eea817514531668d7e00765731449.txt), [LICENSE-MIT](licenses/378f5840b258e2779c39418f3f2d7b2b.txt) |
| [proc-macro2](https://crates.io/crates/proc-macro2/1.0.107) | 1.0.107 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/62c7a1e35f56406896d7aa7ca52d0cc0.txt), [LICENSE-MIT](licenses/23f18e03dc49df91622fe2a761764974.txt) |
| [quote](https://crates.io/crates/quote/1.0.47) | 1.0.47 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/62c7a1e35f56406896d7aa7ca52d0cc0.txt), [LICENSE-MIT](licenses/23f18e03dc49df91622fe2a761764974.txt) |
| [r-efi](https://crates.io/crates/r-efi/5.3.0) | 5.3.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later | See upstream package |
| [r-efi](https://crates.io/crates/r-efi/6.0.0) | 6.0.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later | See upstream package |
| [rand](https://crates.io/crates/rand/0.10.3) | 0.10.3 | MIT OR Apache-2.0 | [COPYRIGHT](licenses/90eb64f0279b0d9432accfa6023ff803.txt), [LICENSE-APACHE](licenses/35242e7a83f69875e6edeff02291e688.txt), [LICENSE-MIT](licenses/209fbbe0ad52d9235e37badf9cadfe4d.txt) |
| [rand_core](https://crates.io/crates/rand_core/0.10.1) | 0.10.1 | MIT OR Apache-2.0 | [COPYRIGHT](licenses/92b81db30f7ab6d693e0af5661a40d27.txt), [LICENSE-APACHE](licenses/6df43f6f4b5d4587f3d8d71e45532c68.txt), [LICENSE-MIT](licenses/8b6e9feec03e7c9a5facb26855cecd31.txt) |
| [ref-cast](https://crates.io/crates/ref-cast/1.0.27) | 1.0.27 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/62c7a1e35f56406896d7aa7ca52d0cc0.txt), [LICENSE-MIT](licenses/23f18e03dc49df91622fe2a761764974.txt) |
| [ref-cast-impl](https://crates.io/crates/ref-cast-impl/1.0.27) | 1.0.27 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/62c7a1e35f56406896d7aa7ca52d0cc0.txt), [LICENSE-MIT](licenses/23f18e03dc49df91622fe2a761764974.txt) |
| [rmcp](https://crates.io/crates/rmcp/3.7.0) | 3.7.0 | Apache-2.0 | [LICENSE](licenses/0382b0057770ca05e9c350a50aa3b1c1.txt) |
| [rmcp-macros](https://crates.io/crates/rmcp-macros/3.7.0) | 3.7.0 | Apache-2.0 | [LICENSE](licenses/0382b0057770ca05e9c350a50aa3b1c1.txt) |
| [rusqlite](https://crates.io/crates/rusqlite/0.41.0) | 0.41.0 | MIT | [LICENSE](licenses/c10c1f27337546471e5f7e4e97fdd398.txt) |
| [rustversion](https://crates.io/crates/rustversion/1.0.23) | 1.0.23 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/62c7a1e35f56406896d7aa7ca52d0cc0.txt), [LICENSE-MIT](licenses/23f18e03dc49df91622fe2a761764974.txt) |
| [schemars](https://crates.io/crates/schemars/1.2.3) | 1.2.3 | MIT | [LICENSE](licenses/1954992a2b32e8a2af24a4c11b726902.txt) |
| [schemars_derive](https://crates.io/crates/schemars_derive/1.2.3) | 1.2.3 | MIT | [LICENSE](licenses/1954992a2b32e8a2af24a4c11b726902.txt) |
| [serde](https://crates.io/crates/serde/1.0.229) | 1.0.229 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/62c7a1e35f56406896d7aa7ca52d0cc0.txt), [LICENSE-MIT](licenses/23f18e03dc49df91622fe2a761764974.txt) |
| [serde_core](https://crates.io/crates/serde_core/1.0.229) | 1.0.229 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/62c7a1e35f56406896d7aa7ca52d0cc0.txt), [LICENSE-MIT](licenses/23f18e03dc49df91622fe2a761764974.txt) |
| [serde_derive](https://crates.io/crates/serde_derive/1.0.229) | 1.0.229 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/62c7a1e35f56406896d7aa7ca52d0cc0.txt), [LICENSE-MIT](licenses/23f18e03dc49df91622fe2a761764974.txt) |
| [serde_derive_internals](https://crates.io/crates/serde_derive_internals/0.30.0) | 0.30.0 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/62c7a1e35f56406896d7aa7ca52d0cc0.txt), [LICENSE-MIT](licenses/23f18e03dc49df91622fe2a761764974.txt) |
| [serde_json](https://crates.io/crates/serde_json/1.0.154) | 1.0.154 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/62c7a1e35f56406896d7aa7ca52d0cc0.txt), [LICENSE-MIT](licenses/23f18e03dc49df91622fe2a761764974.txt) |
| [shlex](https://crates.io/crates/shlex/2.0.1) | 2.0.1 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/553fffcd9b1cb158bc3e9edc35da85ca.txt), [LICENSE-MIT](licenses/4455bf75a91154108304cb283e0fea99.txt) |
| [slab](https://crates.io/crates/slab/0.4.12) | 0.4.12 | MIT | [LICENSE](licenses/8ce0830173fdac609dfb4ea603fdc002.txt) |
| [smallvec](https://crates.io/crates/smallvec/1.16.4) | 1.16.4 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/a60eea817514531668d7e00765731449.txt), [LICENSE-MIT](licenses/0b28172679e0009b655da42797c03fd1.txt) |
| [socket2](https://crates.io/crates/socket2/0.6.5) | 0.6.5 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/a60eea817514531668d7e00765731449.txt), [LICENSE-MIT](licenses/378f5840b258e2779c39418f3f2d7b2b.txt) |
| [sse-stream](https://crates.io/crates/sse-stream/0.2.6) | 0.2.6 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/cfc7749b96f63bd31c3c42b5c471bf75.txt), [LICENSE-MIT](licenses/049d23c7810a0723f186a193f841378d.txt) |
| [strsim](https://crates.io/crates/strsim/0.11.1) | 0.11.1 | MIT | [LICENSE](licenses/1e697ce8d21401fbf1bddd9b5c3fd4c4.txt) |
| [syn](https://crates.io/crates/syn/2.0.119) | 2.0.119 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/62c7a1e35f56406896d7aa7ca52d0cc0.txt), [LICENSE-MIT](licenses/23f18e03dc49df91622fe2a761764974.txt) |
| [syn](https://crates.io/crates/syn/3.0.9) | 3.0.9 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/62c7a1e35f56406896d7aa7ca52d0cc0.txt), [LICENSE-MIT](licenses/23f18e03dc49df91622fe2a761764974.txt) |
| [sync_wrapper](https://crates.io/crates/sync_wrapper/1.0.2) | 1.0.2 | Apache-2.0 | [LICENSE](licenses/0d542e0c8804e39aa7f37eb00da5a762.txt) |
| [thiserror](https://crates.io/crates/thiserror/2.0.21) | 2.0.21 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/62c7a1e35f56406896d7aa7ca52d0cc0.txt), [LICENSE-MIT](licenses/23f18e03dc49df91622fe2a761764974.txt) |
| [thiserror-impl](https://crates.io/crates/thiserror-impl/2.0.21) | 2.0.21 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/62c7a1e35f56406896d7aa7ca52d0cc0.txt), [LICENSE-MIT](licenses/23f18e03dc49df91622fe2a761764974.txt) |
| [tokio](https://crates.io/crates/tokio/1.54.1) | 1.54.1 | MIT | [LICENSE](licenses/253cd04c6714889df2d32f3f64d66917.txt) |
| [tokio-macros](https://crates.io/crates/tokio-macros/2.7.2) | 2.7.2 | MIT | [LICENSE](licenses/0b83dc40cba89b9922bb84b0a9c7d276.txt) |
| [tokio-stream](https://crates.io/crates/tokio-stream/0.1.19) | 0.1.19 | MIT | [LICENSE](licenses/253cd04c6714889df2d32f3f64d66917.txt) |
| [tokio-util](https://crates.io/crates/tokio-util/0.7.20) | 0.7.20 | MIT | [LICENSE](licenses/253cd04c6714889df2d32f3f64d66917.txt) |
| [tower](https://crates.io/crates/tower/0.5.3) | 0.5.3 | MIT | [LICENSE](licenses/4249c8e6c5ebb85f97c77e6457c6fafc.txt) |
| [tower-layer](https://crates.io/crates/tower-layer/0.3.3) | 0.3.3 | MIT | [LICENSE](licenses/4249c8e6c5ebb85f97c77e6457c6fafc.txt) |
| [tower-service](https://crates.io/crates/tower-service/0.3.3) | 0.3.3 | MIT | [LICENSE](licenses/4249c8e6c5ebb85f97c77e6457c6fafc.txt) |
| [tracing](https://crates.io/crates/tracing/0.1.44) | 0.1.44 | MIT | [LICENSE](licenses/898b1ae9821e98daf8964c8d6c7f6164.txt) |
| [tracing-attributes](https://crates.io/crates/tracing-attributes/0.1.31) | 0.1.31 | MIT | [LICENSE](licenses/898b1ae9821e98daf8964c8d6c7f6164.txt) |
| [tracing-core](https://crates.io/crates/tracing-core/0.1.36) | 0.1.36 | MIT | [LICENSE](licenses/898b1ae9821e98daf8964c8d6c7f6164.txt) |
| [unicode-ident](https://crates.io/crates/unicode-ident/1.0.27) | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 | [LICENSE-APACHE](licenses/62c7a1e35f56406896d7aa7ca52d0cc0.txt), [LICENSE-MIT](licenses/23f18e03dc49df91622fe2a761764974.txt), [LICENSE-UNICODE](licenses/f7db81051789b729fea528a63ec4c938.txt) |
| [uuid](https://crates.io/crates/uuid/1.29.0) | 1.29.0 | Apache-2.0 OR MIT | [LICENSE-APACHE](licenses/a60eea817514531668d7e00765731449.txt), [LICENSE-MIT](licenses/436bc5a105d8e57dcd8778730f3754f7.txt) |
| [vcpkg](https://crates.io/crates/vcpkg/0.2.15) | 0.2.15 | MIT/Apache-2.0 | [LICENSE-APACHE](licenses/3708458dee7f359ac6c9c5558023ed48.txt), [LICENSE-MIT](licenses/ff7d99434986ba68f6575d689fd9cc11.txt) |
| [wasi](https://crates.io/crates/wasi/0.11.1+wasi-snapshot-preview1) | 0.11.1+wasi-snapshot-preview1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | [LICENSE-APACHE](licenses/a60eea817514531668d7e00765731449.txt), [LICENSE-Apache-2.0_WITH_LLVM-exception](licenses/268872b9816f90fd8e85db5a28d33f81.txt), [LICENSE-MIT](licenses/23f18e03dc49df91622fe2a761764974.txt) |
| [wasip2](https://crates.io/crates/wasip2/1.0.4+wasi-0.2.12) | 1.0.4+wasi-0.2.12 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | [LICENSE-APACHE](licenses/a60eea817514531668d7e00765731449.txt), [LICENSE-Apache-2.0_WITH_LLVM-exception](licenses/268872b9816f90fd8e85db5a28d33f81.txt), [LICENSE-MIT](licenses/23f18e03dc49df91622fe2a761764974.txt) |
| [wasm-bindgen](https://crates.io/crates/wasm-bindgen/0.2.129) | 0.2.129 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/a60eea817514531668d7e00765731449.txt), [LICENSE-MIT](licenses/378f5840b258e2779c39418f3f2d7b2b.txt) |
| [wasm-bindgen-macro](https://crates.io/crates/wasm-bindgen-macro/0.2.129) | 0.2.129 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/a60eea817514531668d7e00765731449.txt), [LICENSE-MIT](licenses/378f5840b258e2779c39418f3f2d7b2b.txt) |
| [wasm-bindgen-macro-support](https://crates.io/crates/wasm-bindgen-macro-support/0.2.129) | 0.2.129 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/a60eea817514531668d7e00765731449.txt), [LICENSE-MIT](licenses/378f5840b258e2779c39418f3f2d7b2b.txt) |
| [wasm-bindgen-shared](https://crates.io/crates/wasm-bindgen-shared/0.2.129) | 0.2.129 | MIT OR Apache-2.0 | [LICENSE-APACHE](licenses/a60eea817514531668d7e00765731449.txt), [LICENSE-MIT](licenses/378f5840b258e2779c39418f3f2d7b2b.txt) |
| [windows](https://crates.io/crates/windows/0.62.2) | 0.62.2 | MIT OR Apache-2.0 | [license-apache-2.0](licenses/c16f8dcf1a368b83be78d826ea23de40.txt), [license-mit](licenses/c2cfccb812fe482101a8f04597dfc5a9.txt) |
| [windows-collections](https://crates.io/crates/windows-collections/0.3.2) | 0.3.2 | MIT OR Apache-2.0 | [license-apache-2.0](licenses/c16f8dcf1a368b83be78d826ea23de40.txt), [license-mit](licenses/c2cfccb812fe482101a8f04597dfc5a9.txt) |
| [windows-core](https://crates.io/crates/windows-core/0.62.2) | 0.62.2 | MIT OR Apache-2.0 | [license-apache-2.0](licenses/c16f8dcf1a368b83be78d826ea23de40.txt), [license-mit](licenses/c2cfccb812fe482101a8f04597dfc5a9.txt) |
| [windows-future](https://crates.io/crates/windows-future/0.3.2) | 0.3.2 | MIT OR Apache-2.0 | [license-apache-2.0](licenses/c16f8dcf1a368b83be78d826ea23de40.txt), [license-mit](licenses/c2cfccb812fe482101a8f04597dfc5a9.txt) |
| [windows-implement](https://crates.io/crates/windows-implement/0.60.2) | 0.60.2 | MIT OR Apache-2.0 | [license-apache-2.0](licenses/c16f8dcf1a368b83be78d826ea23de40.txt), [license-mit](licenses/c2cfccb812fe482101a8f04597dfc5a9.txt) |
| [windows-interface](https://crates.io/crates/windows-interface/0.59.3) | 0.59.3 | MIT OR Apache-2.0 | [license-apache-2.0](licenses/c16f8dcf1a368b83be78d826ea23de40.txt), [license-mit](licenses/c2cfccb812fe482101a8f04597dfc5a9.txt) |
| [windows-link](https://crates.io/crates/windows-link/0.2.1) | 0.2.1 | MIT OR Apache-2.0 | [license-apache-2.0](licenses/c16f8dcf1a368b83be78d826ea23de40.txt), [license-mit](licenses/c2cfccb812fe482101a8f04597dfc5a9.txt) |
| [windows-numerics](https://crates.io/crates/windows-numerics/0.3.1) | 0.3.1 | MIT OR Apache-2.0 | [license-apache-2.0](licenses/c16f8dcf1a368b83be78d826ea23de40.txt), [license-mit](licenses/c2cfccb812fe482101a8f04597dfc5a9.txt) |
| [windows-result](https://crates.io/crates/windows-result/0.4.1) | 0.4.1 | MIT OR Apache-2.0 | [license-apache-2.0](licenses/c16f8dcf1a368b83be78d826ea23de40.txt), [license-mit](licenses/c2cfccb812fe482101a8f04597dfc5a9.txt) |
| [windows-strings](https://crates.io/crates/windows-strings/0.5.1) | 0.5.1 | MIT OR Apache-2.0 | [license-apache-2.0](licenses/c16f8dcf1a368b83be78d826ea23de40.txt), [license-mit](licenses/c2cfccb812fe482101a8f04597dfc5a9.txt) |
| [windows-sys](https://crates.io/crates/windows-sys/0.61.2) | 0.61.2 | MIT OR Apache-2.0 | [license-apache-2.0](licenses/c16f8dcf1a368b83be78d826ea23de40.txt), [license-mit](licenses/c2cfccb812fe482101a8f04597dfc5a9.txt) |
| [windows-threading](https://crates.io/crates/windows-threading/0.2.1) | 0.2.1 | MIT OR Apache-2.0 | [license-apache-2.0](licenses/c16f8dcf1a368b83be78d826ea23de40.txt), [license-mit](licenses/c2cfccb812fe482101a8f04597dfc5a9.txt) |
| [wit-bindgen](https://crates.io/crates/wit-bindgen/0.57.1) | 0.57.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | [LICENSE-APACHE](licenses/a60eea817514531668d7e00765731449.txt), [LICENSE-Apache-2.0_WITH_LLVM-exception](licenses/268872b9816f90fd8e85db5a28d33f81.txt), [LICENSE-MIT](licenses/23f18e03dc49df91622fe2a761764974.txt) |
| [zmij](https://crates.io/crates/zmij/1.0.23) | 1.0.23 | MIT | [LICENSE-MIT](licenses/23f18e03dc49df91622fe2a761764974.txt) |
