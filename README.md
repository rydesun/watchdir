# watchdir

A simple tool to find newly operations in specified directory,
and do it recursively. It requires inotify to work properly.

When diving into deeper directory recursively,
a directory will be ignored in the following situations:

- no permission
- symlink

Also by default, it will ignore what happend in hidden directories.
Use `--hidden` option to supress this behavior.

## Usage

```bash
watchdir DIR
```

**IMPORTANT**: DO NOT watch at a large directory, such as `/` or `~`,
it do harm to performance.

## Installation

```bash
cargo install --locked --git https://github.com/rydesun/watchdir
```
