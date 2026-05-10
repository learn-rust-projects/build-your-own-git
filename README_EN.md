# Build Your Own Git

This project is a challenge to implement a simplified version of Git from scratch. The goal is to understand Git's internal mechanics, including object storage, indexing, and commits, by building your own version step by step.

---

## Setup

This challenge is developed using [Codecrafters Course SDK](https://github.com/codecrafters-io/course-sdk). Please read the SDK README for information on:

- How to contribute language support.
- How to submit your solution.

Our project is based on this SDK and includes custom test scripts to automatically verify your implementation.

---

## Testing

Run the following command in the project root directory:

```bash
cd build-your-own-git
make test
```

This command will execute all automated tests, checking the correctness of your Git implementation, including:

Object storage (blobs, trees, commits)

Index file creation and updates

Commit objects and HEAD references

if you want to run the tests manually, please refer to [Test.md](./build-your-own-git/Test.md).

## License

Licensed under either of

 - Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
 - MIT license
   ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

## Ref

- [Step-by-Step Guide to Implementing the Git Protocol](https://i27ae15.github.io/git-protocol-doc/docs/git-protocol/intro)
- [http-protocol](https://git-scm.com/docs/http-protocol)
- [gitprotocol-pack](https://git-scm.com/docs/gitprotocol-pack)
- [Unpacking Git Packfiles](https://codewords.recurse.com/issues/three/unpacking-git-packfiles)
- [gitformat-pack](https://git-scm.com/docs/gitformat-pack)
- [note](https://github.com/Levio-z/learn-rust/blob/main/Archive/archive_links/projects/build-your-own-git.md)