# The master rewrite of 2026-11-25

On 2026-11-25 the owner had `master`'s history rewritten so that no commit on it credits Claude as
an author or co-author. Two commits did, both from pull request #409 (2026-11-23), authored and
committed as `Claude <noreply@anthropic.com>` with a `Co-Authored-By: Claude Fable 5.2` trailer and
a `Claude-Session:` line:

| Before | After | Commit |
|---|---|---|
| `61ab575d` | `52449ffc` | feat(capacity): track capacity for the machine, with a tracker outside the engine |
| `8f7a91ae` | `ab822fb8` | Merge master into machine-level capacity, reconciling it with #403 |

Each now names the `eventloops` account as author and committer, keeps its original dates, and has
the `Co-Authored-By:` and `Claude-Session:` lines and their preceding blank separator removed.
The rest of each message is byte-identical.
The owner chose to change these two commits only: older commits that carry a `Claude-Session:` line
and no Claude author or co-author are unchanged.

Every other descendant of those two commits was re-created with updated parent lines. Its tree,
author, committer, dates and message are byte-identical, and every tree on the rewritten `master`
equals the tree it replaced. `master` moved from `0d45280c` to `50c11ded`. A signature on a
re-created commit could not be kept, since it covered the old parents. Open pull requests were moved
onto the new history the same way, with their diffs unchanged.

`upstroke-pr-policy` refuses a pull request whose own commits carry a credit form listed in
`.github/scripts/validate-pr-commits.sh`, a check added with this record.

## Old and new commit ids

The map covers the commits that were on `master` at the rewrite. A commit id from the old `master`,
cited before this date in `findings/`, a pull request, a review or a ruling, that appears in the left
column is the commit in the right column on today's `master`; one from the old `master` that does not
appear was not rewritten. Commits on the pull-request branches open at the time were moved onto the
new history the same way and also have new ids; they are not listed here, and a pull request's own
page names its current commits.

| Before | After |
|---|---|
| `61ab575dee162cd245fc511e4c7d4bb10e335637` | `52449ffcdb5692b75d701d2eb9653b583817ff19` |
| `8f7a91ae8947f6d3ff830a83d1d58a5db562f292` | `ab822fb8b1975d57a27b7f90a8eabb3ecccd1510` |
| `99841f95cbde722c42d627e6583b248d27407020` | `7d1d7cb60787210164086200dcfdda12d3c1b85b` |
| `cec175bfbf9ef960917ad966e1ad8fd6a4aa9b18` | `255f576a656f35ab2aeff697a68b606444cd8b23` |
| `e380550f6a17889599fbab75231881103eaf08a2` | `6c993101236535a25bb30fa4978f954e2c6e36d3` |
| `82bdf72070380849878497a766bb7e485f7b4eb1` | `2cd6f238f47b6235dc34340cd1374b1d022a2035` |
| `9e1a63434116bb1cefe1deff14e43b3d28ecbdc2` | `b7437eb3ed2d51f4e45a5d35f1f438c68592df4d` |
| `71d2a0cfad505f9b366668d0722ac530a72a3412` | `a48c5f96aac2ad94075a011e429fd4b9ad8ad27f` |
| `116748fed9e27c937e7a413523112841d8e8f7b8` | `99fcb43808a8413fc0600690952a8910106cb127` |
| `32fb664b6a0259dab69496a22c81cdfdbc6e0341` | `8d3a5394b9cd82154838990c8dd59f2fba6243f5` |
| `08c7f16271e9278bf314b6c6a87d3be40a0b88d0` | `9c0bee7cd959b8ebc6862ed954df0abb804006d7` |
| `759cbcc38bf599e42c0c838411829a9d9322692c` | `43093307fc22f6e80ad52f746e69542ab1a4fa7f` |
| `6de40c83fb2533d999f1223b39fc14267c1fc397` | `7c7f381e4d564146f43a13189f2d65f63d294cff` |
| `825f4b641712a2f0f94a6f7b69eb85c2bfdbd3e7` | `8dcc0ce6a3120918dcaf3493552f3ac8cf9c4d61` |
| `87cac8016b4aab4a24718469dc1253d5447013ff` | `7955ce03ef0ff59597d416653a8cc219d5e2af02` |
| `79682552d7091a3366da0cad3a03dc7fa25c0811` | `600ad1128dd49794b8559305ebd8877cbd2baa40` |
| `e2da11224878d65f6836131d6f89c2f3df847685` | `b16e5b1edc80e39ab568b4c21d6083684c5e0a16` |
| `675fc38ee2be92260b75fd947708057ee9f98b40` | `c729050b2dc86bdcbb36d1494214e1126dc37fbe` |
| `5ded81c0d51e9be811893efb0f87246dfaea42da` | `a9b086c19cf44523fb755b2bd5551fb9b2ac56d6` |
| `0161b0674f14205755df9005a9984cd5c05dbcf4` | `cca98da72ce11e24bf6f73bb5a4f7bdbaf52589d` |
| `378bef1da848dd67e622d5a4131f123da0620e37` | `178a8cdf3c243ec6b07aca268d4124405fb0a1ca` |
| `c5061b817c9e61993bf19cf43a06cf81720bdefe` | `b21d9659c3312e87f16ad16570fecf7450389b01` |
| `81bb1ab2fcb49350fc39939ecd0d72971cf892ac` | `56ec6380bd5188caf2f2287380e158aa3f806e08` |
| `d3ae08c6182899776fea326f9ead8efd1132e073` | `bdae239b38118ebacb17f9da73f6d29df6d7b2e5` |
| `79ad7e903cd59f78d541856744d4664eb8e7cdab` | `0e757f62bb8e94b5ad0d623fd5403604beace4b8` |
| `a7776da9567991012a3e77d83fbe502d923dc578` | `32d60d50fc4275378d7dd8ef40179eee5fff368f` |
| `ad88180bc16505194d3d49f0bbfa1e653a2aa0b3` | `67dd5504177514e9be2130a0b38b59c1caf15af0` |
| `1b11be12b126f64888818842f4eda1e9e8d45513` | `865a6c32c6380706d7da4a470cbf273f098cc073` |
| `d5ceea1c1514811a5690a00e9d21776d55d401df` | `1f00dec5b1ccd3040be8993a1308af9e59bd5667` |
| `80edca754c8ce36837116c1ac5ee87b063d51a23` | `3726e4bd53772c2a46fe9098cebf3d431e89af4e` |
| `b921e4a606f0e9f9f5e1681015209279bb331fa6` | `b94f61f7b10a6f19e1ca8d426a88f83b9c0a349d` |
| `30733b93d9e7f052dd6dfca55b8c5b3656f4be49` | `565987d6b71b746c3c85e4905803090ee3974dec` |
| `b11ba8a1ba09bf60e42534c4d9c367be1db73d34` | `b4361a2dceec982a2f2775843686b33a364b0cf9` |
| `fd7c7324936aefb16a35cc943608b04e98c67853` | `387c5b39d355707c77e2c40a9f0bdd33f36384f3` |
| `5cda2c869336188af130c30cd990f27059f355f5` | `c6353be4c4a66eea3d2bf1c296f6825c470f1141` |
| `d6a743439a7be70c49899e319a693fafc2ac4259` | `7bf13e35567d15498d76c927099e4b3347498953` |
| `9030a83a6a77895f8415cb830090ff23a5d8d7a5` | `9ff44d54e86b4cbb1c4ab4565d897771c62ab0b2` |
| `39747d82566955152f72fa30f301ff9b59518bcf` | `8bb63e0e35ebad07aa8b89f044e7cdee844eb92e` |
| `54ff649c9759da307934e041501e477a6402c9af` | `5f7b5b71daeb3382d0575fbd9b2373e409554b35` |
| `fee065883995755c71f27ce3e438d02608b9eccc` | `41b2c1ecce001c9d2d62d50deba37b222e0f4cb8` |
| `cdeacc703f5be47889946746fefa500fd57f15f4` | `bd4796599910d9235e9403db844c75794bfcafc3` |
| `6d7fb532c6659bf1051d52217e56a36ddca6983f` | `88dd27ecd15841a2e72eaeb2732b22dfb819e39a` |
| `39863d6fe76263a7e952baf27a2d5d68d87cabf6` | `aad74f9943ba5cb7b2d8c800f7e4204e2a7fb280` |
| `3060dfb7dcfadf743a9d851fc933dcc03ae75cff` | `19c4dd72c999a4d070a9fe4e3d439e562aaa8f9c` |
| `870e4e0f0e4991ce24d5f0f871522daed4155a71` | `e6afd228c3bce258ca14206a3bb43b7cd0520e27` |
| `ca85d8958c95ce2b45700fae678fb57c23244dcb` | `ca6009eee2c89e4f6b28523f109a71e76ada237a` |
| `1724628d80193f92bf5890da1923a4530f15a617` | `3d8d26bbca0f5dab9da9ace2939e099caba0f27d` |
| `3d96aee8aafb78ddfd6f80036faa43d696ffb2d6` | `19c8c8a660b240936e74b805302af5aa821b83af` |
| `8be4c1485c1f3cc0000e60b1cefc5b47d21ed9ca` | `854c20fec414d3bf54647254fb4df8cfa498ac09` |
| `72a0e630542c94771cae78d8048a19a2e89dec7b` | `654160f2d485a30e92e69e9c568be023157a261f` |
| `7686020c2e8f02946d8f2d38ad405fc76d56f5c5` | `9a9c798046a68255d42717cddc17b6683ece8d05` |
| `9ba051b81a95c94ea7a42856d6b75082b49524c7` | `b7907ab52ae4e73cd0da9081da14d19b77d8f07b` |
| `e17fb6c96dc53f2bb98c0641a17175848976e956` | `119dce39026fb098efc47d54dd66a4f38f6a5862` |
| `1e7c6a8a1cf2af8fb60f540de8afdd21241efa9b` | `0b2eb929b19e96e332e3e6cebca1939112db0a60` |
| `30e0ea0e32dcf32e0fc5d96d7013fd1593211d96` | `2db2f661d9f72601e4765dc387cdffc37cc1de44` |
| `6abbe1c35a62dab4fd25db7d43d45390733c1947` | `40cf90faa10e7bf638358067996edbf68a432ee5` |
| `b2f90d99d7d0758627d4303cb57ac8f2fec62772` | `69b8965609b712a29fc067774d0c83ca8b0d2269` |
| `457c0692ad1269f26f19016704dcf21e1f698026` | `b589112209d8ffca981313c2eb16eb0c12643b24` |
| `ca825a86ba0d4a674be88c42b85aa2085031a5c5` | `2e09395fbe1be0cfb662a1b50ae912430ac1a596` |
| `469666ed8ea0fe25076214a1c78293fa640c1ac9` | `ec2efe078d375ba8e4a1490b0d8a95993d264b5d` |
| `681f429f8f3dba7c19ed248f69711b59a1fe17b1` | `18c0ec775848eb57024e6121f29060e708bfe94e` |
| `058e4da0bf9ec85c01a38d8558c6142bdd0041d5` | `b261a400de989b3e0b76f9b6b43a65484bd2a7e9` |
| `a142defc853803de628a89705b32f78eaaa0f0ca` | `ef14e5aab2fe2cb47b5eebbda7db6299e95a222c` |
| `d1dfaf60072a0965ed4bb5c302be3d736b9663b4` | `d1dd69f1e02e11c3cdb1a568712a7223cac6a966` |
| `483ee5bd6db53627bdd8657a9cf88b3ea3401687` | `19891d5030e3f92c4c55b29a8f7506720d549c61` |
| `75a5143add98542b99d3c83c224f5851512517f0` | `8f4f5bb725acab26145f7e1cfd23b44f8e2cb79e` |
| `e1e39e974f631203bd4609926b6738470b54ec46` | `3536e271cce19c8292ea836aeded643b86e54154` |
| `da87e3325038fade65a206de93c194975f35daa4` | `32bd217265e1262776bea9ac3f98a478a352b524` |
| `aa811e226a4068b896aed1a5c2f35907663f6685` | `af02a2b4154f20833b28b806b9c9a2fac1c8b995` |
| `3b016a458e5e46a88b1963e7d959bff6fd60ed72` | `27d95b38b0cd5a32eb3df32ba813d7c8daf27c0d` |
| `533d4713ad2b5ad9c8744a311c038f2881778b83` | `47dc49fe990ea844d8297c29b1785cf43f52d18e` |
| `3b40b704dc7436e988be671e077b96e978f195ba` | `170cf078699597df540282f8cd4bd688bdb68f64` |
| `c442aeb31923f82ee27d60bd1d7a58bee1f93502` | `7934a7b085237d52a4509432f5a34f8fb3dc47b1` |
| `5ef9901dadb612e5fa9bfaf04160753a5e90416b` | `bb50b4762120f21270bd13292d125f4005fbeb99` |
| `e1e44c84df4af69e28a4db96a341cc819ef3acc2` | `69bddc6a5c66ec5d9e64b5be93fe2cdaf20b78aa` |
| `18d9206e2839fb7867d989c673331b428c4f8856` | `18cbc3211fa07327f991c1f809505067da569f79` |
| `3271e5e60cda8f8350c118983cb6deb72d06e758` | `604f32b0809041245d002bd67fffea0cd634a23c` |
| `517763346fdb3d20f77fba18742c2eefce7aed32` | `13ec946c0d7a835ac39222be6fec4b50a2afee2e` |
| `63549dbc530e7588653cc19bbd206738311d124f` | `f1ba5cfd78311fc9a7bf55fef666261ee80b98bc` |
| `0d45280c2605c0e401bd76d63002c476c443aa58` | `50c11ded63e3d4e5ea87b07d3fc25caff0c46bd3` |
