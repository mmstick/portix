use std::process::Command;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

#[allow(dead_code)]
#[derive(Debug, Eq)]
pub struct Pkg {
    name: String,
    versions: Vec<String>,
    desc: String,
}

impl Pkg {
    pub fn new(name: &str, versions: Vec<String>, desc: &str) -> Pkg {
        Pkg {name: name.to_string(), versions: Vec::new(), desc: desc.to_string() }
    }
}

impl Ord for Pkg {
    fn cmp(&self, other: &Pkg) -> ::std::cmp::Ordering {
        self.name.cmp(&other.name)
    }
}

impl PartialOrd for Pkg {
    fn partial_cmp(&self, other: &Pkg) -> Option<::std::cmp::Ordering> {
        Some(self.name.cmp(&other.name))
    }
}

impl PartialEq for Pkg {
    fn eq(&self, other: &Pkg) -> bool {
        self.name == other.name
    }
}

pub fn parse_data_with_eix(map: &mut BTreeMap<String, BTreeSet<Pkg>>) {
    let output = String::from_utf8(Command::new("sh")
            .arg("-c")
            .arg(r"NAMEVERSION='<category>/<name> <version> <description>\n' EIX_LIMIT_COMPACT=0 eix -c --format '<availableversions:NAMEVERSION>' --pure-packages")
            .output()
            .expect("failed to get eix output")
            .stdout
        ).expect("eix output is not UTF-8 compatible");

    let mut item = "this string is not empty for a reason";
    let mut desc = "";
    let mut versions: Vec<String> = Vec::new();
    for line in output.lines() {
        if line.starts_with(item) {
            let version_with_desc = &line[(line.find(' ').unwrap() + 1)..line.len()];
            let version = &version_with_desc[0..version_with_desc.find(' ').unwrap()];
            versions.push(version.to_string());
        }
        else {
            if !versions.is_empty() {
                let (category, pkg) = {
                    let mut item_split = item.split("/");
                    (item_split.next().unwrap(), item_split.next().unwrap())
                };
                println!("{:?} from {:?} as {:?} with {:?}", category, pkg, versions, desc);
                map.entry(category.to_string()).or_insert(BTreeSet::new()).insert(Pkg::new(pkg, versions.clone(), desc));
            }
            versions.clear();
            item = &line[0..line.find(' ').unwrap()];
            let version_with_desc = &line[(line.find(' ').unwrap() + 1)..line.len()];
            let version = &version_with_desc[0..version_with_desc.find(' ').unwrap()];
            desc = &version_with_desc[(version_with_desc.find(' ').unwrap() + 1)..version_with_desc.len()];
            versions.push(version.to_string());
        }
    }
}

#[allow(dead_code)]
pub fn parse_data_with_portageq(map: &mut BTreeMap<String, BTreeSet<Pkg>>) {
    let repos = String::from_utf8(Command::new("sh")
            .arg("-c")
            .arg("portageq get_repos /")
            .output()
            .expect("failed to get repos list")
            .stdout
        ).expect("repo names are not UTF-8 compatible");
    let repos: Vec<&str> = repos.trim().split(' ').collect();

    for repo in repos.iter() {
        let repo_path = String::from_utf8(Command::new("sh")
                .arg("-c")
                .arg(format!("portageq get_repo_path / {}", repo))
                .output()
                .expect("failed to find repo path")
                .stdout
            ).expect("repo path is not UTF-8 compatible");
        let repo_path = repo_path.trim();

        for category_dir in fs::read_dir(repo_path).expect("path does not exist") {
            let category_dir = category_dir.expect("intermittent IO error");
            let category = category_dir.file_name().into_string().unwrap();
            if category_dir.path().is_file() || category.starts_with(".") {
                continue;
            }
            map.entry(category.clone()).or_insert(BTreeSet::new());

            let package_dirs = match fs::read_dir(category_dir.path()) {
                    Ok(a) => a,
                    Err(_) => continue, // if it's just a file reset the loop
                };
            for package_dir in package_dirs {
                let package_dir = package_dir.expect("intermittent IO error");
                let package = package_dir.file_name().into_string().unwrap();
                if package_dir.path().is_file() || package.starts_with(".") {
                    continue;
                }
                //map.get_mut(&category).unwrap().insert(Pkg::new(&package));
            }
        }
    }
}

