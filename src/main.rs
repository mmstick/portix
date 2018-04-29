#![feature(pattern_parentheses)]
extern crate gtk;
extern crate rusqlite;

use backend::PortixConnection;

use gtk::prelude::*;
use gtk::{Window, WindowType};

use rusqlite::Connection;

mod backend;

fn main() {
    if gtk::init().is_err() {
        println!("failed to initialize GTK.");
    }
    let conn = Connection::open_in_memory().unwrap();
    conn.parse_for_pkgs();
    conn.parse_for_sets();

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

    let search = gtk::SearchEntry::new();
    search.set_hexpand(true);

    let hbox1 = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    hbox1.add(&gtk::Label::new("View: "));
    hbox1.add(&combo_box);
    hbox1.add(&gtk::Button::new_with_label("Refresh"));
    hbox1.add(&search);

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
    //for (category, pkgs) in data.all_packages_map.iter() {
    //}
    let mut statement_category = conn.prepare("SELECT DISTINCT category FROM all_packages").expect("sql cannot be converted to a C string");
    let mut rows_category = statement_category.query(&[]).expect("failed to query database");
    while let Some(Ok(row_category)) = rows_category.next() {
        let mut statement_pkg = conn.prepare(&format!("SELECT package FROM all_packages WHERE category = '{}'", row_category.get::<_, String>(0))).expect("sql cannot be converted to a C string");
        let mut rows_pkg = statement_category.query(&[]).expect("failed to query database");

        let mut pkg_count: u64 = 0;
        while let Some(Ok(_)) = rows_pkg.next() {
            pkg_count += 1;
        }
        model_category.insert_with_values(None, &[0,1], &[&row_category.get::<_, String>(0), &pkg_count]);
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
    let notebook_labels = ["Summary", "Dependencies", "Changelog", "Installed files", "Ebuild", "USE flags"];
    for &label in notebook_labels.iter() {
        notebook.append_page(&gtk::ScrolledWindow::new(None, None), Some(&gtk::Label::new(label)));
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

    //let data_clone = data.clone();
    combo_box.connect_changed(move |combo_box| {
        if let Some(entry) = combo_box.get_active_text() {
            model_category.clear();
            //if entry == "Installed Packages" {
            //    for (category, pkgs) in data_clone.installed_packages_map.iter() {
            //        model_category.insert_with_values(None, &[0, 1], &[&category, &(pkgs.len() as u64)]);
            //    }
            //}
            //else if entry == "All Packages" {
            //    for (category, pkgs) in data_clone.all_packages_map.iter() {
            //        model_category.insert_with_values(None, &[0, 1], &[&category, &(pkgs.len() as u64)]);
            //    }
            //}
            //else if entry == "Sets" {
            //    for (set, pkgs) in data_clone.portage_sets_map.iter() {
            //        model_category.insert_with_values(None, &[0, 1], &[&set, &(pkgs.len() as u64)]);
            //    }
            //}
        }
    });

    tree_view_category.get_selection().connect_changed(move |selected_category| {
        model_pkg_list.clear();
        selected_category.set_mode(gtk::SelectionMode::Single);

        if let Some((tree_model_category, tree_iter_category)) = selected_category.get_selected() {
            if let Some(selected) = tree_model_category.get_value(&tree_iter_category, 0).get::<String>() {
                let entry = combo_box.get_active_text().unwrap_or("".to_string());
                //let mut blank_set = std::collections::BTreeSet::new();
                //let pkgs = if entry == "Installed Packages"{
                //    //println!("{:?}", selected);
                //    data.installed_packages_map.get(&selected).unwrap_or(&blank_set)
                //}
                //else if entry == "All Packages" {
                //    data.all_packages_map.get(&selected).unwrap_or(&blank_set)
                //}
                //else if entry == "Sets" {
                //    data.portage_sets_map.get(&selected).unwrap_or(&blank_set)
                //}
                //else {
                //    data.all_packages_map.get(&selected).unwrap_or(&blank_set)
                //};
                //for (i, pkg) in pkgs.iter().enumerate() {
                //    let tree_iter_pkgs = model_pkg_list.insert(i as i32);
                //    model_pkg_list.set(&tree_iter_pkgs, &[0, 1, 2, 3], &[&pkg.name, &pkg.installed_version, &pkg.recommended_version, &pkg.desc]);
                //}
            }
        }
    });

    //tree_view_pkgs.get_selection().connect_changed(move |selected_pkg| {
    //    selected_pkg.set_mode(gtk::SelectionMode::Single);

    //    if let Some((tree_model_pkg, tree_iter_pkg)) = selected_pkg.get_selected() {
    //        if let Some(selected) = tree_model_pkg.get_value(&tree_iter_pkg, 0).get::<String>() {
    //        }
    //    }
    //});

    window.connect_delete_event(|_, _| {
        gtk::main_quit();
        Inhibit(false)
    });

    gtk::main();
}
