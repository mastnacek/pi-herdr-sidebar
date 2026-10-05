use super::creation::picker_viewport;

#[test]
fn the_viewport_grows_with_the_box_and_follows_the_selection() {
    // 8-row box → 6 rows of projects.
    assert_eq!(picker_viewport(12, 0, 8), (6, 0));
    assert_eq!(picker_viewport(12, 5, 8), (6, 0));
    assert_eq!(picker_viewport(12, 6, 8), (6, 1));
    assert_eq!(picker_viewport(12, 11, 8), (6, 6));
    // A short list fits whole, no scrolling.
    assert_eq!(picker_viewport(3, 2, 8), (3, 0));
    // A taller box shows more rows.
    assert_eq!(picker_viewport(12, 11, 14), (12, 0));
}
