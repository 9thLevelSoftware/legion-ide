use xtask::completion::qualifies_as_product_evidence;

#[test]
fn direct_dispatch_and_fake_dependencies_cannot_certify_product() {
    assert!(!qualifies_as_product_evidence(
        "product",
        "runtime-dispatch",
        "passed",
        false,
    ));
    assert!(!qualifies_as_product_evidence(
        "product",
        "native-input",
        "passed",
        true,
    ));
    assert!(!qualifies_as_product_evidence(
        "product",
        "native-input",
        "skipped",
        false,
    ));
    assert!(qualifies_as_product_evidence(
        "product",
        "native-input",
        "passed",
        false,
    ));
}

#[test]
fn only_allowed_layers_routes_results_and_dependency_state_qualify() {
    let layers = ["component", "integrated", "product"];
    let routes = [
        "native-input",
        "runtime-dispatch",
        "snapshot-injection",
        "none",
    ];
    let results = ["passed", "failed", "skipped", "blocked"];
    let mut qualifying = Vec::new();

    for layer in layers {
        for route in routes {
            for result in results {
                for substituted in [false, true] {
                    if qualifies_as_product_evidence(layer, route, result, substituted) {
                        qualifying.push((layer, route, result, substituted));
                    }
                }
            }
        }
    }

    assert_eq!(
        qualifying,
        vec![("product", "native-input", "passed", false)]
    );
}

#[test]
fn unknown_and_empty_labels_do_not_qualify() {
    for (layer, route, result) in [
        ("Product", "native-input", "passed"),
        ("product", "NATIVE-INPUT", "passed"),
        ("product", "native-input", "PASSED"),
        ("unknown", "native-input", "passed"),
        ("", "", ""),
    ] {
        assert!(!qualifies_as_product_evidence(layer, route, result, false));
    }
}
