use std::path::PathBuf;
use std::{env, fs};

use chrono::prelude::*;

fn main() {
    if let Some(git_sha) = get_git_sha() {
        println!("cargo:rustc-env=GIT_SHA={}", git_sha);
    }

    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    let time = match env::var("SOURCE_DATE_EPOCH") {
        Ok(t) => t.parse::<i64>().unwrap(),
        _ => Utc::now().timestamp(),
    };
    println!("cargo:rustc-env=BUILD_DATE={}", time);
}

fn get_git_sha() -> Option<String> {
    let mut cwd = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    let mut gitdir;
    loop {
        gitdir = cwd.join(".git");
        if gitdir.is_dir() {
            break;
        } else if !cwd.pop() {
            return None;
        }
    }

    let git_head_file = gitdir.join("HEAD");
    if let Some(path) = git_head_file.to_str() {
        println!("cargo:rerun-if-changed={}", path);
    }

    if let Ok(mut head_content) = fs::read_to_string(&git_head_file) {
        if head_content.ends_with('\n') {
            head_content.pop();
        }

        if let Some(ref_file) = head_content.strip_prefix("ref: ") {
            let ref_file = gitdir.join(&ref_file);
            if !ref_file.is_file() {
                return None;
            }
            if let Some(path) = ref_file.to_str() {
                println!("cargo:rerun-if-changed={}", path);
            }
            return fs::read_to_string(&ref_file).ok();
        } else if head_content.len() == 40 {
            return Some(head_content);
        }
    }
    None
}
