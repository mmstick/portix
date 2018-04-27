#![feature(pattern_parentheses)]
extern crate gtk;

use std::collections::{BTreeMap, BTreeSet};
use gtk::prelude::*;

use gtk::{Window, WindowType};

mod backend;

fn main() {
    if gtk::init().is_err() {
        println!("failed to initialize GTK.");
    }

    let menubar = gtk::MenuBar::new();
    menubar.append(&gtk::MenuItem::new_with_label(&"Actions"));
    menubar.append(&gtk::MenuItem::new_with_label(&"Settings"));
    menubar.append(&gtk::MenuItem::new_with_label(&"Help"));

    let toolbar = gtk::Toolbar::new();
    toolbar.insert(&gtk::ToolButton::new::<gtk::Widget, _, _>(None, "Emerge"), 0);
    toolbar.insert(&gtk::ToolButton::new::<gtk::Widget, _, _>(None, "Advance Emerge"), 1);
    toolbar.insert(&gtk::ToolButton::new::<gtk::Widget, _, _>(None, "Unmerge"), 2);
    toolbar.insert(&gtk::SeparatorToolItem::new(), 3);
    toolbar.insert(&gtk::ToolButton::new::<gtk::Widget, _, _>(None, "Sync"), 4);
    toolbar.insert(&gtk::ToolButton::new::<gtk::Widget, _, _>(None, "Upgrade"), 5);
    toolbar.insert(&gtk::ToolButton::new::<gtk::Widget, _, _>(None, "Queue"), 6);

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

    let mut pkg_data: BTreeMap<String, BTreeSet<backend::Pkg>> = BTreeMap::new();
    backend::parse_data_with_eix(&mut pkg_data);

    let model_category = gtk::ListStore::new(&[gtk::Type::String, gtk::Type::U64]);
    for (category, pkgs) in pkg_data.iter() {
        model_category.insert_with_values(None, &[0,1], &[&category, &(pkgs.len() as u64)]);
    }

    let tree_view_category = gtk::TreeView::new_with_model(&model_category);
    tree_view_category.append_column(&column_category);
    tree_view_category.append_column(&column_pkg_num);
    tree_view_category.set_visible(true);
    let scrollable_category = gtk::ScrolledWindow::new(None, None);
    scrollable_category.add(&tree_view_category);

    let column_packages = make_tree_view_column("Packages", 0);
    let column_installed = make_tree_view_column("Installed", 1);
    let column_recommended = make_tree_view_column("Recommended", 2);
    let column_download_size = make_tree_view_column("Download Size", 3);
    let column_description = make_tree_view_column("Description", 4);

    let model_pkg_list = gtk::ListStore::new(&[gtk::Type::String, gtk::Type::String, gtk::Type::String, gtk::Type::String, gtk::Type::String]);

    let tree_view_pkgs = gtk::TreeView::new_with_model(&model_pkg_list);
    tree_view_pkgs.append_column(&column_packages);
    tree_view_pkgs.append_column(&column_installed);
    tree_view_pkgs.append_column(&column_recommended);
    tree_view_pkgs.append_column(&column_download_size);
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

    tree_view_category.get_selection().connect_changed(move |selected_category| {
        model_pkg_list.clear();
        selected_category.set_mode(gtk::SelectionMode::Single);

        if let Some((tree_model_category, tree_iter_category)) = selected_category.get_selected() {
            if let Some(selected) = tree_model_category.get_value(&tree_iter_category, 0).get::<String>() {
                let pkgs = pkg_data.get(&selected).unwrap();
                for (i, pkg) in pkgs.iter().enumerate() {
                    let tree_iter_pkgs = model_pkg_list.insert(i as i32);
                    model_pkg_list.set(&tree_iter_pkgs, &[0, 2, 4], &[&pkg.name, &pkg.recommended_version, &pkg.desc]);
                }
            }
        }
    });

    window.connect_delete_event(|_, _| {
        gtk::main_quit();
        Inhibit(false)
    });

    gtk::main();
}
