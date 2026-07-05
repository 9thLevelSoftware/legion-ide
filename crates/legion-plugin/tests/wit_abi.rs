use std::{fs, path::PathBuf};

#[test]
fn plugin_wit_abi_declares_grammar_theme_and_lsp_host_world() {
    let wit_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("wit");

    let grammars = fs::read_to_string(wit_dir.join("grammars.wit")).expect("read grammars.wit");
    assert!(grammars.contains("interface grammars"));
    assert!(grammars.contains("record grammar-contribution"));
    assert!(grammars.contains("register-grammar: func"));

    let themes = fs::read_to_string(wit_dir.join("themes.wit")).expect("read themes.wit");
    assert!(themes.contains("interface themes"));
    assert!(themes.contains("record theme-contribution"));
    assert!(themes.contains("register-theme: func"));

    let lsp = fs::read_to_string(wit_dir.join("lsp.wit")).expect("read lsp.wit");
    assert!(lsp.contains("interface lsp"));
    assert!(lsp.contains("record lsp-adapter-contribution"));
    assert!(lsp.contains("world plugin-host"));
    assert!(lsp.contains("import grammars"));
    assert!(lsp.contains("import themes"));
    assert!(lsp.contains("import lsp"));
}
