extern crate gtk;
//extern crate glib;
extern crate rusqlite;

use std::thread;
use std::rc::Rc;
use std::sync::mpsc::{channel, TryRecvError};

use backend::PortixConnection;

use gtk::prelude::*;
use gtk::{Window, WindowType};
//use glib::signal::{signal_handler_block, signal_handler_unblock};

use rusqlite::Connection;

mod backend;

enum Request {
    Ebuilds(String),
    FileList(String),
    ModelCategory(String),
    Other(String),
}

enum Response {
    Other(Vec<(String, String, String, String)>),
    ModelCategory(Vec<(String, i32)>),
    StringQuery(String)
}

fn main() {
    let (db_request_sender, db_request_receiver) = channel::<Request>();
    let (db_response_sender, db_response_receiver) = channel::<Response>();

    thread::spawn(move || {
        let conn = Connection::open(backend::DB_PATH).expect("failed to open connection");
        rusqlite::vtab::csvtab::load_module(&conn).unwrap();
        let conn = if !conn.tables_exist() {
            loading_tables(conn)
        } else if conn.tables_need_reloading() {
            println!("*Database needs reloading again*");
            loading_tables(conn)
        } else {
            conn
        };

        loop {
            let selection = match db_request_receiver.recv() {
                Ok(selection) => selection,
                Err(why) => {
                    eprintln!("error when receiving: {}", why);
                    return;
                }
            };

            match selection {
                Request::Ebuilds(query) => {
                    db_response_sender.send(Response::StringQuery(conn.get_ebuild_with_query(&query)));
                },
                Request::FileList(query) => {
                    db_response_sender.send(Response::StringQuery(backend::get_file_list(&query)));
                }
                Request::ModelCategory(query) => {
                    let mut statement = match conn.prepare(&query) {
                        Ok(selection) => selection,
                        Err(why) => {
                            eprintln!("failed to prepare query: {}", why);
                            continue
                        }
                    };

                    let mut rows = match statement.query(&[]) {
                        Ok(selection) => selection,
                        Err(why) => {
                            eprintln!("failed to query: {}", why);
                            continue
                        }
                    };

                    let mut response = Vec::new();
                    while let Some(Ok(row)) = rows.next() {
                        response.push((row.get::<_, String>(0), row.get::<_, i32>(1)));
                    }

                    db_response_sender.send(Response::ModelCategory(response));
                }
                Request::Other(query) => {
                    let mut statement = match conn.prepare(&query) {
                        Ok(selection) => selection,
                        Err(why) => {
                            eprintln!("failed to prepare query: {}", why);
                            continue
                        }
                    };

                    let mut rows = match statement.query(&[]) {
                        Ok(selection) => selection,
                        Err(why) => {
                            eprintln!("failed to query: {}", why);
                            continue
                        }
                    };

                    let mut response = Vec::new();
                    while let Some(Ok(row)) = rows.next() {
                        response.push((row.get::<_, String>(0), row.get::<_, String>(1), row.get::<_, String>(2), row.get::<_, String>(3)));
                    }

                    db_response_sender.send(Response::Other(response));
                }
            };
        }
    });

    let db_request = Rc::new(db_request_sender);
    let db_response = Rc::new(db_response_receiver);

    if gtk::init().is_err() {
        println!("failed to initialize GTK.");
    }

    fn loading_tables(conn: Connection) -> Connection {
        println!("(1/4) Storing repo hash info into database...");
        conn.store_repo_hashes();
        println!("Done");
        println!("(2/4) Loading package info into database...");
        conn.parse_for_pkgs();
        println!("Done");
        println!("(3/4) Loading portage set info into database...");
        conn.parse_for_sets();
        println!("Done");
        println!("(4/4) Loading ebuild info into database...");
        conn.parse_for_ebuilds();
        println!("Done");
        conn
    }

    let menubar = gtk::MenuBar::new();
    menubar.append(&gtk::MenuItem::new_with_label(&"Actions"));
    menubar.append(&gtk::MenuItem::new_with_label(&"Settings"));
    menubar.append(&gtk::MenuItem::new_with_label(&"Help"));

    let toolbuttons: Vec<_> = {
        let icon_names_and_labels = [("list-add", "Emerge"), ("emblem-system", "Advance Emerge"), ("list-remove", "Unmerge"), ("view-refresh", "Sync"), ("media-seek-forward", "Upgrade"), ("media-playback-start", "Queue")];
        icon_names_and_labels.iter().map(|&(icon, label)| {
            gtk::ToolButton::new(&gtk::Image::new_from_icon_name(icon, 100), label)
        }).collect()
    };
    let separator = gtk::SeparatorToolItem::new();
    toolbuttons[0].set_sensitive(false);
    toolbuttons[1].set_sensitive(false);
    toolbuttons[2].set_sensitive(false);
    let toolbar = gtk::Toolbar::new();
    toolbar.insert(&toolbuttons[0], 0);
    toolbar.insert(&toolbuttons[1], 1);
    toolbar.insert(&toolbuttons[2], 2);
    toolbar.insert(&separator, 3);
    toolbar.insert(&toolbuttons[3], 4);
    toolbar.insert(&toolbuttons[4], 5);
    toolbar.insert(&toolbuttons[5], 6);
    toolbar.set_property_toolbar_style(gtk::ToolbarStyle::Both);

    let combo_box = gtk::ComboBoxText::new();
    let combo_box_labels = ["All Packages", "Installed Packages", "Search Results", "Upgradeable Packages", "Deprecated Packages", "Sets"];
    for label in combo_box_labels.iter() {
        combo_box.append_text(label);
    }
    combo_box.set_active(0); // Set "All Packages" to be default

    let search_entry = gtk::SearchEntry::new();
    search_entry.set_hexpand(true);

    let hbox1 = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    hbox1.add(&gtk::Label::new("View: "));
    hbox1.add(&combo_box);
    hbox1.add(&gtk::Button::new_with_label("Refresh"));
    hbox1.add(&search_entry);

    pub fn make_tree_view_column(title: &str, column_number: i32) -> gtk::TreeViewColumn {
        let column = gtk::TreeViewColumn::new();
        let cell = gtk::CellRendererText::new();
        column.set_visible(true);
        column.set_title(title);
        column.pack_start(&cell, false);
        column.add_attribute(&cell, "text", column_number);
        column
    }

    let column_category = make_tree_view_column("Categories", 0);
    let column_pkg_num = make_tree_view_column("# Pkgs", 1);

    let model_category = gtk::ListStore::new(&[gtk::Type::String, gtk::Type::U64]);

    db_request.send(Request::ModelCategory("SELECT category, count(DISTINCT name) as pkg_count
                                      FROM all_packages
                                      GROUP BY category".into()));
    match db_response.recv() {
        Ok(Response::ModelCategory(rows)) => {
            for row in rows {
                model_category.insert_with_values(None, &[0, 1], &[&row.0, &row.1]);
            }
        }
        Ok(_) => {
            eprintln!("invalid response");
            return;
        }
        Err(why) => {
            eprintln!("failed to receive: {}", why);
            return;
        }
    }

    let tree_view_category = gtk::TreeView::new_with_model(&model_category);
    tree_view_category.append_column(&column_category);
    tree_view_category.append_column(&column_pkg_num);
    tree_view_category.set_visible(true);
    let scrollable_category = gtk::ScrolledWindow::new(None, None);
    scrollable_category.add(&tree_view_category);
    scrollable_category.set_size_request(300, 400);

    let column_packages = make_tree_view_column("Packages", 0);
    let column_installed = make_tree_view_column("Installed", 1);
    let column_recommended = make_tree_view_column("Recommended", 2);
    let column_description = make_tree_view_column("Description", 3);

    let model_pkg_list = gtk::ListStore::new(&[gtk::Type::String, gtk::Type::String, gtk::Type::String, gtk::Type::String, gtk::Type::String]);

    let tree_view_pkgs = gtk::TreeView::new_with_model(&model_pkg_list);
    tree_view_pkgs.append_column(&column_packages);
    tree_view_pkgs.append_column(&column_installed);
    tree_view_pkgs.append_column(&column_recommended);
    tree_view_pkgs.append_column(&column_description);
    tree_view_pkgs.set_visible(true);
    let scrollable_pkg = gtk::ScrolledWindow::new(None, None);
    scrollable_pkg.add(&tree_view_pkgs);

    let paned_category_pkg = gtk::Paned::new(gtk::Orientation::Horizontal);
    paned_category_pkg.add1(&scrollable_category);
    paned_category_pkg.add2(&scrollable_pkg);
    paned_category_pkg.set_wide_handle(true);
    paned_category_pkg.set_hexpand(true);

    let notebook = gtk::Notebook::new();
    let notebook_labels = ["Summary", "Dependencies", "Installed files", "Ebuild", "USE flags"];
    let notebook_buffers = Rc::new([gtk::TextBuffer::new(&gtk::TextTagTable::new()),
                                    gtk::TextBuffer::new(&gtk::TextTagTable::new()),
                                    gtk::TextBuffer::new(&gtk::TextTagTable::new()),
                                    gtk::TextBuffer::new(&gtk::TextTagTable::new()),
                                    gtk::TextBuffer::new(&gtk::TextTagTable::new())]);
    for (&label, buffer) in notebook_labels.iter().zip(notebook_buffers.iter()) {
        let scrolled_window = gtk::ScrolledWindow::new(None, None);
        scrolled_window.add(&gtk::TextView::new_with_buffer(buffer));
        notebook.append_page(&scrolled_window, Some(&gtk::Label::new(label)));
    }

    let paned_everything = gtk::Paned::new(gtk::Orientation::Vertical);
    paned_everything.add1(&paned_category_pkg);
    paned_everything.add2(&notebook);
    paned_everything.set_wide_handle(true);
    paned_everything.set_vexpand(true);

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 5);
    vbox.add(&menubar);
    vbox.add(&toolbar);
    vbox.add(&hbox1);
    vbox.add(&paned_everything);

    let window = Window::new(WindowType::Toplevel);
    window.set_title("Portage GUI");
    window.set_default_size(1200, 800);
    window.add(&vbox);
    window.show_all();

    {
        let db_response = db_response.clone();
        let db_request = db_request.clone();
        let tree_view_pkgs = tree_view_pkgs.clone();
        let model_category = model_category.clone();
        combo_box.connect_changed(move |combo_box| {
            tree_view_pkgs.get_selection().unselect_all();
            if let Some(entry) = combo_box.get_active_text() {
                let selection = match &*entry {
                    "Installed Packages" =>
                        "SELECT category, count(DISTINCT name) as pkg_count
                         FROM installed_packages
                         GROUP BY category",
                    "All Packages" =>
                        "SELECT category, count(DISTINCT name) as pkg_count
                         FROM all_packages
                         GROUP BY category",
                    "Sets" =>
                        "SELECT portage_set, count(DISTINCT category_and_name) as pkg_count
                         FROM portage_sets
                         GROUP BY portage_set",
                    _ => return,
                };

                model_category.clear();
                db_request.send(Request::ModelCategory(selection.into()));

                let model_category = model_category.clone();
                let db_response = db_response.clone();
                gtk::timeout_add(100, move || {

                    match db_response.try_recv() {
                        Ok(Response::ModelCategory(rows)) => {
                            for row in rows {
                                model_category.insert_with_values(None, &[0, 1], &[&row.0, &row.1]);
                            }
                            Continue(false)
                        }
                        Ok(_) => {
                            eprintln!("invalid response");
                            Continue(false)
                        }
                        Err(TryRecvError::Empty) => Continue(true),
                        Err(why) => {
                            eprintln!("failed to receive: {}", why);
                            Continue(false)
                        }
                    }
                });
            }
        });
    }

    {
        let db_response = db_response.clone();
        let db_request = db_request.clone();
        let combo_box = combo_box.clone();
        let tree_view_pkgs = tree_view_pkgs.clone();
        let model_pkg_list = model_pkg_list.clone();
        tree_view_category.get_selection().connect_changed(move |selected_category| {
            model_pkg_list.clear();
            tree_view_pkgs.get_selection().unselect_all();
            selected_category.set_mode(gtk::SelectionMode::Single);

            if let Some((tree_model_category, tree_iter_category)) = selected_category.get_selected() {
                if let Some(selected) = tree_model_category.get_value(&tree_iter_category, 0).get::<String>() {
                    let entry = combo_box.get_active_text().unwrap_or("".to_string());
                    let selection = match &*entry {
                        "Installed Packages" => format!(r#"SELECT installed_packages.name AS package_name,
                                                           IFNULL(installed_packages.version, "") AS installed_version,
                                                           IFNULL(recommended_packages.version, "Not available") AS recommended_version,
                                                           all_packages.description AS description
                                                           FROM installed_packages
                                                           LEFT JOIN all_packages
                                                           ON installed_packages.category = all_packages.category
                                                           AND installed_packages.name = all_packages.name
                                                           LEFT JOIN recommended_packages
                                                           ON all_packages.category = recommended_packages.category
                                                           AND all_packages.name = recommended_packages.name
                                                           WHERE installed_packages.category LIKE '{}'
                                                           GROUP BY package_name
                                                           ORDER BY installed_packages.category ASC"#,
                                                           selected),

                        "All Packages" => format!(r#"SELECT all_packages.name AS package_name,
                                                     IFNULL(installed_packages.version, "") AS installed_version,
                                                     IFNULL(recommended_packages.version, "Not available") AS recommended_version,
                                                     all_packages.description AS description
                                                     FROM all_packages
                                                     LEFT JOIN installed_packages
                                                     ON all_packages.category = installed_packages.category
                                                     AND all_packages.name = installed_packages.name
                                                     LEFT JOIN recommended_packages
                                                     ON all_packages.category = recommended_packages.category
                                                     AND all_packages.name = recommended_packages.name
                                                     WHERE all_packages.category LIKE '{}'
                                                     GROUP BY package_name
                                                     ORDER BY all_packages.category ASC"#,
                                                     selected),

                        "Sets" => format!(r#"SELECT portage_sets.category_and_name AS category_and_name,
                                             IFNULL(installed_packages.version, "") AS installed_version,
                                             IFNULL(recommended_packages.version, "Not available") AS recommended_version,
                                             all_packages.description AS description
                                             FROM portage_sets
                                             LEFT JOIN all_packages
                                             ON portage_sets.category = all_packages.category
                                             AND portage_sets.name = all_packages.name
                                             LEFT JOIN installed_packages
                                             ON portage_sets.category = installed_packages.category
                                             AND portage_sets.name = installed_packages.name
                                             LEFT JOIN recommended_packages
                                             ON portage_sets.category = recommended_packages.category
                                             AND portage_sets.name = recommended_packages.name
                                             WHERE portage_sets.portage_set LIKE '{}'
                                             GROUP BY category_and_name
                                             ORDER BY portage_sets.portage_set ASC"#,
                                             selected),

                        _ => format!(r#"SELECT all_packages.name AS package_name,
                                        IFNULL(installed_packages.version, "") AS installed_version,
                                        IFNULL(recommended_packages.version, "Not available") AS recommended_version,
                                        all_packages.description AS description
                                        FROM all_packages
                                        LEFT JOIN installed_packages
                                        ON all_packages.category = installed_packages.category
                                        AND all_packages.name = installed_packages.name
                                        LEFT JOIN recommended_packages
                                        ON all_packages.category = recommended_packages.category
                                        AND all_packages.name = recommended_packages.name
                                        WHERE all_packages.category LIKE '{}'
                                        GROUP BY package_name
                                        ORDER BY all_packages.category ASC"#,
                                        selected),
                    };

                    db_request.send(Request::Other(selection));

                    let model_pkg_list = model_pkg_list.clone();
                    let db_response = db_response.clone();
                    gtk::timeout_add(100, move || {
                        match db_response.try_recv() {
                            Ok(Response::Other(pkg_rows)) => {
                                for row in pkg_rows {
                                    model_pkg_list.insert_with_values(None, &[0, 1, 2, 3], &[&row.0, &row.1, &row.2, &row.3]);
                                }
                                Continue(false)
                            },
                            Ok(_) => {
                                eprintln!("invalid response");
                                Continue(false)
                            }
                            Err(TryRecvError::Empty) => Continue(true),
                            Err(why) => {
                                eprintln!("error when receiving: {}", why);
                                Continue(false)
                            }
                        }
                    });
                }
            }
        });
    }

    {
        let db_response = db_response.clone();
        let db_request = db_request.clone();
        let combo_box = combo_box.clone();
        let notebook = notebook.clone();
        let notebook_buffers = notebook_buffers.clone();
        tree_view_pkgs.get_selection().connect_changed(move |selected_pkg| {
            selected_pkg.set_mode(gtk::SelectionMode::Single);

            if let Some((tree_model_pkg, tree_iter_pkg)) = selected_pkg.get_selected() {
                if let Some(selected) = tree_model_pkg.get_value(&tree_iter_pkg, 0).get::<String>() {
                    let entry = combo_box.get_active_text().unwrap_or("".to_string());
                    if let Some(current_page) = notebook.get_current_page() {
                        let query = match current_page {
                            page if page == 2 => {
                                Request::FileList(if entry == "Sets" {
                                    let split: Vec<&str> = selected.split('/').collect();
                                    let package = match split.get(1) {
                                        Some(a) => *a,
                                        None => return,
                                    };
                                    package.into()
                                }
                                else { selected })
                            }
                            page if page == 3 => {
                                Request::Ebuilds(if entry == "Sets" {
                                    let split: Vec<&str> = selected.split('/').collect();
                                    format!("SELECT ebuild_path
                                             FROM ebuilds
                                             WHERE ebuilds.name = '{}'",
                                                 match split.get(1) {
                                                     Some(a) => a,
                                                     None => return,
                                                 }
                                           )
                                }
                                else {
                                    format!("SELECT ebuild_path
                                             FROM ebuilds
                                             WHERE ebuilds.name = '{}'", selected)
                                })
                            }
                            _ => return,
                        };

                        db_request.send(query);

                        let notebook_buffers = notebook_buffers.clone();
                        let db_response = db_response.clone();
                        gtk::timeout_add(100, move || {
                            match db_response.try_recv() {
                                Ok(Response::StringQuery(response)) => {
                                    notebook_buffers[current_page as usize].set_text(&response);
                                    Continue(false)
                                },
                                Ok(_) => {
                                    eprintln!("invalid response");
                                    Continue(false)
                                }
                                Err(TryRecvError::Empty) => Continue(true),
                                Err(why) => {
                                    eprintln!("error when receiving: {}", why);
                                    Continue(false)
                                }
                            }
                        });
                    }
                }
            }
        });
    }

    {
        let db_response = db_response.clone();
        let db_request = db_request.clone();
        notebook.connect_switch_page(move |_, _, current_page| {
            let package_selection = tree_view_pkgs.get_selection();
            package_selection.set_mode(gtk::SelectionMode::Single);

            if let Some((tree_model_pkg, tree_iter_pkg)) = package_selection.get_selected() {
                if let Some(selected) = tree_model_pkg.get_value(&tree_iter_pkg, 0).get::<String>() {
                    let entry = combo_box.get_active_text().unwrap_or("".to_string());

                    let query = match current_page {
                        page if page == 2 => {
                            Request::FileList(if entry == "Sets" {
                                let split: Vec<&str> = selected.split('/').collect();
                                let package = match split.get(1) {
                                    Some(a) => *a,
                                    None => return,
                                };
                                package.into()
                            }
                            else { selected })
                        }
                        page if page == 3 => {
                            Request::Ebuilds(if entry == "Sets" {
                                let split: Vec<&str> = selected.split('/').collect();
                                format!("SELECT ebuild_path
                                         FROM ebuilds
                                         WHERE ebuilds.name = '{}'",
                                             match split.get(1) {
                                                 Some(a) => a,
                                                 None => return,
                                             }
                                       )
                            }
                            else {
                                format!("SELECT ebuild_path
                                         FROM ebuilds
                                         WHERE ebuilds.name = '{}'", selected)
                            })
                        }
                        _ => return,
                    };

                    db_request.send(query);
                    let db_response = db_response.clone();
                    let notebook_buffers = notebook_buffers.clone();
                    gtk::timeout_add(100, move || {
                        match db_response.try_recv() {
                            Ok(Response::StringQuery(response)) => {
                                notebook_buffers[current_page as usize].set_text(&response);
                                Continue(false)
                            },
                            Ok(_) => {
                                eprintln!("invalid response");
                                Continue(false)
                            }
                            Err(TryRecvError::Empty) => Continue(true),
                            Err(why) => {
                                eprintln!("error when receiving: {}", why);
                                Continue(false)
                            }
                        }
                    });
                }
            }
        });
    }

    {
        let db_response = db_response.clone();
        let db_request = db_request.clone();
        search_entry.connect_activate(move |search_entry| {
            let search_entry = search_entry.clone();
            model_pkg_list.clear();

            if let Some(search) = search_entry.get_text() {
                let query = format!(r#"SELECT all_packages.name AS package_name,
                                       IFNULL(installed_packages.version, "") AS installed_version,
                                       IFNULL(recommended_packages.version, "Not available") AS recommended_version,
                                       all_packages.description AS description
                                       FROM all_packages
                                       LEFT JOIN installed_packages
                                       ON all_packages.category = installed_packages.category
                                       AND all_packages.name = installed_packages.name
                                       LEFT JOIN recommended_packages
                                       ON all_packages.category = recommended_packages.category
                                       AND all_packages.name = recommended_packages.name
                                       WHERE all_packages.name LIKE '%{}%'
                                       GROUP BY package_name
                                       ORDER BY all_packages.category ASC"#,
                                       search);
                db_request.send(Request::Other(query));
            }

            let model_pkg_list = model_pkg_list.clone();
            let db_response = db_response.clone();
            gtk::timeout_add(100, move || {
                match db_response.try_recv() {
                    Ok(Response::Other(pkg_rows)) => {
                        for row in pkg_rows {
                            model_pkg_list.insert_with_values(None, &[0, 1, 2, 3], &[&row.0, &row.1, &row.2, &row.3]);
                        }
                        Continue(false)
                    },
                    Ok(_) => {
                        eprintln!("invalid response");
                        Continue(false)
                    }
                    Err(TryRecvError::Empty) => Continue(true),
                    Err(why) => {
                        eprintln!("error when receiving: {}", why);
                        Continue(false)
                    }
                }
            });
        });
    }

    window.connect_delete_event(|_, _| {
        gtk::main_quit();
        Inhibit(false)
    });

    gtk::main();
}
