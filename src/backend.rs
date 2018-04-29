extern crate rusqlite;

use self::rusqlite::Connection;

use std::collections::HashMap;
use std::fs;
use std::process::Command;
use std::thread;

pub trait PortixConnection {
    fn parse_for_pkgs(&self);
    fn parse_for_sets(&self);
}

impl PortixConnection for Connection {
    fn parse_for_pkgs(&self) {
        self.execute("CREATE TABLE all_packages (
                      category            TEXT,
                      package             TEXT,
                      versions            TEXT,
                      installed_version   TEXT,
                      recommended_version TEXT,
                      description         TEXT
                      )", &[]).unwrap();
        self.execute("CREATE TABLE installed_packages (
                      category            TEXT,
                      package             TEXT,
                      versions            TEXT,
                      installed_version   TEXT,
                      recommended_version TEXT,
                      description         TEXT
                      )", &[]).unwrap();

        let child_output = thread::spawn(move || {
            String::from_utf8(Command::new("sh")
                    .arg("-c")
                    .arg(r"NAMEVERSION='<category>/<name> <version> <description>\n' EIX_LIMIT_COMPACT=0 eix -c --format '<availableversions:NAMEVERSION>' --pure-packages")
                    .output()
                    .expect("failed to get eix output")
                    .stdout
                ).expect("eix output is not UTF-8 compatible")
        });

        let child_installed_version_output = thread::spawn(move || {
            let installed_version_output = String::from_utf8(Command::new("sh")
                    .arg("-c")
                    .arg(r"qlist -ICv|sed -re 's/-([0-9])/ \1/'")
                    .output()
                    .expect("failed to get qlist output")
                    .stdout
                ).expect("qlist output is not UTF-8 compatible");
            installed_version_output.lines().map(|line| {
                let item = &line[0..line.find(' ').unwrap()];
                let version = &line[(line.find(' ').unwrap() + 1)..line.len()];
                (item.to_string(), version.to_string())
            }).collect::<HashMap<_, _>>()
        });

        let child_recommended_version_output = thread::spawn(move || {
            let recommended_version_output = String::from_utf8(Command::new("sh")
                    .arg("-c")
                    .arg(r"NAMEVERSION='<category>/<name> <version>\n' EIX_LIMIT_COMPACT=0 eix -c --format '<bestversion:NAMEVERSION>' --pure-packages")
                    .output()
                    .expect("failed to get eix output")
                    .stdout
                ).expect("eix output is not UTF-8 compatible");
            recommended_version_output.lines().map(|line| {
                let item = &line[0..line.find(' ').unwrap()];
                let version = &line[(line.find(' ').unwrap() + 1)..line.len()];
                (item.to_string(), version.to_string())
            }).collect::<HashMap<_, _>>()
        });

        let child_global_keywords = thread::spawn(move || {
            let global_keywords = String::from_utf8(Command::new("sh")
                    .arg("-c")
                    .arg(r"emerge --info|grep ACCEPT_KEYWORDS")
                    .output()
                    .expect("failed to get emerge output")
                    .stdout
                ).expect("emerge output is not UTF-8 compatible");
            global_keywords[(global_keywords.find("\"").unwrap() + 1)..global_keywords.rfind("\"").unwrap()]
                .split(' ').map(|s| s.to_string()).collect::<Vec<_>>()
        });


        let child_arch_list = thread::spawn(move || {
            let arch_list = String::from_utf8(Command::new("sh")
                    .arg("-c")
                    .arg(r"cat $(portageq get_repo_path / gentoo)/profiles/arch.list")
                    .output()
                    .expect("failed to get portageq output")
                    .stdout
                ).expect("portageq output is not UTF-8 compatible");
            let mut list = Vec::new();
            for arch in arch_list.lines() {
                if arch.is_empty() {
                    break;
                }
                list.push(arch.to_string());
            }
            list
        });


        let mut item = String::new();
        let mut desc = String::new();
        let mut versions = String::new();
        let mut output = child_output.join().unwrap();
        output.push_str("extra line needed to get previous item in iterator\n");
        let installed_version_output_map = child_installed_version_output.join().unwrap();
        let recommended_version_output_map = child_recommended_version_output.join().unwrap();
        let global_keywords = child_global_keywords.join().unwrap();
        let arch_list = child_arch_list.join().unwrap();
        for line in output.lines().map(|line| line.to_string()) {
            let current_item = &line[0..line.find(' ').unwrap()];
            if current_item == item {
                let version_with_desc = &line[(line.find(' ').unwrap() + 1)..line.len()];
                let version = &version_with_desc[0..version_with_desc.find(' ').unwrap()];
                versions.push_str(&format!("{}\n", version));
            }
            else {
                if !versions.is_empty() {
                    let (category, pkg) = {
                        let mut item_split = item.split("/");
                        (item_split.next().unwrap(), item_split.next().unwrap())
                    };
                    let blank = String::new();
                    let installed_version = installed_version_output_map.get(&item).unwrap_or(&blank);
                    let mut keyword = String::new();
                    let recommended_version = recommended_version_output_map.get(&item).unwrap_or({
                        for global_keyword in global_keywords.iter() {
                            for arch in arch_list.iter() {
                                if global_keyword == arch {
                                    keyword = "Not available".to_string();
                                    break;
                                }
                                else if *global_keyword == format!("~{}", arch) {
                                    keyword = "Keyworded".to_string();
                                    break;
                                }
                            }
                        }
                        &keyword
                    });
                    self.execute("INSERT INTO all_packages (category, package, versions, installed_version, recommended_version, description)
                                  VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                                  &[&category, &pkg, &versions, &*installed_version, &*recommended_version, &desc]).unwrap();
                      
                      
                    if !installed_version.is_empty() {
                        self.execute("INSERT INTO installed_packages (category, package, versions, installed_version, recommended_version, description)
                                      VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                                      &[&category, &pkg, &versions, &*installed_version, &*recommended_version, &desc]).unwrap();
                    }
                }
                versions.clear();
                item = line[0..line.find(' ').unwrap()].to_string();
                let version_with_desc = &line[(line.find(' ').unwrap() + 1)..line.len()];
                let version = &version_with_desc[0..version_with_desc.find(' ').unwrap()];
                desc = version_with_desc[(version_with_desc.find(' ').unwrap() + 1)..version_with_desc.len()].to_string();
                versions.push_str(&format!("{}\n", version));
            }
        }
    }

    fn parse_for_sets(&self) {
        self.execute("CREATE TABLE portage_sets (
                      set                 TEXT,
                      category_and_pkg    TEXT,
                      versions            TEXT,
                      installed_version   TEXT,
                      recommended_version TEXT,
                      description         TEXT
                      )", &[]).unwrap();
        for set in fs::read_dir("/etc/portage/sets").expect("failed to find /etc/portage/sets directory") {
            let set = set.expect("intermittent IO error");
            //let set_name = set.file_name().into_string().unwrap();
            use ::std::io::BufRead;
            let set_file = ::std::io::BufReader::new(fs::File::open(set.path()).unwrap());
            for line in set_file.lines() {
                let mut line = line.unwrap();
                let mut split = line.split('/');
                let (category, pkg) = {
                    (split.next().unwrap(), split.next().unwrap())
                }; 
                //for all_pkg in self.all_packages_map.get(category).unwrap() {
                //    if all_pkg.name == pkg {
                //        let mut all_pkg_clone = all_pkg.clone();
                //        all_pkg_clone.name = line.to_string();
                //        self.portage_sets_map.entry(set_name.clone())
                //                              .or_insert(BTreeSet::new())
                //                              .insert(all_pkg_clone);
                //        break;
                //    }
                //}
                
                let mut statement = self.prepare("SELECT category, package, versions, installed_version, recommended_version, description FROM all_packages").expect("sql cannot be converted to a C string");
                let mut rows = statement.query(&[]).expect("failed to query database");
                while let Some(Ok(row)) = rows.next() {
                    if row.get::<_, String>(0) == category && row.get::<_, String>(1) == pkg {
                        self.execute("INSERT INTO portage_sets (set, category_and_pkg, versions, installed_version, recommended_version, description)
                                      VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                                      &[&set.path().file_name().unwrap().to_str(), &line, &row.get::<_, String>(2), &row.get::<_, String>(3), &row.get::<_, String>(4), &row.get::<_, String>(5)]).unwrap();
                        break;
                    }
                }
            }
        }
    }
}

#[allow(dead_code)]
pub fn parse_data_with_portageq() {
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
            }
        }
    }
}
