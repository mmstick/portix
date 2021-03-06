extern crate rayon;
extern crate regex;
extern crate rusqlite;

use self::rayon::prelude::*;

use self::regex::Regex;

use self::rusqlite::Connection;

use std::fs;
use std::io::prelude::*;
use std::process::Command;
use std::thread;
use std::sync::{Arc, Mutex};

pub const DB_PATH: &str = "./target/debug/portix.db";

pub trait PortixConnection {
    fn parse_for_pkgs(&self);
    fn parse_for_sets(&self);
    fn parse_for_ebuilds(&self);
    fn get_ebuild_with_query(&self, query: &str) -> String;
    fn get_search_count(&self, search: &str) -> i32;
    fn store_repo_hashes(&self);
    fn tables_need_reloading(&self) -> bool;
    fn tables_exist(&self) -> bool;
}

impl PortixConnection for Connection {
    fn parse_for_pkgs(&self) {
        let child_output = thread::spawn(move || {
            String::from_utf8(
                Command::new("sh")
                    .arg("-c")
                    .arg(r#"NAMEVERSION='<category>=<name>=<version>=<description>\n' EIX_LIMIT_COMPACT=0 eix -c --format '<availableversions:NAMEVERSION>' --pure-packages|sed -e 's|[,\/"]||g' -e 's/=/,/;s/=/,/;s/=/,/'"#)
                    .output()
                    .expect("failed to get eix output")
                    .stdout
                ).expect("eix output is not UTF-8 compatible")
        });

        let child_installed_version_output = thread::spawn(move || {
            String::from_utf8(
                Command::new("sh")
                    .arg("-c")
                    .arg(r#"portageq match / '*/*'|sed -re "s/-([0-9])/,\1/" -e "s/\//\,/g""#)
                    .output()
                    .expect("failed to get qlist output")
                    .stdout
                ).expect("qlist output is not UTF-8 compatible")
        });

        let child_recommended_version_output = thread::spawn(move || {
            String::from_utf8(
                Command::new("sh")
                    .arg("-c")
                    .arg(r#"NAMEVERSION='<category>=<name>=<version>\n' EIX_LIMIT_COMPACT=0 eix -c --format '<bestversion:NAMEVERSION>' --pure-packages|sed -e 's|[,\/"]||g' -e 's/=/,/;s/=/,/'"#)
                    .output()
                    .expect("failed to get eix output")
                    .stdout
                ).expect("eix output is not UTF-8 compatible")
        });

        //let child_global_keywords = thread::spawn(move || {
        //    let global_keywords = String::from_utf8(
        //        Command::new("sh")
        //            .arg("-c")
        //            .arg(r"emerge --info|grep ACCEPT_KEYWORDS")
        //            .output()
        //            .expect("failed to get emerge output")
        //            .stdout
        //        ).expect("emerge output is not UTF-8 compatible");
        //    global_keywords[(global_keywords.find("\"").unwrap() + 1)..global_keywords.rfind("\"").unwrap()]
        //        .split(' ').map(|s| s.to_string()).collect::<Vec<_>>()
        //});


        //let child_arch_list = thread::spawn(move || {
        //    let arch_list = String::from_utf8(
        //        Command::new("sh")
        //            .arg("-c")
        //            .arg(r"cat $(portageq get_repo_path / gentoo)/profiles/arch.list")
        //            .output()
        //            .expect("failed to get portageq output")
        //            .stdout
        //        ).expect("portageq output is not UTF-8 compatible");
        //    let mut list = Vec::new();
        //    for arch in arch_list.lines() {
        //        if arch.is_empty() {
        //            break;
        //        }
        //        list.push(arch.to_string());
        //    }
        //    list
        //});


        let output = child_output.join().unwrap();
        let installed_packages_output = child_installed_version_output.join().unwrap();
        let recommended_packages_output = child_recommended_version_output.join().unwrap();

        let mut all_packages_csv = fs::OpenOptions::new().write(true).create(true).open("./target/debug/portix_all_packages.csv").expect("failed to create portix_all_packages.csv file");
        let mut installed_packages_csv = fs::OpenOptions::new().write(true).create(true).open("./target/debug/portix_installed_packages.csv").expect("failed to create portix_installed_packages.csv file");
        let mut recommended_packages_csv = fs::OpenOptions::new().write(true).create(true).open("./target/debug/portix_recommended_packages.csv").expect("failed to create portix_recommended_packages.csv file");
        all_packages_csv.write_all(output.as_bytes()).expect("failed to write all packages output into file");
        installed_packages_csv.write_all(installed_packages_output.as_bytes()).expect("failed to write installed packages output into file");
        recommended_packages_csv.write_all(recommended_packages_output.as_bytes()).expect("failed to write recommended packages output into file");

        self.execute_batch("DROP TABLE IF EXISTS all_packages;
                            CREATE VIRTUAL TABLE all_packages_vtab
                            USING csv('./target/debug/portix_all_packages.csv', category, name, version, description);
                            CREATE TABLE all_packages AS SELECT * FROM all_packages_vtab;
                            DROP TABLE all_packages_vtab;

                            DROP TABLE IF EXISTS installed_packages;
                            CREATE VIRTUAL TABLE installed_packages_vtab
                            USING csv('./target/debug/portix_installed_packages.csv', category, name, version);
                            CREATE TABLE installed_packages AS SELECT * FROM installed_packages_vtab;
                            DROP TABLE installed_packages_vtab;

                            DROP TABLE IF EXISTS recommended_packages;
                            CREATE VIRTUAL TABLE recommended_packages_vtab
                            USING csv('./target/debug/portix_recommended_packages.csv', category, name, version);
                            CREATE TABLE recommended_packages AS SELECT * FROM recommended_packages_vtab;
                            DROP TABLE recommended_packages_vtab;").unwrap();

        fs::remove_file("./target/debug/portix_all_packages.csv")
            .expect("failed to remove portix_all_packages.csv file due to lack of permissions");
        fs::remove_file("./target/debug/portix_installed_packages.csv")
            .expect("failed to remove portix_installed_packages.csv file due to lack of permissions");
        fs::remove_file("./target/debug/portix_recommended_packages.csv")
            .expect("failed to remove portix_recommended_packages.csv file due to lack of permissions");

        //let global_keywords = child_global_keywords.join().unwrap();
        //let arch_list = child_arch_list.join().unwrap();
        //let mut item = String::new();
        //let mut desc = String::new();
        //let mut versions = String::new();
        //output.push_str("extra line needed to get previous item in iterator\n");
        //for line in output.lines().map(|line| line.to_string()) {
        //    let current_item = &line[0..line.find(' ').unwrap()];
        //    if current_item == item {
        //        let version_with_desc = &line[(line.find(' ').unwrap() + 1)..line.len()];
        //        let version = &version_with_desc[0..version_with_desc.find(' ').unwrap()];
        //        versions.push_str(&format!("{}\n", version));
        //    }
        //    else {
        //        if !versions.is_empty() {
        //            let (category, pkg) = {
        //                let mut item_split = item.split("/");
        //                (item_split.next().unwrap(), item_split.next().unwrap())
        //            };
        //            let blank = String::new();
        //            let installed_version = installed_version_output_map.get(&item).unwrap_or(&blank);
        //            let mut keyword = String::new();
        //            let recommended_version = recommended_version_output_map.get(&item).unwrap_or({
        //                for global_keyword in global_keywords.iter() {
        //                    for arch in arch_list.iter() {
        //                        if global_keyword == arch {
        //                            keyword = "Not available".to_string();
        //                            break;
        //                        }
        //                        else if *global_keyword == format!("~{}", arch) {
        //                            keyword = "Keyworded".to_string();
        //                            break;
        //                        }
        //                    }
        //                }
        //                &keyword
        //            });
        //            self.execute("INSERT INTO all_packages (category, package, versions, installed_version, recommended_version, description)
        //                          VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        //                          &[&category, &pkg, &versions, &*installed_version, &*recommended_version, &desc]).unwrap();
        //              
        //              
        //            if !installed_version.is_empty() {
        //                self.execute("INSERT INTO installed_packages (category, package, versions, installed_version, recommended_version, description)
        //                              VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        //                              &[&category, &pkg, &versions, &*installed_version, &*recommended_version, &desc]).unwrap();
        //            }
        //        }
        //        versions.clear();
        //        item = line[0..line.find(' ').unwrap()].to_string();
        //        let version_with_desc = &line[(line.find(' ').unwrap() + 1)..line.len()];
        //        let version = &version_with_desc[0..version_with_desc.find(' ').unwrap()];
        //        desc = version_with_desc[(version_with_desc.find(' ').unwrap() + 1)..version_with_desc.len()].to_string();
        //        versions.push_str(&format!("{}\n", version));
        //    }
        //}
    }

    fn parse_for_sets(&self) {
        self.execute_batch("DROP TABLE IF EXISTS portage_sets;
                            CREATE TABLE portage_sets (
                            portage_set       TEXT,
                            category_and_name TEXT,
                            category          TEXT,
                            name              TEXT
                            );").unwrap();
        for set in fs::read_dir("/etc/portage/sets").expect("failed to find /etc/portage/sets directory") {
            let set = set.expect("intermittent IO error");
            use ::std::io::BufRead;
            let set_file = ::std::io::BufReader::new(fs::File::open(set.path()).unwrap());
            for line in set_file.lines() {
                let mut line = line.unwrap();
                let mut split = line.split('/');
                let (category, package) = {
                    (split.next().unwrap(), split.next().unwrap())
                }; 
                
                let mut statement = self.prepare("SELECT category, name FROM all_packages").expect("sql cannot be converted to a C string");
                let mut rows = statement.query(&[]).expect("failed to query database");
                while let Some(Ok(row)) = rows.next() {
                    if row.get::<_, String>(0) == category && row.get::<_, String>(1) == package {
                        self.execute("INSERT INTO portage_sets (portage_set, category_and_name, category, name)
                                      VALUES (?1, ?2, ?3, ?4)",
                                      &[&set.path().file_name().unwrap().to_str(), &line, &category, &package]).unwrap();
                        break;
                    }
                }
            }
        }
    }

    fn parse_for_ebuilds(&self) {
        let repos = String::from_utf8(Command::new("sh")
                .arg("-c")
                .arg("portageq get_repos /")
                .output()
                .expect("failed to get repos list")
                .stdout
            ).expect("repo names are not UTF-8 compatible");
        let repos: Vec<String> = repos.trim().split(' ').map(|repo| repo.to_owned()).collect();
        let csv_string = Arc::new(Mutex::new(String::new()));

        for repo in repos.into_iter() {
            let repo_path = String::from_utf8(Command::new("sh")
                    .arg("-c")
                    .arg(format!("portageq get_repo_path / {}", repo))
                    .output()
                    .expect("failed to find repo path")
                    .stdout
                ).expect("repo path is not UTF-8 compatible");
            let repo_path = repo_path.trim();

            let csv_string_clone = csv_string.clone();
            fs::read_dir(repo_path).expect("repo path does not exist").collect::<Vec<_>>().par_iter().for_each(|category_entry| {
                let category_path = category_entry.as_ref().expect("intermittent IO error").path();
                let category = category_path.clone();
                let category = category.file_name().unwrap().to_string_lossy();
                if category_path.is_file() || category.starts_with(".") {
                    return;
                }

                for package_entry in fs::read_dir(category_path).expect("category path does not exist") {
                    let package_path = package_entry.expect("intermittent IO error").path();
                    let package = package_path.clone();
                    let package = package.file_name().unwrap().to_string_lossy();
                    if package_path.is_file() || package.starts_with(".") {
                        continue;
                    }

                    for file_entry in fs::read_dir(package_path).unwrap() {
                        let file_path = file_entry.expect("intermittent IO error").path();
                        let file_string = file_path.file_name().unwrap().to_string_lossy();
                        if file_string.ends_with(".ebuild") {
                            let regex = Regex::new(r".*-(\d.*).ebuild").unwrap();
                            let version = &regex.captures(&file_string).unwrap()[1];
                            let mut lock = csv_string_clone.lock().expect("failed to lock csv string");
                            lock.push_str(&category);
                            lock.push_str(",");
                            lock.push_str(&package);
                            lock.push_str(",");
                            lock.push_str(version);
                            lock.push_str(",");
                            lock.push_str(&file_path.to_str().unwrap());
                            lock.push_str("\n");
                        }
                    }
                }
            });
        }

        csv_string.try_lock().unwrap().pop(); // pop out the last unneeded new line character
        let mut ebuilds_csv = fs::File::create("./target/debug/portix_ebuilds.csv").expect("failed to create portix_ebuilds.csv file");
        ebuilds_csv.write_all(&mut csv_string.try_lock().unwrap().as_bytes()).expect("failed to write to portix_ebuilds.csv file");

        self.execute_batch("DROP TABLE IF EXISTS ebuilds;
                            CREATE VIRTUAL TABLE ebuilds_vtab
                            USING csv('./target/debug/portix_ebuilds.csv', category, name, version, ebuild_path);
                            CREATE TABLE ebuilds AS SELECT * FROM ebuilds_vtab;
                            DROP TABLE ebuilds_vtab;").unwrap();

        fs::remove_file("./target/debug/portix_ebuilds.csv")
            .expect("failed to remove portix_ebuilds.csv file due to lack of permissions");
    }

    fn get_ebuild_with_query(&self, query: &str) -> String {
        let mut statement = self.prepare(query).expect("sql cannot be converted to a C string");
        let mut queries = statement.query(&[]).expect("failed to query database");
        if let Some(Ok(query)) = queries.next() {
            let mut ebuild_text = String::new();
            let mut ebuild_file = fs::File::open(query.get::<_, String>(0)).unwrap();
            ebuild_file.read_to_string(&mut ebuild_text).unwrap();
            ebuild_text
        }
        else {
            String::new()
        }
    }

    fn get_search_count(&self, search: &str) -> i32 {
        let count = format!(r#"SELECT count() as search_count
                               FROM (
                               SELECT *
                               FROM all_packages
                               WHERE all_packages.name LIKE '%{}%'
                               GROUP BY all_packages.name
                               ORDER BY all_packages.category ASC
                               )"#,
                               search);
        let mut statement = self.prepare(&count).expect("sql cannot be converted to a C string");
        let mut query_count = statement.query(&[]).expect("failed to query database");
        query_count.next().unwrap().unwrap().get::<_, i32>(0)
    }

    fn store_repo_hashes(&self) {
        self.execute_batch("DROP TABLE IF EXISTS repo_hashes;
                            CREATE TABLE repo_hashes (
                            repo_path  TEXT,
                            head_hash TEXT
                            );").unwrap();

        let repos = String::from_utf8(Command::new("sh")
                .arg("-c")
                .arg("portageq get_repos /")
                .output()
                .expect("failed to get repos list")
                .stdout
            ).expect("repo names are not UTF-8 compatible");
        let repos: Vec<_> = repos.trim().split(' ').collect();

        for repo in repos.into_iter() {
            let repo_path = String::from_utf8(Command::new("sh")
                    .arg("-c")
                    .arg(format!("portageq get_repo_path / {}", repo))
                    .output()
                    .expect("failed to find repo path")
                    .stdout
                ).expect("repo path is not UTF-8 compatible");
            let repo_path = repo_path.trim();
            let git_head_hash = String::from_utf8(
                    match Command::new("sh").arg("-c").arg("git rev-parse @").current_dir(repo_path).output() {
                        Ok(a) => a.stdout,
                        Err(_) => continue,
                    }
                ).expect("repo hash is not UTF-8 compatible");

            self.execute("INSERT INTO repo_hashes (repo_path, head_hash)
                          VALUES (?1, ?2)",
                          &[&repo_path, &git_head_hash.trim()]).expect("failed to insert data into repo_hashes table");
        }
    }

    fn tables_need_reloading(&self) -> bool {
        let mut statement = self.prepare("SELECT repo_path, head_hash FROM repo_hashes").expect("sql cannot be converted to a C string");
        let mut rows = statement.query(&[]).expect("failed to query database");

        while let Some(Ok(row)) = rows.next() {
            let repo_path = row.get::<_, String>(0);
            let previous_head_hash = row.get::<_, String>(1);

            let new_head_hash = String::from_utf8(
                    match Command::new("sh").arg("-c").arg("git rev-parse @").current_dir(repo_path).output() {
                        Ok(a) => a.stdout,
                        Err(_) => continue,
                    }
                ).expect("repo hash is not UTF-8 compatible");
            if previous_head_hash != new_head_hash.trim() {
                return true;
            }
        }
        false
    }

    fn tables_exist(&self) -> bool {
        let mut statement = self.prepare("SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'all_packages'").expect("sql cannot be converted to a C string");
        let mut query_all_packages = statement.query(&[]).expect("failed to query database");

        let mut statement = self.prepare("SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'installed_packages'").expect("sql cannot be converted to a C string");
        let mut query_installed_packages = statement.query(&[]).expect("failed to query database");

        let mut statement = self.prepare("SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'recommended_packages'").expect("sql cannot be converted to a C string");
        let mut query_recommended_packages = statement.query(&[]).expect("failed to query database");

        let mut statement = self.prepare("SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'portage_sets'").expect("sql cannot be converted to a C string");
        let mut query_portage_sets = statement.query(&[]).expect("failed to query database");

        let mut statement = self.prepare("SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'ebuilds'").expect("sql cannot be converted to a C string");
        let mut query_ebuilds = statement.query(&[]).expect("failed to query database");

        let mut statement = self.prepare("SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'repo_hashes'").expect("sql cannot be converted to a C string");
        let mut query_repo_hashes = statement.query(&[]).expect("failed to query database");

        if query_all_packages.next().unwrap().unwrap().get::<_, i32>(0) == 1 &&
           query_installed_packages.next().unwrap().unwrap().get::<_, i32>(0) == 1 &&
           query_recommended_packages.next().unwrap().unwrap().get::<_, i32>(0) == 1 &&
           query_portage_sets.next().unwrap().unwrap().get::<_, i32>(0) == 1 &&
           query_ebuilds.next().unwrap().unwrap().get::<_, i32>(0) == 1 &&
           query_repo_hashes.next().unwrap().unwrap().get::<_, i32>(0) == 1 {
               return true;
        }
        false
    }
}

pub fn get_file_list(package: &str) -> String {
    String::from_utf8(Command::new("sh")
            .arg("-c")
            .arg(format!("qlist {}", package))
            .output()
            .expect("failed to get qlist output")
            .stdout
        ).expect("repo names are not UTF-8 compatible")
}
