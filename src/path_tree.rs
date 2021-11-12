use std::{
    collections::HashMap,
    ffi::OsString,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, Weak},
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
    table: AHashMap<T, Arc<Mutex<Node<T>>>>,
    tree: Option<Arc<Mutex<Node<T>>>>,
}

impl<T> Head<T>
where
    T: std::hash::Hash + std::cmp::Eq + Copy,
{
    pub fn new(prefix: PathBuf) -> Self {
        Self { prefix, tree: None, table: AHashMap::new() }
    }

    pub fn has(&self, value: T) -> bool {
        self.table.contains_key(&value)
    }

    pub fn insert(&mut self, path: &Path, value: T) -> Result<()> {
        let path_rest = path
            .strip_prefix(&self.prefix)
            .context(PrefixMismatched { path })?;
        let new_node = match &self.tree {
            Some(node) => Node::insert(Arc::clone(node), path_rest, value)?,
            None => {
                let node = Arc::new(Mutex::new(Node::new(
                    path.as_os_str().to_owned(),
                    value,
                    None,
                )));
                self.tree = Some(Arc::clone(&node));
                node
            }
        };
        self.table.insert(value, new_node);
        Ok(())
    }

    pub fn delete(&mut self, value: T) -> Result<Vec<T>> {
        let node = self.table.get(&value).context(ValueNotFound)?;
        let path = node.lock().unwrap().path();
        let path_rest = path
            .strip_prefix(&self.prefix)
            .context(PrefixMismatched { path: path.to_owned() })?;
        let values = {
            if path_rest.as_os_str().is_empty() {
                self.tree.take().unwrap().lock().unwrap().values()
            } else {
                let tree = self.tree.as_ref().context(EmptyTree)?;
                Node::pop(Arc::clone(tree), path_rest)?
                    .lock()
                    .unwrap()
                    .values()
            }
        };
        for v in &values {
            self.table.remove(v);
        }
        Ok(values)
    }

    pub fn rename(&self, value: T, new_path: &Path) -> Result<()> {
        let node = self.table.get(&value).context(ValueNotFound)?;
        let old_path = node.lock().unwrap().path();
        let old_path_rest = old_path
            .strip_prefix(&self.prefix)
            .context(PrefixMismatched { path: old_path.to_owned() })?;
        let new_path_rest = new_path
            .strip_prefix(&self.prefix)
            .context(PrefixMismatched { path: new_path })?;
        let tree = self.tree.as_ref().context(EmptyTree)?;
        Node::rename(Arc::clone(tree), old_path_rest, new_path_rest)
    }

    pub fn path(&self, value: T) -> PathBuf {
        self.table[&value].lock().unwrap().path()
    }

    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.table.keys()
    }
}

pub struct Node<T> {
    key: OsString,
    value: T,
    parent: Weak<Mutex<Node<T>>>,
    children: HashMap<OsString, Arc<Mutex<Node<T>>>>,
}

impl<T> Node<T>
where
    T: std::hash::Hash + std::cmp::Eq + Copy,
{
    fn new(
        key: OsString,
        value: T,
        parent: Option<&Arc<Mutex<Node<T>>>>,
    ) -> Self {
        Self {
            key,
            value,
            parent: match parent {
                Some(node) => Arc::downgrade(node),
                None => Weak::new(),
            },
            children: HashMap::new(),
        }
    }

    fn get(self_: Arc<Mutex<Self>>, path: &Path) -> Option<Arc<Mutex<Self>>> {
        let mut path = path.components();
        path.try_fold(self_, |acc, i| {
            let acc = acc.lock().unwrap();
            acc.children.get(i.as_os_str()).map(Arc::clone)
        })
    }

    fn insert(
        self_: Arc<Mutex<Self>>,
        path: &Path,
        value: T,
    ) -> Result<Arc<Mutex<Node<T>>>> {
        let parent = {
            let p = path.parent().context(InvalidPath { path })?;
            Self::get(self_, p).context(PathNotFound { path })?
        };
        let key = path.file_name().context(InvalidPath { path })?;
        let node = Arc::new(Mutex::new(Self::new(
            key.to_owned(),
            value,
            Some(&parent),
        )));

        parent
            .lock()
            .unwrap()
            .children
            .insert(key.to_owned(), Arc::clone(&node));

        Ok(node)
    }

    fn pop(self_: Arc<Mutex<Self>>, path: &Path) -> Result<Arc<Mutex<Self>>> {
        let name = path.file_name().context(InvalidPath { path })?;
        let parent = {
            let p = path.parent().context(InvalidPath { path })?;
            Self::get(self_, p).context(PathNotFound { path })?
        };
        let mut p = parent.lock().unwrap();
        p.children.remove(name).context(PathNotFound { path })
    }

    fn rename(
        self_: Arc<Mutex<Self>>,
        old_path: &Path,
        new_path: &Path,
    ) -> Result<()> {
        let node = Self::pop(Arc::clone(&self_), old_path)?;
        let parent = {
            let p =
                new_path.parent().context(InvalidPath { path: new_path })?;
            Self::get(self_, p).context(PathNotFound { path: new_path })?
        };

        let new_name =
            new_path.file_name().context(InvalidPath { path: new_path })?;
        node.lock().unwrap().key = new_name.to_owned();
        node.lock().unwrap().parent = Arc::downgrade(&parent);
        parent.lock().unwrap().children.insert(new_name.to_owned(), node);
        Ok(())
    }

    fn values(&self) -> Vec<T> {
        let mut values = vec![self.value];
        let mut stack: Vec<Arc<Mutex<Node<T>>>> =
            self.children.values().map(Arc::clone).collect();

        while let Some(node) = stack.pop() {
            let node = node.lock().unwrap();
            values.push(node.value);
            for c in node.children.values() {
                stack.push(Arc::clone(c));
            }
        }
        values
    }

    fn path(&self) -> PathBuf {
        let mut path = PathBuf::new();
        let mut temp = vec![self.key.to_owned()];

        let mut cur = self.parent.upgrade();
        while let Some(node) = cur {
            temp.push(node.lock().unwrap().key.to_owned());
            cur = node.lock().unwrap().parent.upgrade();
        }
        for i in temp.iter().rev() {
            path.push(i);
        }
        path
    }
}
