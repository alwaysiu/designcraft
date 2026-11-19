//! Synthetic regression coverage for IDML script labels (#1129).

use std::io::{Cursor, Read, Write};

use designcraft_doc::ItemId;
use designcraft_engine::{DocState, Session};
use designcraft_idml::{export_idml, import_idml};
use serde_json::json;

fn parent_fixture() -> Vec<u8> {
    let xml = r#"<Document><Layer Self="L" Name="Synthetic"/>
      <MasterSpread Self="m" Name="A-Parent" NamePrefix="A" BaseName="Parent" PageCount="1">
        <Page Self="mp" GeometricBounds="0 0 200 200"/>
        <Rectangle Self="r" ItemLayer="L" ContentType="Unassigned">
          <Properties><Label>
            <KeyValuePair Key="Label" Value="slot-01"/>
            <KeyValuePair Key="slot" Value="01"/>
          </Label><PathGeometry><GeometryPathType PathOpen="false"><PathPointArray>
            <PathPointType Anchor="20 20"/><PathPointType Anchor="20 80"/>
            <PathPointType Anchor="80 80"/><PathPointType Anchor="80 20"/>
          </PathPointArray></GeometryPathType></PathGeometry></Properties>
        </Rectangle>
      </MasterSpread>
      <Spread Self="sp" PageCount="1">
        <Page Self="p" AppliedMaster="m" GeometricBounds="0 0 200 200"/>
      </Spread></Document>"#;
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("designmap.xml", zip::write::SimpleFileOptions::default()).unwrap();
    zip.write_all(xml.as_bytes()).unwrap();
    zip.finish().unwrap().into_inner()
}

fn zip_text(bytes: &[u8], prefix: &str) -> String {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut out = String::new();
    for i in 0..zip.len() {
        let mut part = zip.by_index(i).unwrap();
        if part.name().starts_with(prefix) {
            part.read_to_string(&mut out).unwrap();
        }
    }
    out
}

#[test]
fn parent_script_labels_survive_import_override_and_export() {
    let doc = import_idml(&parent_fixture()).unwrap();
    let parent = doc.parents[0].items[0].id;
    assert_eq!(doc.item(parent).unwrap().label, "slot-01");
    assert_eq!(doc.item(parent).unwrap().script_label("slot"), "01");
    let mut session = Session::new();
    session.add_document(DocState::new(doc, None));
    let result = session.execute("layout.overrideParentItems", &json!({"page": 0, "ids": [parent.0]})).unwrap();
    let copies = result["ids"].as_array().unwrap();
    assert_eq!(copies.len(), 1);
    let copy = ItemId(copies[0].as_u64().unwrap());
    let doc = &session.doc().unwrap().doc;
    assert_eq!(doc.item(copy).unwrap().overrides, Some(parent));
    assert_eq!(doc.item(copy).unwrap().label, "slot-01");
    assert_eq!(doc.item(copy).unwrap().script_label("slot"), "01");
    for query in [json!({"label": "slot-01"}), json!({"key": "slot", "value": "01"})] {
        let result = session.execute("object.findByLabel", &query).unwrap();
        let ids = result["ids"].as_array().unwrap();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&json!(parent.0)));
        assert!(ids.contains(&json!(copy.0)));
    }
    let bytes = export_idml(&session.doc().unwrap().doc);
    for prefix in ["MasterSpreads/", "Spreads/"] {
        let xml = zip_text(&bytes, prefix);
        assert_eq!(xml.matches("<Label>").count(), 1, "{prefix}: {xml}");
        assert!(xml.contains(r#"<KeyValuePair Key="Label" Value="slot-01"/>"#), "{prefix}: {xml}");
        assert!(xml.contains(r#"<KeyValuePair Key="slot" Value="01"/>"#), "{prefix}: {xml}");
    }
    let back = import_idml(&bytes).unwrap();
    assert_eq!(back.parents[0].items[0].label, "slot-01");
    assert_eq!(back.spreads[0].items[0].label, "slot-01");
    assert_eq!(back.spreads[0].items[0].script_label("slot"), "01");
    assert_eq!(back.spreads[0].items[0].overrides, Some(back.parents[0].items[0].id));
    back.check().unwrap();
}

fn assert_label_round_trip(label: &str, encoded: &str) {
    let mut session = Session::new();
    session.execute("file.new", &json!({"pages": 1, "facingPages": false})).unwrap();
    let item = session.execute("frame.create", &json!({"rect": [20, 20, 80, 80], "content": "unassigned"})).unwrap()["id"].clone();
    for key in ["Label", "slot"] {
        session.execute("object.insertLabel", &json!({"key": key, "value": label, "ids": [item]})).unwrap();
    }
    let bytes = export_idml(&session.doc().unwrap().doc);
    let xml = zip_text(&bytes, "Spreads/");
    assert!(xml.contains(&format!(r#"Key="Label" Value="{encoded}""#)), "{xml}");
    assert!(xml.contains(&format!(r#"Key="slot" Value="{encoded}""#)), "{xml}");
    let back = import_idml(&bytes).unwrap();
    let frame = &back.spreads[0].items[0];
    assert_eq!(frame.label, label);
    assert_eq!(frame.script_label("slot"), label);
    let mut reopened = Session::new();
    reopened.add_document(DocState::new(back, None));
    for query in [json!({"label": label}), json!({"key": "slot", "value": label})] {
        let found = reopened.execute("object.findByLabel", &query).unwrap();
        assert_eq!(found["ids"].as_array().unwrap().len(), 1);
    }
}

#[test]
fn korean_script_label_survives_idml_round_trip() {
    assert_label_round_trip("L야프-01", "L야프-01");
}

#[test]
fn xml_escaped_script_label_survives_idml_round_trip() {
    assert_label_round_trip("slot-&\"<01", "slot-&amp;&quot;&lt;01");
}
