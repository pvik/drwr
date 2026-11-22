use super::*;

fn text(html: &str) -> String {
    from_html(html, true).text
}

#[test]
fn breaks_lines_where_html_does() {
    assert_eq!(text("a<br>b"), "a\nb");
    assert_eq!(text("<div>a</div><div>b</div>"), "a\nb");
    assert_eq!(text("a<div>b</div>c"), "a\nb\nc");
    // A break at the end of a block adds no line of its own.
    assert_eq!(text("<div>a<br></div><div>b</div>"), "a\nb");
    // Empty lines that push the text up are kept.
    assert_eq!(text("VtGate<div><br></div><div><br></div>"), "VtGate\n\n");
    assert_eq!(text("<br>a"), "\na");
    assert_eq!(text("<ul><li>one</li><li>two</li></ul>"), "• one\n• two");
}

#[test]
fn breaks_lines_at_newlines_unless_told_not_to() {
    assert_eq!(text("a\nb"), "a\nb");
    assert_eq!(text("a<div>b</div>\n"), "a\nb\n");
    // nl2Br=0
    assert_eq!(from_html("  a \n  b  ", false).text, "a b");
}

#[test]
fn collapses_whitespace_like_html() {
    assert_eq!(text("  a \t  b  "), "a b");
    assert_eq!(text("a&nbsp;&nbsp;b"), "a  b");
    assert_eq!(
        from_html("<div> a </div>\n<div> b</div>", false).text,
        "a\nb"
    );
    assert_eq!(text("<table><tr><td>a</td><td>b</td></tr></table>"), "a b");
}

#[test]
fn decodes_entities() {
    assert_eq!(
        text("a &amp; b &lt;c&gt; &#39;d&#x27; &hellip;"),
        "a & b <c> 'd' …"
    );
    assert_eq!(text("&NotEqualTilde; 👩&zwj;💻"), "≂̸ 👩‍💻");
    // Unknown or unterminated ones stay as written.
    assert_eq!(text("&bogus; &amp"), "&bogus; &amp");
    assert_eq!(text("1 < 2"), "1 < 2");
}

#[test]
fn leaves_out_characters_xml_cant_hold() {
    assert_eq!(text("a&#1;b&#x7;c&#0;d&#xD800;"), "abc\u{fffd}d\u{fffd}");
    assert_eq!(from_plain("a\u{1}b\tc").text, "ab\tc");
}

#[test]
fn drops_comments_styles_and_tags() {
    assert_eq!(text("a<!-- b -->c"), "ac");
    assert_eq!(text("<style>p { color: red }</style>a"), "a");
    assert_eq!(text("<span title=\"x > y\">a</span>"), "a");
    assert_eq!(text("<!DOCTYPE html><?xml version=\"1.0\"?>a"), "a");
}

#[test]
fn keeps_bold_and_italic_that_cover_everything() {
    let all = from_html("<b>Title</b>", true);
    assert!(all.bold && !all.italic && !all.lost_formatting);
    let nested = from_html("<i><b>a</b> <b>b</b></i>", true);
    assert!(nested.bold && nested.italic && !nested.lost_formatting);
    let part = from_html("<b>Title</b> body", true);
    assert!(!part.bold && part.lost_formatting);
    let part = from_html("<i>Title</i> body", true);
    assert!(!part.italic && part.lost_formatting);
}

#[test]
fn notices_formatting_it_drops() {
    assert!(from_html("<font color=\"#ff0000\">a</font>", true).lost_formatting);
    assert!(from_html("<span style=\"font-size: 20px\">a</span>", true).lost_formatting);
    assert!(
        !from_html("<span style=\"background-color: initial;\">a</span>", true).lost_formatting
    );
    assert!(!from_html("<div>a<br>b</div>", true).lost_formatting);
}

#[test]
fn plain_labels_keep_their_text() {
    assert_eq!(from_plain("a <b>\r\nc").text, "a <b>\nc");
}
