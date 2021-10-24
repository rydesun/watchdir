use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use ahash::AHashMap;

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

    pub fn insert(&mut self, path: &Path, value: T) {
        let path_rest = path.strip_prefix(&self.prefix).unwrap();
        let new_node = match &self.tree {
            Some(node) => Node::insert(Rc::clone(node), path_rest, value),
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
    }

    pub fn delete(&mut self, value: T) -> Vec<T> {
        let node = self.table.get(&value).unwrap();
        let path = node.borrow().path();
        let path_rest = path.strip_prefix(&self.prefix).unwrap();
        let values =
            Node::pop(Rc::clone(self.tree.as_ref().unwrap()), path_rest)
                .unwrap()
                .borrow()
                .values();
        for v in &values {
            self.table.remove(v);
        }
        values
    }

    pub fn rename(&self, value: T, new_path: &Path) {
        let node = self.table.get(&value).unwrap();
        let old_path = node.borrow().path();
        let old_path_rest = old_path.strip_prefix(&self.prefix).unwrap();
        let new_path_rest = new_path.strip_prefix(&self.prefix).unwrap();
        Node::rename(
            Rc::clone(self.tree.as_ref().unwrap()),
            old_path_rest,
            new_path_rest,
        );
    }

    pub fn get_full_path(&self, value: T, path: &Path) -> PathBuf {
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
    ) -> Rc<RefCell<Node<T>>> {
        let parent = Self::get(self_, path.parent().unwrap()).unwrap();

        let key = path.file_name().unwrap();
        let node = Rc::new(RefCell::new(Self::new(
            key.to_owned(),
            value,
            Some(&parent),
        )));

        parent.borrow_mut().children.insert(key.to_owned(), Rc::clone(&node));

        node
    }

    fn pop(
        self_: Rc<RefCell<Self>>,
        path: &Path,
    ) -> Option<Rc<RefCell<Self>>> {
        let parent = Self::get(self_, path.parent().unwrap())?;
        let x = parent.borrow_mut().children.remove(path.file_name().unwrap());
        x
    }

    fn rename(self_: Rc<RefCell<Self>>, old_path: &Path, new_path: &Path) {
        let node = Self::pop(Rc::clone(&self_), old_path).unwrap();
        let parent = Self::get(self_, new_path.parent().unwrap()).unwrap();

        let new_name = new_path.file_name().unwrap();
        node.borrow_mut().key = new_name.to_owned();
        node.borrow_mut().parent = Rc::downgrade(&parent);
        parent.borrow_mut().children.insert(new_name.to_owned(), node);
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
