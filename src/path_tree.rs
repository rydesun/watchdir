use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub struct Head {
    prefix: PathBuf,
    pair: HashMap<i32, PathBuf>,
    node: Option<Node>,
}

impl Head {
    pub fn new(prefix: PathBuf) -> Self {
        Self { prefix, node: None, pair: HashMap::new() }
    }

    pub fn get(&mut self, p: &Path) -> Option<&mut Node> {
        let rest = p.strip_prefix(&self.prefix).unwrap();
        let node = self.node.as_mut().unwrap();
        rest.components()
            .try_fold(node, |acc, i| acc.children.get_mut(i.as_os_str()))
    }

    pub fn insert(&mut self, p: &Path, value: i32) {
        let rest = p.strip_prefix(&self.prefix).unwrap();
        match &mut self.node {
            Some(node) => node.insert(rest, value),
            None => {
                self.node = Some(Node::new(rest.as_os_str().to_owned(), value))
            }
        }
        self.pair.insert(value, p.to_owned());
    }

    pub fn delete(&mut self, value: i32) -> Vec<i32> {
        let path = self.pair.get(&value).unwrap();
        let rest = path.strip_prefix(&self.prefix).unwrap();
        let values =
            self.node.as_mut().unwrap().delete(rest).unwrap().values();
        for v in &values {
            self.pair.remove(v);
        }
        values
    }

    pub fn rename(&mut self, value: i32, new: &Path) {
        let old = self.pair.get(&value).unwrap().to_owned();
        let old_rest = old.strip_prefix(&self.prefix).unwrap();
        let new_rest = new.strip_prefix(&self.prefix).unwrap();
        let values = self.get(&old).unwrap().values();
        let len = old.components().fold(0, |acc, _| acc + 1);

        self.node.as_mut().unwrap().rename(old_rest, new_rest);
        for v in values {
            let p = self.pair.get_mut(&v).unwrap();
            let new_dir = if v == value {
                new.to_owned()
            } else {
                new.join(p.components().skip(len).collect::<PathBuf>())
            };
            *p = new_dir;
        }
    }

    pub fn get_full_path(&self, value: i32, path: &Path) -> PathBuf {
        self.pair[&value].join(path)
    }

    pub fn values(&self) -> impl Iterator<Item = &i32> {
        self.pair.keys()
    }
}

pub struct Node {
    key: OsString,
    value: i32,
    children: HashMap<OsString, Node>,
}

impl Node {
    fn new(key: OsString, value: i32) -> Self {
        Self { key, value, children: HashMap::new() }
    }

    fn get(&mut self, kms: &Path) -> Option<&mut Self> {
        kms.components()
            .try_fold(self, |acc, i| acc.children.get_mut(i.as_os_str()))
    }

    fn insert(&mut self, kms: &Path, value: i32) {
        let node: &mut Node = self.get(kms.parent().unwrap()).unwrap();
        let component = kms.file_name().unwrap().to_owned();
        node.children
            .insert(component.to_owned(), Self::new(component, value));
    }

    fn delete(&mut self, kms: &Path) -> Option<Self> {
        let parent_node: &mut Node = self.get(kms.parent().unwrap()).unwrap();
        parent_node.children.remove(kms.file_name().unwrap())
    }

    fn rename(&mut self, old: &Path, new: &Path) {
        let mut node = self.delete(old).unwrap();
        node.key = new.file_name().unwrap().to_owned();
        let parent_node = self.get(new.parent().unwrap()).unwrap();
        parent_node.children.insert(node.key.to_owned(), node);
    }

    fn values(&self) -> Vec<i32> {
        let mut values = Vec::new();
        let mut stack = vec![self];

        while let Some(node) = stack.pop() {
            values.push(node.value);
            for v in node.children.values() {
                stack.push(v);
            }
        }

        values
    }
}
