# Corrosion

Vendored copy of [Corrosion](https://github.com/corrosion-rs/corrosion) v0.6.1
(commit 1499b14e4906a2890f5cee1547c8848db261753d), MIT licensed: only
`CMakeLists.txt`, `cmake/` and `LICENSE`, unmodified.

It is added with `add_subdirectory()` rather than `include()`, because
`Corrosion.cmake` calls `cmake_minimum_required(VERSION 3.22)`, which would
otherwise change the CMake policies of the whole project.

To update: replace these files with the same ones from a newer release tag and
update the version and commit above.
