# watchdir

A simple tool to find newly created files by watching several directories. Watch at directory in shallow depth, not recursively.

## Usage

- Watch several directories

```bash
watchdir /etc ~/.config
```

- Watch dotfiles created in home directory

```bash
watchdir ~ | grep ".*/\."
```

Then someone create dotfile in my home directory `/home/rydesun`

```bash
touch ~/.hidden_file
```

And I can find this line in the output

```
/home/rydesun/.hidden_file
```

## Installation

```bash
cargo install --git https://github.com/rydesun/watchdir
```
