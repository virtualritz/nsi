use super::*;

#[test]
fn a_row_of_its_own_node_renames_there_and_nowhere_else() {
    assert_eq!(draft_attribute("nurbs", "nu"), Some("u.count"));
    assert_eq!(draft_attribute("mesh", "nu"), None);
    // `sourcemodels` is the instance source, which is why `objects`
    // means two things and the row is scoped.
    assert_eq!(
        draft_attribute("instances", "sourcemodels"),
        Some("objects")
    );
    assert_eq!(draft_attribute("mesh", "sourcemodels"), None);
}

#[test]
fn a_common_row_renames_on_every_node() {
    for node_type in ["mesh", "nurbs", "global", "whatever"] {
        assert_eq!(draft_attribute(node_type, "nicename"), Some("nice-name"));
    }
}

#[test]
fn the_osl_globals_are_not_renamed() {
    for name in ["P", "N", "Pw", "u", "v"] {
        assert_eq!(draft_attribute("mesh", name), None, "{name}");
        assert_eq!(draft_attribute("nurbs", name), None, "{name}");
    }
}

#[test]
fn an_unknown_name_is_not_guessed_at() {
    assert_eq!(draft_attribute("mesh", "somethingelse"), None);
    assert_eq!(legacy_attribute("mesh", "something-else"), None);
}

#[test]
fn every_row_maps_back() {
    for &(node_type, legacy, draft) in table::ATTRIBUTES {
        assert_eq!(draft_attribute(node_type, legacy), Some(draft));
        assert_eq!(legacy_attribute(node_type, draft), Some(legacy));
    }
}

#[test]
fn node_types_map_both_ways() {
    assert_eq!(draft_node_type("outputdriver"), Some("output-driver"));
    assert_eq!(legacy_node_type("output-driver"), Some("outputdriver"));
    assert_eq!(draft_node_type("mesh"), None);
}
