use super::{
    NUMBERED_ROWS, PasteRoute, PinRefusal, Popup, Tab, appeared, detector_for, expired,
    paste_route, row_number,
};
use copyveil_dbus::ItemSummary;

fn item(id: u64, sensitivity: &str, pin_slot: u32) -> ItemSummary {
    ItemSummary {
        id,
        kind: "text".into(),
        mime: String::new(),
        sensitivity: sensitivity.into(),
        reason: String::new(),
        pin_slot,
        age_ms: 0,
        expires_in_ms: 0,
        size: 1,
        preview: String::new(),
    }
}

/// Five Items, newest first: 5 (Secret), 4 (pinned in slot 2), 3, 2 (pinned in slot 1), 1.
fn popup() -> Popup {
    let mut popup = Popup::default();
    popup.set_items(vec![
        item(5, "secret", 0),
        item(4, "plain", 2),
        item(3, "suspected", 0),
        item(2, "plain", 1),
        item(1, "plain", 0),
    ]);
    popup
}

fn ids(popup: &Popup) -> Vec<u64> {
    popup.visible().iter().map(|item| item.id).collect()
}

#[test]
fn all_lists_newest_first_and_selects_the_first_row() {
    let popup = popup();
    assert_eq!(popup.tab(), Tab::All);
    assert_eq!(ids(&popup), [5, 4, 3, 2, 1]);
    assert_eq!(popup.selected().map(|item| item.id), Some(5));
    assert_eq!(popup.selected_index(), Some(0));
}

#[test]
fn pinned_lists_pin_slot_order() {
    let mut popup = popup();
    popup.set_tab(Tab::Pinned);
    assert_eq!(ids(&popup), [2, 4]);
    assert_eq!(popup.selected().map(|item| item.id), Some(2));
}

#[test]
fn arrows_move_and_stop_at_the_ends() {
    let mut popup = popup();
    popup.move_selection(1);
    assert_eq!(popup.selected_index(), Some(1));
    popup.move_selection(10);
    assert_eq!(popup.selected_index(), Some(4));
    popup.move_selection(-2);
    assert_eq!(popup.selected_index(), Some(2));
    popup.move_selection(-10);
    assert_eq!(popup.selected_index(), Some(0));
}

#[test]
fn an_empty_list_has_no_selection() {
    let mut popup = Popup::default();
    popup.move_selection(1);
    assert!(popup.selected().is_none());
    assert_eq!(popup.selected_index(), None);
    assert!(popup.numbered(1).is_none());
}

#[test]
fn a_refresh_keeps_the_selected_item() {
    let mut popup = popup();
    popup.move_selection(2);
    assert_eq!(popup.selected().map(|item| item.id), Some(3));
    let mut items: Vec<ItemSummary> = popup.visible().into_iter().cloned().collect();
    items.insert(0, item(6, "plain", 0));
    popup.set_items(items);
    assert_eq!(popup.selected().map(|item| item.id), Some(3));
    assert_eq!(popup.selected_index(), Some(3));
}

#[test]
fn a_refresh_without_the_selected_item_selects_the_first_row() {
    let mut popup = popup();
    popup.move_selection(1);
    popup.set_items(vec![item(9, "plain", 0), item(8, "plain", 0)]);
    assert_eq!(popup.selected().map(|item| item.id), Some(9));
}

#[test]
fn search_keeps_the_daemons_matches_in_list_order() {
    let mut popup = popup();
    popup.set_matches(Some(vec![3, 1]));
    assert_eq!(ids(&popup), [3, 1]);
    assert_eq!(popup.selected().map(|item| item.id), Some(3));
    popup.set_matches(Some(Vec::new()));
    assert!(popup.visible().is_empty());
    assert!(popup.selected().is_none());
    popup.set_matches(None);
    assert_eq!(ids(&popup), [5, 4, 3, 2, 1]);
}

#[test]
fn a_refresh_during_a_search_keeps_the_selection() {
    let mut popup = popup();
    popup.set_matches(Some(vec![4, 3, 1]));
    popup.move_selection(1);
    popup.set_matches_keeping_selection(Some(vec![4, 3, 1]));
    assert_eq!(popup.selected().map(|item| item.id), Some(3));
    popup.set_matches_keeping_selection(Some(vec![4, 1]));
    assert_eq!(popup.selected().map(|item| item.id), Some(4));
}

#[test]
fn search_and_the_pinned_tab_combine() {
    let mut popup = popup();
    popup.set_tab(Tab::Pinned);
    popup.set_matches(Some(vec![4, 3]));
    assert_eq!(ids(&popup), [4]);
}

#[test]
fn clicking_selects_only_visible_items() {
    let mut popup = popup();
    popup.select(2);
    assert_eq!(popup.selected().map(|item| item.id), Some(2));
    popup.set_tab(Tab::Pinned);
    popup.select(5);
    assert_eq!(popup.selected().map(|item| item.id), Some(2));
}

#[test]
fn ctrl_number_pastes_the_row_with_that_number() {
    let popup = popup();
    assert_eq!(popup.numbered(1).map(|item| item.id), Some(5));
    assert_eq!(popup.numbered(5).map(|item| item.id), Some(1));
    assert!(popup.numbered(6).is_none());
    assert!(popup.numbered(0).is_none());
    assert!(popup.numbered(NUMBERED_ROWS + 1).is_none());

    let mut many = Popup::default();
    many.set_items((1..=12).rev().map(|id| item(id, "plain", 0)).collect());
    assert_eq!(many.numbered(9).map(|item| item.id), Some(4));
    assert!(many.numbered(10).is_none());
}

#[test]
fn only_the_first_nine_rows_get_numbers() {
    assert_eq!(row_number(0), Some(1));
    assert_eq!(row_number(8), Some(9));
    assert_eq!(row_number(9), None);
}

#[test]
fn only_plain_items_may_be_pinned() {
    assert_eq!(Popup::may_pin(&item(1, "plain", 0)), Ok(()));
    assert_eq!(
        Popup::may_pin(&item(1, "secret", 0)),
        Err(PinRefusal::Secret)
    );
    assert_eq!(
        Popup::may_pin(&item(1, "suspected", 0)),
        Err(PinRefusal::Suspected)
    );
    assert!(PinRefusal::Secret.message().contains("expire"));
    assert!(PinRefusal::Suspected.message().contains("Not a secret"));
}

#[test]
fn only_suspected_items_offer_not_a_secret() {
    assert!(Popup::may_unflag(&item(1, "suspected", 0)));
    assert!(!Popup::may_unflag(&item(1, "secret", 0)));
    assert!(!Popup::may_unflag(&item(1, "plain", 0)));
}

#[test]
fn the_flagging_detector_is_found_by_its_description() {
    let detectors = vec![
        (
            "github-token".to_owned(),
            "GitHub tokens".to_owned(),
            "Looks like a GitHub token".to_owned(),
        ),
        (
            "jwt".to_owned(),
            "JSON Web Tokens".to_owned(),
            "Looks like a JSON Web Token".to_owned(),
        ),
    ];
    assert_eq!(
        detector_for("Looks like a JSON Web Token", &detectors),
        Some(("jwt", "JSON Web Tokens"))
    );
    assert_eq!(detector_for("From password manager", &detectors), None);
}

#[test]
fn only_rows_new_since_the_last_listing_draw_their_veil() {
    assert!(appeared(None, &[1, 2]).is_empty());
    assert_eq!(appeared(Some(&[1, 2]), &[3, 1, 2]), [3]);
    assert!(appeared(Some(&[1, 2]), &[2]).is_empty());
}

#[test]
fn only_rows_whose_expiry_came_dissolve() {
    // Deadlines in seconds: 2 is due at 5, 9 isn't.
    let shown = [(1, 2), (2, 9), (3, 1)];
    assert_eq!(expired(&shown, &[3], 5), [1]);
    // Removed by hand before its Expiry: it just goes.
    assert!(expired(&shown, &[1, 3], 5).is_empty());
    assert_eq!(expired(&shown, &[], 9), [1, 2, 3]);
}

#[test]
fn rich_text_keeps_its_formatting_unless_pasted_as_plain_text() {
    let mut rich = item(1, "plain", 0);
    rich.kind = "rich-text".into();
    assert_eq!(paste_route(&rich, false), PasteRoute::Rich);
    assert_eq!(paste_route(&rich, true), PasteRoute::Source);
    assert_eq!(paste_route(&item(2, "plain", 0), false), PasteRoute::Source);
    assert_eq!(
        paste_route(&item(3, "secret", 0), false),
        PasteRoute::Source
    );
}
