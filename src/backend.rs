use std::process::Command;
use std::collections::{HashMap, BTreeMap, BTreeSet};
use std::fs;

#[allow(dead_code)]
#[derive(Debug, Eq)]
pub struct Pkg {
    pub name: String,
    pub versions: Vec<String>,
    pub recommended_version: String,
    pub desc: String,
}

impl Pkg {
    pub fn new(name: &str, versions: Vec<String>, recommened_version: &str, desc: &str) -> Pkg {
        Pkg {
            name: name.to_string(),
            versions: versions,
            recommended_version: recommened_version.to_string(),
            desc: desc.to_string()
        }
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
    // TODO: run in parallel
    let mut output = String::from_utf8(Command::new("sh")
            .arg("-c")
            .arg(r"NAMEVERSION='<category>/<name> <version> <description>\n' EIX_LIMIT_COMPACT=0 eix -c --format '<availableversions:NAMEVERSION>' --pure-packages")
            .output()
            .expect("failed to get eix output")
            .stdout
        ).expect("eix output is not UTF-8 compatible");

    // TODO: run in parallel
    let recommened_version_output = String::from_utf8(Command::new("sh")
            .arg("-c")
            .arg(r"NAMEVERSION='<category>/<name> <version>\n' EIX_LIMIT_COMPACT=0 eix -c --format '<bestversion:NAMEVERSION>' --pure-packages")
            .output()
            .expect("failed to get eix output")
            .stdout
        ).expect("eix output is not UTF-8 compatible");
    let recommended_map: HashMap<_, _> = recommened_version_output.lines().map(|line| {
        let item = &line[0..line.find(' ').unwrap()];
        let version = &line[(line.find(' ').unwrap() + 1)..line.len()];
        (item, version)
    }).collect();

    // TODO: run in parallel
    let global_keywords = String::from_utf8(Command::new("sh")
            .arg("-c")
            .arg(r"emerge --info|grep ACCEPT_KEYWORDS")
            .output()
            .expect("failed to get eix output")
            .stdout
        ).expect("eix output is not UTF-8 compatible");
    let global_keywords: Vec<_> = global_keywords[(global_keywords.find("\"").unwrap() + 1)..global_keywords.rfind("\"").unwrap()].split(' ').collect();

    // TODO: run in parallel
    let arch_list = String::from_utf8(Command::new("sh")
            .arg("-c")
            .arg(r"cat $(portageq get_repo_path / gentoo)/profiles/arch.list")
            .output()
            .expect("failed to get eix output")
            .stdout
        ).expect("eix output is not UTF-8 compatible");
    let arch_list = {
        let mut list = Vec::new();
        for arch in arch_list.lines() {
            if arch.is_empty() {
                break;
            }
            list.push(arch);
        }
        list
    };

    let mut item = "this string is not empty for a reason";
    let mut desc = "";
    let mut versions: Vec<String> = Vec::new();
    output.push_str("extra line needed to get previous item in iterator\n");
    for line in output.lines() {
        let current_item = &line[0..line.find(' ').unwrap()];
        if current_item == item {
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
                //println!("{:?} from {:?} as {:?} with {:?}", category, pkg, versions, desc);
                map.entry(category.to_string())
                   .or_insert(BTreeSet::new())
                   .insert(Pkg::new(pkg, versions.clone(),
                                    recommended_map.get(item).unwrap_or({
                                        let mut keyword = &"";
                                        for global_keyword in global_keywords.iter() {
                                            for arch in arch_list.iter() {
                                                if global_keyword == arch {
                                                    keyword = &"Not available";
                                                    break;
                                                }
                                                else if *global_keyword == &format!("~{}", arch) {
                                                    keyword = &"Keyworded";
                                                    break;
                                                }
                                            }
                                        }
                                        keyword
                                    }), desc));
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

