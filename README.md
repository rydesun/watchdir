# watchdir

A simple tool to find newly created files or directories by watching several directories,
and do it recursively.

When diving into deeper directory recursively,
a directory will be ignored in the following situations:

- have no permission
- is a symlink

Also by default, it will not ignore created hidden dotfiles or directories,
but will not dive into hidden subdirectories.
Use `--hidden` to supress this behavior.

## Usage

- Watch several directories

```bash
watchdir /etc ~/.config
```

- Watch dotfiles created in home config directory

```bash
watchdir ~/.config | grep ".*/\."
```

For example, someone create a dotfile in my home config directory `/home/rydesun/.config/`

```bash
touch ~/.config/.hidden_file
```

And find this line in the output

```text
/home/rydesun/.config/.hidden_file
```

**IMPORTANT**: DO NOT watch at a large directory, such as `/` or `~`,
it do harm to performance.

## Installation

```bash
cargo install --locked --git https://github.com/rydesun/watchdir
```
