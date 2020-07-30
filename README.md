# watchdir

A simple tool to find newly created files by watching several directories,
and do it recursively.

A directory is ignored in the following situations:

- have no permission
- is hidden by dot
- is a symlink

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
