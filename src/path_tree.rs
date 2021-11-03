use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::OsString,
    path::{Path, PathBuf},
    rc::{Rc, Weak},
};

use ahash::AHashMap;
use snafu::*;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("The prefixes of paths are mismatched: {}", path.display()))]
    PrefixMismatched { source: std::path::StripPrefixError, path: PathBuf },

    #[snafu(display("Unknown value"))]
    ValueNotFound,

    #[snafu(display("Path tree is empty"))]
    EmptyTree,

    #[snafu(display("Unknown path: {}", path.display()))]
    PathNotFound { path: PathBuf },

    #[snafu(display("invalid path: {}", path.display()))]
    InvalidPath { path: PathBuf },
}

type Result<T, E = Error> = std::result::Result<T, E>;

pub struct Head<T> {
    prefix: PathBuf,
    table: AHashMap<T, Rc<RefCell<Node<T>>>>,
    tree: Option<Rc<RefCell<Node<T>>>>,
}

impl<T> Head<T>
where
    T: std::hash::Hash + std::cmp::Eq + Copy,
{
    pub fn new(prefix: PathBuf) -> Self {
        Self { prefix, tree: None, table: AHashMap::new() }
    }

    pub fn insert(&mut self, path: &Path, value: T) -> Result<()> {
        let path_rest = path
            .strip_prefix(&self.prefix)
            .context(PrefixMismatched { path })?;
        let new_node = match &self.tree {
            Some(node) => Node::insert(Rc::clone(node), path_rest, value)?,
            None => {
                let node = Rc::new(RefCell::new(Node::new(
                    path.as_os_str().to_owned(),
                    value,
                    None,
                )));
                self.tree = Some(Rc::clone(&node));
                node
            }
        };
        self.table.insert(value, new_node);
        Ok(())
    }

    pub fn delete(&mut self, value: T) -> Result<Vec<T>> {
        let node = self.table.get(&value).context(ValueNotFound)?;
        let path = node.borrow().path();
        let path_rest = path
            .strip_prefix(&self.prefix)
            .context(PrefixMismatched { path: path.to_owned() })?;
        let values = {
            if path_rest.as_os_str().is_empty() {
                self.tree.take().unwrap().borrow().values()
            } else {
                let tree = self.tree.as_ref().context(EmptyTree)?;
                Node::pop(Rc::clone(tree), path_rest)?.borrow().values()
            }
        };
        for v in &values {
            self.table.remove(v);
        }
        Ok(values)
    }

    pub fn rename(&self, value: T, new_path: &Path) -> Result<()> {
        let node = self.table.get(&value).context(ValueNotFound)?;
        let old_path = node.borrow().path();
        let old_path_rest = old_path
            .strip_prefix(&self.prefix)
            .context(PrefixMismatched { path: old_path.to_owned() })?;
        let new_path_rest = new_path
            .strip_prefix(&self.prefix)
            .context(PrefixMismatched { path: new_path })?;
        let tree = self.tree.as_ref().context(EmptyTree)?;
        Node::rename(Rc::clone(tree), old_path_rest, new_path_rest)
    }

    pub fn full_path(&self, value: T, path: &Path) -> PathBuf {
        self.table[&value].borrow().path().join(path)
    }

    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.table.keys()
    }
}

pub struct Node<T> {
    key: OsString,
    value: T,
    parent: Weak<RefCell<Node<T>>>,
    children: HashMap<OsString, Rc<RefCell<Node<T>>>>,
}

impl<T> Node<T>
where
    T: std::hash::Hash + std::cmp::Eq + Copy,
{
    fn new(
        key: OsString,
        value: T,
        parent: Option<&Rc<RefCell<Node<T>>>>,
    ) -> Self {
        Self {
            key,
            value,
            parent: match parent {
                Some(node) => Rc::downgrade(node),
                None => Weak::new(),
            },
            children: HashMap::new(),
        }
    }

    fn get(
        self_: Rc<RefCell<Self>>,
        path: &Path,
    ) -> Option<Rc<RefCell<Self>>> {
        let mut path = path.components();
        path.try_fold(self_, |acc, i| {
            let acc = acc.borrow();
            acc.children.get(i.as_os_str()).map(Rc::clone)
        })
    }

    fn insert(
        self_: Rc<RefCell<Self>>,
        path: &Path,
        value: T,
    ) -> Result<Rc<RefCell<Node<T>>>> {
        let parent = {
            let p = path.parent().context(InvalidPath { path })?;
            Self::get(self_, p).context(PathNotFound { path })?
        };
        let key = path.file_name().context(InvalidPath { path })?;
        let node = Rc::new(RefCell::new(Self::new(
            key.to_owned(),
            value,
            Some(&parent),
        )));

        parent.borrow_mut().children.insert(key.to_owned(), Rc::clone(&node));

        Ok(node)
    }

    fn pop(
        self_: Rc<RefCell<Self>>,
        path: &Path,
    ) -> Result<Rc<RefCell<Self>>> {
        let name = path.file_name().context(InvalidPath { path })?;
        let parent = {
            let p = path.parent().context(InvalidPath { path })?;
            Self::get(self_, p).context(PathNotFound { path })?
        };
        let mut p = parent.borrow_mut();
        p.children.remove(name).context(PathNotFound { path })
    }

    fn rename(
        self_: Rc<RefCell<Self>>,
        old_path: &Path,
        new_path: &Path,
    ) -> Result<()> {
        let node = Self::pop(Rc::clone(&self_), old_path)?;
        let parent = {
            let p =
                new_path.parent().context(InvalidPath { path: new_path })?;
            Self::get(self_, p).context(PathNotFound { path: new_path })?
        };

        let new_name =
            new_path.file_name().context(InvalidPath { path: new_path })?;
        node.borrow_mut().key = new_name.to_owned();
        node.borrow_mut().parent = Rc::downgrade(&parent);
        parent.borrow_mut().children.insert(new_name.to_owned(), node);
        Ok(())
    }

    fn values(&self) -> Vec<T> {
        let mut values = vec![self.value];
        let mut stack: Vec<Rc<RefCell<Node<T>>>> =
            self.children.values().map(Rc::clone).collect();

        while let Some(node) = stack.pop() {
            let node = node.borrow();
            values.push(node.value);
            for c in node.children.values() {
                stack.push(Rc::clone(c));
            }
        }
        values
    }

    fn path(&self) -> PathBuf {
        let mut path = PathBuf::new();
        let mut temp = vec![self.key.to_owned()];

        let mut cur = self.parent.upgrade();
        while let Some(node) = cur {
            temp.push(node.borrow().key.to_owned());
            cur = node.borrow_mut().parent.upgrade();
        }
        for i in temp.iter().rev() {
            path.push(i);
        }
        path
    }
}
